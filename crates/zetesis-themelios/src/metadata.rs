//! Located source declarations and display selection, separate from semantics.

mod compile;
#[cfg(test)]
mod tests;
mod vocabulary;
pub(crate) use vocabulary::{
    Authority as Admission, Constructor, MetadataVocabulary, Read, Scalar,
};
pub use vocabulary::{MetadataStorageError, MetadataStorageLimits};
mod selection;
mod projection;

pub use projection::{PreparedProjection, ProjectSelection};

pub use selection::{
    AtomSelection, AtomSelectionError, AtomSelectionLimits, OutputSelection, PreparedSelection,
    Signatures,
};

pub use compile::{MetadataError, MetadataFeature, MetadataLimits, MetadataResource};

use themelios_base::span::Location;
use themelios_program::program::{Program as SourceProgram, Project, Show, Statement};
use themelios_program::provenance::Origin;
use themelios_program::provenance::WithProvenance;
use themelios_program::raise::Occurrences;
use themelios_program::symbol::Signature;
use themelios_syntax::ast;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::AstNode;
pub(crate) use vocabulary::Predicate;
use zetesis_core::Predicate as OwnedPredicate;

use crate::diagnostic::unsupported;
use crate::expansion::check;
use crate::{
    AdmissionFailure, ExpansionFailure, ExpansionLimits, ExpansionResource, ProfileFeature,
    ProgramSite, StatementId,
};

/// An accepted borrowed metadata directive. Signature payload belongs to the
/// enclosing immutable metadata vocabulary, independent of logical admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceDirective<'a> {
    /// Intended signed signature for source diagnostics.
    Defined(zetesis_core::catalog::PredicateRef<'a>),
    /// Explicit atom display signature.
    ShowSignature(zetesis_core::catalog::PredicateRef<'a>),
    /// Activate explicit atom display without adding a signature.
    ShowEmpty,
    /// Term-valued observation, separate from atom selection.
    ShowTerm,
    /// Explicit projected-enumeration signature.
    ProjectSignature(zetesis_core::catalog::PredicateRef<'a>),
    /// Atom/body projection requiring complete source grounding.
    ProjectAtom,
}
#[derive(Clone, Copy, Debug)]
enum DirectiveKind {
    Defined(Predicate),
    ShowSignature(Predicate),
    ShowEmpty,
    ShowTerm,
    ProjectSignature(Predicate),
    ProjectAtom,
}
#[derive(Clone, Copy, Debug)]
struct Directive {
    kind: DirectiveKind,
    location: Location,
}
/// One original metadata occurrence, including its original source span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocatedDirective<'a> {
    directive: SourceDirective<'a>,
    location: Location,
}
impl<'a> LocatedDirective<'a> {
    /// The interpreted declaration or display control.
    #[must_use]
    pub fn directive(self) -> SourceDirective<'a> {
        self.directive
    }
    /// Original statement span in its original source.
    #[must_use]
    pub fn location(self) -> Location {
        self.location
    }
}
/// Borrowed original directive occurrences in source-location order.
#[derive(Clone, Copy)]
pub struct Directives<'a> {
    read: Option<Read<'a>>,
    entries: &'a [Directive],
}
impl<'a> Directives<'a> {
    /// Number of original occurrences, including duplicates.
    #[must_use]
    pub fn len(self) -> usize {
        self.entries.len()
    }
    /// Whether no metadata directive was authored.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.entries.is_empty()
    }
    /// Read one original occurrence by its published position.
    #[must_use]
    pub fn at(self, index: usize) -> Option<LocatedDirective<'a>> {
        self.entries.get(index).map(|entry| self.bind(entry))
    }
    fn bind(self, entry: &Directive) -> LocatedDirective<'a> {
        let predicate = |position| {
            self.read
                .expect("predicate coordinate has a published vocabulary")
                .predicate(position)
                .expect("published coordinate belongs to paired immutable prefix")
        };
        let directive = match entry.kind {
            DirectiveKind::Defined(p) => SourceDirective::Defined(predicate(p)),
            DirectiveKind::ShowSignature(p) => SourceDirective::ShowSignature(predicate(p)),
            DirectiveKind::ShowEmpty => SourceDirective::ShowEmpty,
            DirectiveKind::ShowTerm => SourceDirective::ShowTerm,
            DirectiveKind::ProjectSignature(p) => SourceDirective::ProjectSignature(predicate(p)),
            DirectiveKind::ProjectAtom => SourceDirective::ProjectAtom,
        };
        LocatedDirective {
            directive,
            location: entry.location,
        }
    }
    /// Iterate without cloning any predicate payload.
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = LocatedDirective<'a>> + DoubleEndedIterator {
        self.entries.iter().map(move |entry| self.bind(entry))
    }
}
impl std::fmt::Debug for Directives<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for Directives<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for Directives<'_> {}
/// Original declaration/display evidence and policies sharing one typed vocabulary.
#[derive(Clone, Debug, Default)]
pub struct SourceMetadata {
    vocabulary: Option<std::sync::Arc<MetadataVocabulary>>,
    directives: Vec<Directive>,
    output: OutputSelection,
    observations: crate::observation::ObservationProgram,
    projection: ProjectSelection,
}
impl PartialEq for SourceMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.directives() == other.directives()
            && self.output == other.output
            && self.observations == other.observations
            && self.projection == other.projection
    }
}
impl Eq for SourceMetadata {}
/// Unpublished source metadata; only coordinates are retained alongside topology.
#[derive(Default)]
pub(crate) struct Builder {
    authority: Option<Admission>,
    storage: MetadataStorageLimits,
    directives: Vec<Directive>,
    output: selection::Builder,
    observations: Vec<crate::observation::Directive>,
    projection: projection::Builder,
}
impl SourceMetadata {
    /// Every original occurrence in source identity/span order, including duplicates.
    ///
    /// # Panics
    /// Panics if compiled components no longer belong to their paired immutable
    /// vocabulary, which violates the metadata's internal publication invariant.
    #[must_use]
    pub fn directives(&self) -> Directives<'_> {
        Directives {
            read: self.vocabulary.as_ref().map(|v| {
                v.read_with(|| Ok::<_, std::convert::Infallible>(()))
                    .expect("published immutable component prefix")
            }),
            entries: &self.directives,
        }
    }
    /// Atom-channel presentation policy, independent of logical semantics.
    #[must_use]
    pub fn output(&self) -> &OutputSelection {
        &self.output
    }
    /// Atom-channel policy; term observations remain separate.
    #[must_use]
    pub fn atom_selection(&self) -> &AtomSelection {
        &self.output
    }
    /// Source-free atom policy with no provenance or observations.
    #[must_use]
    pub fn for_atoms(output: AtomSelection) -> Self {
        Self {
            output,
            ..Self::default()
        }
    }
    /// Bounded term queries meaningful for any supplied full model.
    #[must_use]
    pub fn observations(&self) -> &crate::observation::ObservationProgram {
        &self.observations
    }
    /// Authored projection policy, independent of full answer identity.
    #[must_use]
    pub fn project_selection(&self) -> &ProjectSelection {
        &self.projection
    }
    pub(crate) fn into_observations(self) -> crate::observation::ObservationProgram {
        self.observations
    }
}
impl Builder {
    pub(crate) fn new(storage: MetadataStorageLimits) -> Self {
        Self {
            storage,
            ..Self::default()
        }
    }
    fn admission(&mut self) -> Result<&mut Admission, MetadataStorageError> {
        if self.authority.is_none() {
            self.authority = Some(Admission::new(self.storage, 0)?);
        }
        Ok(self.authority.as_mut().expect("authority admitted above"))
    }
    fn signature(
        &mut self,
        signature: &Signature,
        location: ProgramSite,
    ) -> Result<Predicate, AdmissionFailure> {
        let predicate = predicate(signature, location)?;
        self.admission()
            .and_then(|admission| admission.predicate((&predicate).into(), 0))
            .map_err(|error| AdmissionFailure::Metadata { error, location })
    }
    pub(crate) fn compile_observations(
        &mut self,
        source: &SourceProgram,
        options: crate::formula_ir::CompilationOptions,
        limits: crate::observation::AdmissionLimits,
        budget: &mut crate::expansion::Budget,
        location: ProgramSite,
    ) -> Result<(), crate::FormulaFailure> {
        if !crate::observation::compile::has_observations(source) {
            return Ok(());
        }
        let admission = self
            .admission()
            .map_err(|error| AdmissionFailure::Metadata { error, location })?;
        let observations =
            crate::observation::compile(source, admission, options, limits, budget, location)?;
        self.observations = observations;
        Ok(())
    }
    pub(crate) fn finish(
        mut self,
        location: ProgramSite,
    ) -> Result<SourceMetadata, AdmissionFailure> {
        self.directives
            .sort_by_key(|entry| (entry.location.source, entry.location.span));
        let vocabulary = self
            .authority
            .map(|authority| authority.finish(0))
            .transpose()
            .map_err(|error| AdmissionFailure::Metadata { error, location })?;
        let observations = if let Some(owner) = vocabulary.as_ref() {
            crate::observation::ObservationProgram::publish(owner.clone(), self.observations)
        } else {
            assert!(self.observations.is_empty());
            crate::observation::ObservationProgram::default()
        };
        Ok(SourceMetadata {
            output: self.output.finish(vocabulary.clone()),
            projection: self.projection.finish(vocabulary.clone()),
            observations,
            directives: self.directives,
            vocabulary,
        })
    }
}

pub(crate) fn check_syntax(
    statement: &ast::Statement,
    parsed: &Parse<ast::Program>,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    let location = parsed.location(statement.syntax().text_range());
    match statement {
        ast::Statement::Project(_) if !formula => {
            return Err(unsupported(ProfileFeature::Statement, location));
        }
        ast::Statement::Defined(_) | ast::Statement::Project(_) => {}
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
            ast::Statement::Defined(_) | ast::Statement::Show(_) | ast::Statement::Project(_)
        ) {
            let location = parsed.location(statement.syntax().text_range());
            let observed = (*used as u128) + 1;
            check(
                ExpansionResource::MetadataStatements,
                observed,
                limits.max_metadata_statements,
                location.into(),
            )?;
            *used += 1;
        }
    }
    Ok(())
}

/// Bound canonical metadata and any retained parsed occurrences before collection.
/// Source admission additionally counts the original syntax before raising.
pub(crate) fn check_program_count(
    program: &SourceProgram,
    limits: ExpansionLimits,
) -> Result<(), ExpansionFailure> {
    let mut used = 0_u128;
    for (index, carrier) in program.statements().enumerate() {
        if !matches!(
            carrier.get(),
            Statement::Defined(_) | Statement::Show(_) | Statement::Project(_)
        ) {
            continue;
        }
        let site = crate::extended::origin(
            carrier,
            ProgramSite::statement(StatementId::new(index), None),
        );
        let parsed = carrier
            .provenance()
            .origins()
            .filter(|origin| matches!(origin, Origin::Parsed(_)))
            .count();
        used = used.saturating_add(parsed.max(1) as u128);
        check(
            ExpansionResource::MetadataStatements,
            used,
            limits.max_metadata_statements,
            site,
        )?;
    }
    Ok(())
}

pub(crate) fn collect(
    program: &SourceProgram,
    metadata: &mut Builder,
) -> Result<(), AdmissionFailure> {
    collect_profile(program, metadata, false)
}

pub(crate) fn collect_profile(
    program: &SourceProgram,
    metadata: &mut Builder,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    collect_profile_at(program, metadata, formula, ProgramSite::program())
}

pub(crate) fn collect_profile_at(
    program: &SourceProgram,
    metadata: &mut Builder,
    formula: bool,
    fallback: ProgramSite,
) -> Result<(), AdmissionFailure> {
    collect_carriers(
        program
            .statements()
            .enumerate()
            .map(|(index, carrier)| (carrier, fallback.with_statement(StatementId::new(index)))),
        metadata,
        formula,
    )
}

/// Original declarations precede occurrence-copy admission. The upstream stream
/// retains duplicates and locations; `finish` establishes the public location
/// order. Formula metadata accepts every raised Show shape, and a raised
/// signature's Name is nonempty, so changing collection order adds no diagnostic
/// precedence between otherwise valid source declarations.
pub(crate) fn collect_occurrences(
    occurrences: &Occurrences,
    metadata: &mut Builder,
) -> Result<(), AdmissionFailure> {
    collect_carriers(
        occurrences
            .occurrences()
            .iter()
            .map(|occurrence| (occurrence.statement(), ProgramSite::program())),
        metadata,
        true,
    )
}

fn collect_carriers<'a>(
    carriers: impl Iterator<Item = (&'a WithProvenance<Statement>, ProgramSite)>,
    metadata: &mut Builder,
    formula: bool,
) -> Result<(), AdmissionFailure> {
    for (carrier, site) in carriers {
        let site = crate::extended::origin(carrier, site);
        let directive = match carrier.get() {
            Statement::Defined(defined) => {
                DirectiveKind::Defined(metadata.signature(&defined.signature, site)?)
            }
            Statement::Show(Show::Signature(signature)) => {
                DirectiveKind::ShowSignature(metadata.signature(signature, site)?)
            }
            // themelios names its faithful `#show.` value `Show::All`;
            // clingo interprets the empty directive as show nothing.
            Statement::Show(Show::All) => DirectiveKind::ShowEmpty,
            Statement::Project(Project::Signature(signature)) => {
                DirectiveKind::ProjectSignature(metadata.signature(signature, site)?)
            }
            Statement::Project(Project::Atom { .. }) => DirectiveKind::ProjectAtom,
            Statement::Show(_) if formula => DirectiveKind::ShowTerm,
            Statement::Show(_) => return Err(unsupported(ProfileFeature::ShowTerm, site)),
            _ => continue,
        };
        // Effective policy is logical. Parsed evidence is optional and never
        // controls whether a constructed declaration takes effect.
        match directive {
            DirectiveKind::Defined(_) | DirectiveKind::ShowTerm => {}
            DirectiveKind::ShowSignature(signature) => metadata.output.include(signature),
            DirectiveKind::ShowEmpty => metadata.output.mark_explicit(),
            DirectiveKind::ProjectSignature(signature) => metadata.projection.signature(signature),
            DirectiveKind::ProjectAtom => metadata.projection.atom(),
        }
        for origin in carrier.provenance().origins() {
            if let Origin::Parsed(location) = origin {
                metadata.directives.push(Directive {
                    kind: directive,
                    location: *location,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn predicate(
    signature: &Signature,
    location: ProgramSite,
) -> Result<OwnedPredicate, AdmissionFailure> {
    let arity = usize::try_from(signature.arity)
        .expect("supported Rust targets represent u32 arities in usize");
    OwnedPredicate::with_sign(
        signature.name.as_str(),
        arity,
        crate::coherence::core_sign(signature.sign),
    )
    .map_err(|error| AdmissionFailure::Construction { error, location })
}
