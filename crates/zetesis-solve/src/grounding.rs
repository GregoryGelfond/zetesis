//! Shared source-evaluation and materialization policy for ordinary solves.
//!
//! `Eager` instantiates every rule before solving. `Lazy` streams eligible
//! integrity constraints over an instantiated producer core and defers
//! certified terminal definitions to per-answer reconstruction. `Auto`
//! first evaluates a certified acyclic normal source directly on the CPU;
//! otherwise it instantiates an eager base and defers certified terminal definitions. The
//! choice of relational source joins for programs in the relational profile is
//! made before formula admission and is not part of this mapping.

use zetesis_themelios::{
    AdmittedFormula, AdmittedFormulaBundle, FormulaBundleFailure, FormulaFailure,
    FormulaMaterialization, GroundingObserver, HybridFormula, PreparedFormula,
    PreparedFormulaBundle, TerminalFormula,
};

use crate::{Grounder, PreparedInput, SolveConfig};

/// A complete semantic input selected for an ordinary solve. The source variant
/// retains the original program and its checked interpretation; it does not
/// manufacture a formula theory containing only the computed answer.
#[derive(Debug)]
pub enum PreparedExecution {
    /// Exact CPU source evaluation without producer formula construction.
    Source(zetesis_themelios::AcyclicSource),
    /// The materialization selected by an explicit or inapplicable source policy.
    Grounded(Box<GroundedFormula>),
}

impl PreparedExecution {
    /// Borrow the coherent owner for an ordinary session.
    #[must_use]
    pub fn input(&self) -> PreparedInput<'_> {
        match self {
            Self::Source(owner) => PreparedInput::source(owner),
            Self::Grounded(owner) => owner.input(),
        }
    }

    /// Original declarations and observation policy.
    #[must_use]
    pub fn metadata(&self) -> &zetesis_themelios::SourceMetadata {
        match self {
            Self::Source(owner) => owner.metadata(),
            Self::Grounded(owner) => owner.metadata(),
        }
    }
}

pub(crate) fn source_compatible(config: &SolveConfig) -> bool {
    config.backend == crate::Backend::Cpu
        && config.oracle == crate::Oracle::Auto
        && config.grounder == Grounder::Auto
        && config.source_batching == crate::SourceBatching::Independent
}

/// Prepare one source or typed program for an ordinary solve. Automatic CPU
/// execution first attempts complete signed-acyclic source certification.
/// Explicit grounding, device and oracle requests retain their materialization
/// policy. A class mismatch resumes the same preparation with its work retained.
///
/// # Errors
/// Returns the selected admission failure. Cancellation, exhausted allowances
/// and invalid source never cause a fresh attempt under another policy.
pub fn prepare_formula_execution(
    prepared: PreparedFormula,
    config: &SolveConfig,
    observer: Option<&dyn GroundingObserver>,
) -> Result<PreparedExecution, FormulaFailure> {
    let prepared = if source_compatible(config) {
        match prepared.try_source_with_observer(observer)? {
            zetesis_themelios::SourceAdmission::Admitted(owner) => {
                return Ok(PreparedExecution::Source(owner));
            }
            zetesis_themelios::SourceAdmission::NotApplicable(prepared) => prepared,
        }
    } else {
        prepared
    };
    ground_formula(prepared, config.grounder, observer)
        .map(|owner| PreparedExecution::Grounded(Box::new(owner)))
}

/// Prepare an original include bundle with the same policy as a single input.
/// No source text is rendered or reparsed at this boundary.
///
/// # Errors
/// Returns the selected admission failure with the original source bundle.
pub fn prepare_bundle_execution(
    prepared: PreparedFormulaBundle,
    config: &SolveConfig,
    observer: Option<&dyn GroundingObserver>,
) -> Result<PreparedExecution, FormulaBundleFailure> {
    let prepared = if source_compatible(config) {
        match prepared.try_source_with_observer(observer)? {
            zetesis_themelios::SourceAdmission::Admitted(owner) => {
                return Ok(PreparedExecution::Source(owner));
            }
            zetesis_themelios::SourceAdmission::NotApplicable(prepared) => prepared,
        }
    } else {
        prepared
    };
    ground_bundle(prepared, config.grounder, observer)
        .map(|owner| PreparedExecution::Grounded(Box::new(owner)))
}

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
