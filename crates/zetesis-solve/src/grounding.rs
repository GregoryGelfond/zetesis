//! The grounder policy for formula programs: one materialization per request.
//!
//! `Eager` instantiates every rule before solving. `Lazy` streams eligible
//! integrity constraints over an instantiated producer core and defers
//! certified terminal definitions to per-answer reconstruction. `Auto`
//! instantiates an eager base and defers certified terminal definitions. The
//! choice of relational source joins for programs in the relational profile is
//! made before formula admission and is not part of this mapping.

use zetesis_themelios::{
    AdmittedFormula, AdmittedFormulaBundle, FormulaBundleFailure, FormulaFailure,
    FormulaMaterialization, GroundingObserver, HybridFormula, PreparedFormula,
    PreparedFormulaBundle, TerminalFormula,
};

use crate::{Grounder, PreparedInput};

/// A formula program materialized for one requested grounder.
#[derive(Debug)]
pub enum GroundedFormula {
    /// Every rule of a single source or logical program instantiated.
    Source(AdmittedFormula),
    /// Every rule of an include bundle instantiated.
    Bundle(AdmittedFormulaBundle),
    /// A producer core with streamed constraints; nothing deferred.
    Hybrid(HybridFormula),
    /// A base, eager or hybrid, with deferred terminal definitions.
    Terminal(TerminalFormula),
}

impl GroundedFormula {
    /// Borrow the materialization as a session input.
    #[must_use]
    pub fn input(&self) -> PreparedInput<'_> {
        match self {
            Self::Source(owner) => PreparedInput::formula(owner),
            Self::Bundle(owner) => PreparedInput::formula_bundle(owner),
            Self::Hybrid(owner) => PreparedInput::hybrid(owner),
            Self::Terminal(owner) => PreparedInput::terminal(owner),
        }
    }

    /// The original declarations and observation policy.
    #[must_use]
    pub fn metadata(&self) -> &zetesis_themelios::SourceMetadata {
        match self {
            Self::Source(owner) => owner.metadata(),
            Self::Bundle(owner) => owner.metadata(),
            Self::Hybrid(owner) => owner.metadata(),
            Self::Terminal(owner) => owner.metadata(),
        }
    }
}

/// Materialize a prepared single-source or logical program for `grounder`.
///
/// # Errors
/// Returns the refusal of the materialization the grounder selects; see
/// `PreparedFormula::ground`, `ground_lazy` and `ground_adaptive`.
pub fn ground_formula(
    prepared: PreparedFormula,
    grounder: Grounder,
    observer: Option<&dyn GroundingObserver>,
) -> Result<GroundedFormula, FormulaFailure> {
    match grounder {
        Grounder::Eager => prepared
            .ground_with_observer(observer)
            .map(GroundedFormula::Source),
        Grounder::Lazy => prepared
            .ground_lazy_with_observer(observer)
            .map(|materialized| match materialized {
                FormulaMaterialization::Complete(owner) => GroundedFormula::Hybrid(owner),
                FormulaMaterialization::Terminal(owner) => GroundedFormula::Terminal(owner),
            }),
        Grounder::Auto => prepared
            .ground_adaptive_with_observer(observer)
            .map(|materialized| match materialized {
                FormulaMaterialization::Complete(owner) => GroundedFormula::Source(owner),
                FormulaMaterialization::Terminal(owner) => GroundedFormula::Terminal(owner),
            }),
    }
}

/// Materialize a prepared include bundle for `grounder`.
///
/// # Errors
/// Returns the refusal of the materialization the grounder selects, with the
/// original bundle.
pub fn ground_bundle(
    prepared: PreparedFormulaBundle,
    grounder: Grounder,
    observer: Option<&dyn GroundingObserver>,
) -> Result<GroundedFormula, FormulaBundleFailure> {
    match grounder {
        Grounder::Eager => prepared
            .ground_with_observer(observer)
            .map(GroundedFormula::Bundle),
        Grounder::Lazy => prepared
            .ground_lazy_with_observer(observer)
            .map(|materialized| match materialized {
                FormulaMaterialization::Complete(owner) => GroundedFormula::Hybrid(owner),
                FormulaMaterialization::Terminal(owner) => GroundedFormula::Terminal(owner),
            }),
        Grounder::Auto => prepared
            .ground_adaptive_with_observer(observer)
            .map(|materialized| match materialized {
                FormulaMaterialization::Complete(owner) => GroundedFormula::Bundle(owner),
                FormulaMaterialization::Terminal(owner) => GroundedFormula::Terminal(owner),
            }),
    }
}
