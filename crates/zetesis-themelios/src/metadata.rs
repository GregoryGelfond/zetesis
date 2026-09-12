//! Located source declarations and display selection, separate from semantics.

mod compile;
mod selection;

pub use selection::{AtomSelection, AtomSelectionError, AtomSelectionLimits, OutputSelection};

pub use compile::{MetadataError, MetadataFeature, MetadataLimits, MetadataResource};

use themelios_base::span::Location;
use themelios_program::program::{Program as SourceProgram, Show, Statement};
use themelios_program::provenance::Origin;
use themelios_program::provenance::WithProvenance;
use themelios_program::raise::{Occurrences, StatementOccurrence};
use themelios_program::symbol::Signature;
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;
use zetesis_core::Predicate;

use crate::diagnostic::unsupported;
use crate::expansion::check;
use crate::{
    AdmissionFailure, ExpansionFailure, ExpansionLimits, ExpansionResource, ProfileFeature,
};

/// An accepted metadata directive. None constructs a logical rule, domain
/// value, candidate atom, or projection of stable-model identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceDirective {
    /// `#defined p/n.` declares an intended signature for source diagnostics.
    Defined(Predicate),
    /// `#show p/n.` adds this signature to the explicit display selection.
    ShowSignature(Predicate),
    /// `#show.` activates explicit display selection without adding a signature.
    /// It does not remove signatures supplied by other `#show` directives.
    ShowEmpty,
    /// A term-valued observation; it does not activate signature-only output.
    ShowTerm,
}

/// One original metadata occurrence, before equal directives are deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedDirective {
    directive: SourceDirective,
    location: Location,
}

impl LocatedDirective {
    /// The interpreted declaration or display control.
    #[must_use]
    pub fn directive(&self) -> &SourceDirective {
        &self.directive
    }
    /// Original statement span in its original source.
    #[must_use]
    pub fn location(&self) -> Location {
        self.location
    }
}

/// Original declaration/display evidence and the resulting display selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceMetadata {
    directives: Vec<LocatedDirective>,
    output: OutputSelection,
    pub(crate) observations: crate::observation::ObservationProgram,
}

impl SourceMetadata {
    /// Every original metadata occurrence, ordered by source identity and span.
    /// Duplicate directives retain their separate locations.
    #[must_use]
    pub fn directives(&self) -> &[LocatedDirective] {
        &self.directives
    }
    /// Presentation policy for full, independently enumerated stable models.
    #[must_use]
    pub fn output(&self) -> &OutputSelection {
        &self.output
    }

    /// Atom-channel policy; term observations remain a separate channel.
    #[must_use]
    pub fn atom_selection(&self) -> &AtomSelection {
        &self.output
    }

    /// Source-free atom policy with no directive provenance or term observations.
    #[must_use]
    pub fn for_atoms(selection: AtomSelection) -> Self {
        Self {
            output: selection,
            ..Self::default()
        }
    }

    /// Bounded term queries, evaluated separately from full-model semantics.
    #[must_use]
    pub fn observations(&self) -> &crate::observation::ObservationProgram {
        &self.observations
    }

    pub(crate) fn finish(mut self) -> Self {
        self.directives
            .sort_by_key(|entry| (entry.location.source, entry.location.span));
        self
    }
}

pub(crate) fn check_syntax(
    statement: &ast::Statement,
    parsed: &Parse<ast::Program>,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    let location = parsed.location(statement.syntax().text_range());
    match statement {
        ast::Statement::Defined(_) => {}
        ast::Statement::Show(show) => {
            if !formula
                && (show.term().is_some() || show.colon_token().is_some() || show.body().is_some())
            {
                return Err(unsupported(ProfileFeature::ShowTerm, location));
            }
        }
        _ => return Ok(()),
    }
    Ok(())
}

pub(crate) fn check_count(
    parsed: &Parse<ast::Program>,
    limits: ExpansionLimits,
    used: &mut usize,
) -> Result<(), ExpansionFailure> {
    for statement in parsed.tree().statements() {
        if matches!(
            statement,
            ast::Statement::Defined(_) | ast::Statement::Show(_)
        ) {
            let location = parsed.location(statement.syntax().text_range());
            let observed = (*used as u128) + 1;
            check(
                ExpansionResource::MetadataStatements,
                observed,
                limits.max_metadata_statements,
                location,
            )?;
            *used += 1;
        }
    }
    Ok(())
}

pub(crate) fn collect(
    program: &SourceProgram,
    metadata: &mut SourceMetadata,
) -> Result<(), AdmissionFailure> {
    collect_profile(program, metadata, false)
}

pub(crate) fn collect_profile(
    program: &SourceProgram,
    metadata: &mut SourceMetadata,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    collect_carriers(program.statements(), metadata, formula)
}

/// Original declarations precede occurrence-copy admission. The upstream stream
/// retains duplicates and locations; `finish` establishes the public location
/// order. Formula metadata accepts every raised Show shape, and a raised
/// signature's Name is nonempty, so changing collection order adds no diagnostic
/// precedence between otherwise valid source declarations.
pub(crate) fn collect_occurrences(
    occurrences: &Occurrences,
    metadata: &mut SourceMetadata,
) -> Result<(), AdmissionFailure> {
    collect_carriers(
        occurrences
            .occurrences()
            .iter()
            .map(StatementOccurrence::statement),
        metadata,
        true,
    )
}

fn collect_carriers<'a>(
    carriers: impl Iterator<Item = &'a WithProvenance<Statement>>,
    metadata: &mut SourceMetadata,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    for carrier in carriers {
        if !matches!(carrier.get(), Statement::Defined(_) | Statement::Show(_)) {
            continue;
        }
        for origin in carrier.provenance().origins() {
            let Origin::Parsed(location) = origin else {
                continue;
            };
            let directive = match carrier.get() {
                Statement::Defined(defined) => {
                    SourceDirective::Defined(predicate(&defined.signature, *location)?)
                }
                Statement::Show(Show::Signature(signature)) => {
                    SourceDirective::ShowSignature(predicate(signature, *location)?)
                }
                // themelios names its faithful `#show.` value `Show::All`;
                // clingo interprets the empty directive as show nothing.
                Statement::Show(Show::All) => SourceDirective::ShowEmpty,
                Statement::Show(_) if formula => SourceDirective::ShowTerm,
                Statement::Show(_) => return Err(unsupported(ProfileFeature::ShowTerm, *location)),
                _ => continue,
            };
            match &directive {
                SourceDirective::Defined(_) | SourceDirective::ShowTerm => {}
                SourceDirective::ShowSignature(signature) => {
                    metadata.output.include(signature.clone());
                }
                SourceDirective::ShowEmpty => metadata.output.mark_explicit(),
            }
            metadata.directives.push(LocatedDirective {
                directive,
                location: *location,
            });
        }
    }
    Ok(())
}

fn predicate(signature: &Signature, location: Location) -> Result<Predicate, AdmissionFailure> {
    let arity = usize::try_from(signature.arity)
        .expect("supported Rust targets represent u32 arities in usize");
    Predicate::with_sign(
        signature.name.as_str(),
        arity,
        crate::coherence::core_sign(signature.sign),
    )
    .map_err(|error| AdmissionFailure::Construction { error, location })
}
