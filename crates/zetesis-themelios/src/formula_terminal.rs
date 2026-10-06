//! Terminal positive definitions are evaluated over each verified base answer.
//!
//! The partition certificate excludes every semantic read of a deferred head.
//! Its one-layer positive extension is therefore unique. Canonical storage
//! presence remains distinct from truth: reconstruction reads selected model
//! rows, never the completed possible-support relation.

mod materialize;
mod partition;
mod reconstruct;
mod storage;
pub(crate) use materialize::{Core, Materialized, materialize, materialize_lazy};

use std::{fmt, sync::Arc};
use themelios_base::source::Source;
use themelios_program::program::Program;
use zetesis_core::AtomCatalog;
use zetesis_ferraris::Theory;

use crate::formula::Compiled;
use crate::formula_ir::RuleIr;
use crate::formula_owner::Owner;
use crate::formula_support::{AccountingBaseline, ClosedSource};
use crate::{AnalysisBasis, FormulaLimits, ProgramSite, SourceBundle, SourceMetadata};

pub use reconstruct::{
    ReconstructionCharges, ReconstructionError, ReconstructionStatistics, TerminalReconstruction,
};

/// Result of source materialization with terminal-definition analysis enabled.
/// A terminal plan still requires base solving and answer reconstruction.
#[derive(Debug)]
pub enum FormulaMaterialization<T> {
    /// The complete source theory was materialized.
    Complete(T),
    /// A certified base theory and its terminal positive definitions.
    Terminal(TerminalFormula),
}

pub(crate) struct Extension {
    original: partition::OriginalAnalysis,
    deferred: Vec<RuleIr>,
    /// Shared with a hybrid base's streamed support, when there is one.
    closed: Arc<ClosedSource>,
    baseline: AccountingBaseline,
    limits: FormulaLimits,
    location: ProgramSite,
    // Named noncanonical retained admission storage. Source AST/provenance
    // follows the existing bounded source-copy policy, not a whole-heap claim.
    metadata_bytes: u128,
}

/// How a terminal owner's base was grounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseKind {
    /// Every base rule was instantiated.
    Eager,
    /// The base's producer core was instantiated and its eligible integrity
    /// constraints (possibly none) are streamed: its stable models are
    /// proposals until the constraints accept them.
    Hybrid,
}

/// A terminal owner's base, by kind.
#[derive(Clone, Copy, Debug)]
pub enum TerminalBase<'a> {
    /// The base theory, whose stable models are the base's answer sets.
    Eager(&'a Theory),
    /// The streamed core; only its checked stable models are base answers.
    Hybrid(&'a crate::StreamedCore),
}

enum Base {
    Eager(Box<Compiled>),
    Hybrid(crate::StreamedCore),
}

impl Base {
    fn compiled(&self) -> &Compiled {
        match self {
            Self::Eager(compiled) => compiled,
            Self::Hybrid(core) => core.compiled(),
        }
    }
}

struct Admitted {
    base: Base,
    extension: Extension,
    source: Arc<Owner>,
    metadata: SourceMetadata,
}

/// One original source owner with a certified terminal-definition partition.
///
/// Its base theory alone does not denote the original answer sets. Each base
/// answer must be extended by [`Self::reconstruction`]. Clones share immutable
/// source, canonical payload and indexes; each reconstruction session owns its
/// cumulative work history and private scratch. Objectives and explicit
/// projection are outside this initial applicability class.
#[derive(Clone)]
pub struct TerminalFormula(Arc<Admitted>);

impl fmt::Debug for TerminalFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalFormula")
            .field("base_kind", &self.base_kind())
            .field("deferred_templates", &self.deferred_templates())
            .finish_non_exhaustive()
    }
}

impl TerminalFormula {
    pub(crate) fn new(
        compiled: Compiled,
        core: Core,
        extension: Extension,
        source: Owner,
        metadata: SourceMetadata,
    ) -> Self {
        let source = Arc::new(source);
        let base = match core {
            Core::Eager => Base::Eager(Box::new(compiled)),
            Core::Hybrid(constraints) => Base::Hybrid(crate::StreamedCore::new(
                compiled,
                constraints.map(|constraints| *constraints),
                Arc::clone(&source),
            )),
        };
        Self(Arc::new(Admitted {
            base,
            extension,
            source,
            metadata,
        }))
    }

    /// How the base was grounded.
    #[must_use]
    pub fn base_kind(&self) -> BaseKind {
        match self.0.base {
            Base::Eager(_) => BaseKind::Eager,
            Base::Hybrid(_) => BaseKind::Hybrid,
        }
    }

    /// The base: its theory when eager, its streamed core when hybrid.
    #[must_use]
    pub fn base(&self) -> TerminalBase<'_> {
        match &self.0.base {
            Base::Eager(compiled) => TerminalBase::Eager(&compiled.theory),
            Base::Hybrid(core) => TerminalBase::Hybrid(core),
        }
    }

    /// Exact original-owner identity. Equal source text is insufficient.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The eager base theory, whose verified answers require positive
    /// extension; `None` for a hybrid base, whose core theory's stable models
    /// are only proposals (see [`Self::base`]).
    #[must_use]
    pub fn base_theory(&self) -> Option<&Theory> {
        match &self.0.base {
            Base::Eager(compiled) => Some(&compiled.theory),
            Base::Hybrid(_) => None,
        }
    }

    /// Exact occurrence owner required for supplied base interpretations.
    #[must_use]
    pub fn base_atom_catalog(&self) -> &AtomCatalog {
        &self.0.base.compiled().atoms
    }

    /// Analysis of the filtered base program, suitable only for base planning.
    #[must_use]
    pub fn base_analysis(&self) -> &themelios_analysis::Analysis {
        &self.0.base.compiled().analysis
    }

    /// Semantic status of the base analysis input.
    #[must_use]
    pub fn base_analysis_basis(&self) -> AnalysisBasis {
        self.0.base.compiled().analysis_basis
    }

    /// Analysis of the complete normalized original source.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.0.extension.original.analysis
    }

    /// Complete normalized original program underlying the partition certificate.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.0.extension.original.program
    }

    /// The partition requires an exact normalized program, never a projection.
    #[must_use]
    pub fn analysis_basis(&self) -> AnalysisBasis {
        self.0.extension.original.basis
    }

    /// Empty objective program; authored objectives exclude this partition.
    #[must_use]
    pub fn objectives(&self) -> &zetesis_objective::ObjectiveProgram {
        &self.0.base.compiled().objectives
    }

    /// General keyed constraints retained in the base theory.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.0.base.compiled().keyed_constraints
    }

    /// Completion status of the original bounded key analysis.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.0.base.compiled().key_analysis
    }

    /// Original declarations and observation policy, applied after extension.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.0.metadata
    }

    /// Original canonical program before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.0.source.program()
    }

    /// Original single source; absent for an include bundle or logical input.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        self.0.source.source()
    }

    /// Complete include bundle; absent for a single source or logical input.
    #[must_use]
    pub fn bundle(&self) -> Option<&SourceBundle> {
        self.0.source.source_bundle()
    }

    /// Source preparation charges, including bounded partition construction.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.0.base.compiled().expansion
    }

    /// Number of deferred lowered producer occurrences, including pool copies.
    #[must_use]
    pub fn deferred_templates(&self) -> usize {
        self.0.extension.deferred.len()
    }

    /// Original statement warnings established before answer reconstruction.
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.0.base.compiled().warnings
    }

    /// Render warnings against retained source bytes or an include bundle.
    /// Logical input instead names the original statement index when available.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        self.0.source.warning_view(self.warnings())
    }

    /// Start an independent reconstruction history with admission work already
    /// charged. This does not prove stability of a supplied base interpretation.
    ///
    /// # Errors
    /// Returns an invalid retained component or resource refusal.
    pub fn reconstruction(&self) -> Result<TerminalReconstruction<'_>, ReconstructionError> {
        TerminalReconstruction::new(self)
    }
}
