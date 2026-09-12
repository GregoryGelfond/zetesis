//! Located source declarations and display selection, separate from semantics.

mod compile;

pub use compile::{MetadataError, MetadataFeature, MetadataLimits, MetadataResource};

use std::collections::BTreeSet;

use themelios_base::span::Location;
use themelios_program::program::{Program as SourceProgram, Show, Statement};
use themelios_program::provenance::Origin;
use themelios_program::provenance::WithProvenance;
use themelios_program::raise::{Occurrences, StatementOccurrence};
use themelios_program::symbol::Signature;
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;
use zetesis_core::{Atom, Predicate};

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

/// Atom-channel selection only. Term observations are independent. Models that display the same atoms remain distinct
/// underlying stable models and must be counted/enumerated independently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputSelection {
    explicit: bool,
    signatures: BTreeSet<Predicate>,
}

/// Truthful atom-channel name; `OutputSelection` remains the compatibility name.
pub type AtomSelection = OutputSelection;

/// Inclusive admission limits for an explicit signed-signature slice.
#[derive(Clone, Copy, Debug)]
pub struct AtomSelectionLimits {
    /// Input occurrences, including duplicates, before set construction.
    pub max_signatures: usize,
    /// Sum of input UTF-8 predicate-name lengths, including duplicates.
    /// The resulting set holds at most this text plus one Predicate per input;
    /// allocator overhead and tree bookkeeping are excluded.
    pub max_name_bytes: usize,
}
impl Default for AtomSelectionLimits {
    fn default() -> Self {
        Self {
            max_signatures: 1_024,
            max_name_bytes: 1_048_576,
        }
    }
}

/// Explicit atom selection was refused before cloning any signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomSelectionError {
    /// Too many input occurrences.
    Signatures {
        /// Inclusive ceiling.
        limit: usize,
        /// Required count.
        observed: usize,
    },
    /// Cumulative predicate-name text exceeds its inclusive ceiling.
    NameBytes {
        /// Inclusive ceiling.
        limit: usize,
        /// Required byte count.
        observed: u128,
    },
}
impl std::fmt::Display for AtomSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "atom selection refused: {self:?}")
    }
}
impl std::error::Error for AtomSelectionError {}

impl OutputSelection {
    /// Select all original atoms. No allocation or observation evaluation occurs.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }
    /// Select no original atoms. Term observations remain independent.
    #[must_use]
    pub fn none() -> Self {
        Self {
            explicit: true,
            signatures: BTreeSet::new(),
        }
    }
    /// Select the union of the supplied signed predicate signatures.
    /// Input occurrences and text are checked before cloning; equal signatures
    /// deduplicate without changing the limits charged for their input. An empty
    /// slice selects no atoms. Time is O(N log N) predicate comparisons, including
    /// name bytes; retained text/cells are bounded by the admitted slice.
    ///
    /// # Errors
    /// Returns a typed inclusive count/text refusal before constructing the set.
    pub fn from_signatures(
        signatures: &[Predicate],
        limits: AtomSelectionLimits,
    ) -> Result<Self, AtomSelectionError> {
        if signatures.len() > limits.max_signatures {
            return Err(AtomSelectionError::Signatures {
                limit: limits.max_signatures,
                observed: signatures.len(),
            });
        }
        let mut bytes = 0_u128;
        for signature in signatures {
            bytes += signature.name().len() as u128;
            if bytes > limits.max_name_bytes as u128 {
                return Err(AtomSelectionError::NameBytes {
                    limit: limits.max_name_bytes,
                    observed: bytes,
                });
            }
        }
        Ok(Self {
            explicit: true,
            signatures: signatures.iter().cloned().collect(),
        })
    }

    /// Whether a signature or empty `#show` directive was present. When false,
    /// every atom is displayed; when true, only the selected signatures are.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.explicit
    }
    /// The union of signed predicate signatures selected explicitly. An empty
    /// set displays no atoms when explicit, and all atoms otherwise.
    #[must_use]
    pub fn signatures(&self) -> &BTreeSet<Predicate> {
        &self.signatures
    }
    /// Whether an atom is selected for display. This must not be used to prune
    /// candidates, reduct closure, stable-model identity, or model counts.
    #[must_use]
    pub fn includes(&self, atom: &Atom) -> bool {
        !self.explicit || self.signatures.contains(atom.predicate())
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
                    metadata.output.explicit = true;
                    metadata.output.signatures.insert(signature.clone());
                }
                SourceDirective::ShowEmpty => metadata.output.explicit = true,
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
