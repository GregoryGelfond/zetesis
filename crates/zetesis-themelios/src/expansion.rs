//! Typed, independently bounded opt-in source expansion.

use std::fmt;

use themelios_base::diagnostic::Diagnostic;
use themelios_base::span::Location;
use themelios_program::term::EvalError;

use crate::AdmissionFailure;

/// Ceilings for the opt-in source expansion door. Zero never means unlimited.
/// These bound source normalization, independently of candidate enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpansionLimits {
    /// Maximum distinct, unannotated constant definitions.
    pub max_constants: usize,
    /// Maximum dependency/pool scans, bounded source copies and bottom-up term
    /// evaluation steps, affine normalization and endpoint scans combined.
    pub max_term_work: usize,
    /// Maximum output templates, also capped by core admission's template limit.
    pub max_templates: usize,
    /// Maximum intermediate scalar alternatives, emitted fact arguments, and
    /// conservatively counted nodes in finite-pool source alternatives and
    /// compiled structural patterns. Original Boolean choice rules additionally
    /// reserve selected syntax nodes before their independent raising.
    pub max_values: usize,
    /// Maximum scalar payload bytes copied during substitution/fact emission
    /// plus finite-pool cursor positions, copied term cells/text, constructor-plan
    /// storage and constructed value node/spelling/frame reservations. Positive
    /// structural patterns also charge plan/cursor cells, constructor and slot
    /// names, deltas and conservative extracted-value construction payload
    /// before allocation.
    /// Evaluated positive positions reserve copied term cells/text, flat
    /// operations, check instructions and initial distinct required-input slots.
    /// Conditional alternatives additionally charge scoped variable payload,
    /// including required-input copies, binding vectors and source atom/body-element
    /// carriers. Aggregate consumer plans reserve instruction cells, required
    /// outer slots, readiness/producers and temporary scheduling cells.
    /// Ordinary Boolean choice keys additionally reserve their scalar identity payload.
    /// Finite affine binding analysis reserves expression/coefficient frames,
    /// inequalities and endpoint arrays before allocation.
    /// Ordinary aggregate, choice and conditional scope clones remain
    /// excluded, as do other AST carriers, provenance and allocator overhead.
    /// Original source storage remains bounded by admission options.
    pub max_scalar_bytes: usize,
    /// Maximum original locations reserved for template evidence and independently
    /// raised Boolean choice rules, including their nested occurrence evidence.
    pub max_origin_locations: usize,
    /// Maximum original `#defined` and `#show` occurrences before deduplication.
    pub max_metadata_statements: usize,
}

impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_constants: 1_024,
            max_term_work: 1_048_576,
            max_templates: 100_000,
            max_values: 1_000_000,
            max_scalar_bytes: 16_777_216,
            max_origin_locations: 1_000_000,
            max_metadata_statements: 1_024,
        }
    }
}

/// A resource consumed by source expansion, never a semantic UNSAT verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpansionResource {
    /// Constant definitions before canonicalization can merge them.
    Constants,
    /// Dependency/pool scans, bounded source copies and term-fold steps.
    TermWork,
    /// Emitted core templates.
    Templates,
    /// Intermediate alternatives, emitted fact arguments, finite-pool source nodes
    /// and independently raised Boolean choice syntax nodes.
    Values,
    /// Copied scalar payload, finite-pool positions, term/plan storage and
    /// constructed/extracted value reservations, including structural pattern deltas.
    ScalarBytes,
    /// Original locations in emitted templates and Boolean choice occurrence evidence.
    Origins,
    /// Original declaration/display occurrences, counted before canonicalization.
    MetadataStatements,
}

/// An opt-in admission refusal. Undefined arithmetic and overflow refuse the
/// whole input; this partial profile does not emulate clingo's warning-and-drop
/// behavior for undefined instances.
#[derive(Debug)]
pub enum ExpansionFailure {
    /// The existing source or core boundary refused its stage.
    Admission(AdmissionFailure),
    /// A checked source-expansion ceiling was exceeded.
    Limit {
        /// Resource being counted.
        resource: ExpansionResource,
        /// Inclusive configured ceiling.
        limit: u128,
        /// Proposed cumulative count, before the allocation or expansion.
        observed: u128,
        /// Original source occurrence associated with the work.
        location: Location,
    },
    /// Multiple source definitions are ambiguous in this deliberately narrow
    /// profile, including textually identical repeated definitions.
    DuplicateConstant {
        /// Constant name.
        name: String,
        /// First original definition.
        first: Location,
        /// Repeated original definition.
        duplicate: Location,
    },
    /// Annotated default/override policies need a broader configuration model.
    ConstantPolicy {
        /// Original definition.
        location: Location,
    },
    /// Constant definitions contain a dependency cycle.
    ConstantCycle {
        /// Active dependency chain followed by its repeated endpoint.
        names: Vec<String>,
        /// Definition that closes the cycle.
        location: Location,
    },
    /// A scalar operation is non-ground, undefined, or outside checked i32.
    Evaluation {
        /// Dependency's checked evaluator refusal.
        error: EvalError,
        /// Original statement containing the expression.
        location: Location,
    },
}

impl From<AdmissionFailure> for ExpansionFailure {
    fn from(error: AdmissionFailure) -> Self {
        Self::Admission(error)
    }
}

impl ExpansionFailure {
    /// Whether another source profile may handle this refusal. This permits
    /// automatic formula admission after an unsupported profile construct or
    /// scalar evaluation stopped at a variable. Syntax, arithmetic undefinedness,
    /// overflow, source/expansion limits, and core failures must not be retried
    /// as if a more permissive mode had resolved them.
    #[must_use]
    pub fn needs_formula_admission(&self) -> bool {
        matches!(
            self,
            Self::Admission(AdmissionFailure::Profile { .. })
                | Self::Evaluation {
                    error: themelios_program::term::EvalError::NotGround { .. },
                    ..
                }
        )
    }

    /// Located diagnostic rendering without discarding the typed refusal.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let location = match self {
            Self::Admission(error) => return error.diagnostics(),
            Self::Limit { location, .. }
            | Self::ConstantPolicy { location }
            | Self::ConstantCycle { location, .. }
            | Self::Evaluation { location, .. } => *location,
            Self::DuplicateConstant { duplicate, .. } => *duplicate,
        };
        vec![crate::diagnostic::diagnostic(
            "source-expansion",
            self.to_string(),
            location,
        )]
    }
}

impl fmt::Display for ExpansionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(f),
            Self::Limit {
                resource,
                limit,
                observed,
                ..
            } => write!(
                f,
                "source expansion {resource:?} count {observed} exceeds {limit}"
            ),
            Self::DuplicateConstant { name, .. } => write!(
                f,
                "source expansion requires one definition of constant {name}"
            ),
            Self::ConstantPolicy { .. } => {
                f.write_str("source expansion admits only unannotated constant definitions")
            }
            Self::ConstantCycle { names, .. } => {
                write!(f, "constant dependency cycle: {}", names.join(" -> "))
            }
            Self::Evaluation { error, .. } => {
                write!(f, "source scalar evaluation refused: {error}")
            }
        }
    }
}

impl std::error::Error for ExpansionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Evaluation { error, .. } => Some(error),
            _ => None,
        }
    }
}

pub(crate) struct Budget {
    limits: ExpansionLimits,
    work: u128,
    templates: u128,
    values: u128,
    scalar_bytes: u128,
    origins: u128,
}

impl Budget {
    pub(crate) fn new(mut limits: ExpansionLimits, core_templates: usize) -> Self {
        limits.max_templates = limits.max_templates.min(core_templates);
        Self {
            limits,
            work: 0,
            templates: 0,
            values: 0,
            scalar_bytes: 0,
            origins: 0,
        }
    }

    pub(crate) fn charge(
        &mut self,
        resource: ExpansionResource,
        amount: u128,
        location: Location,
    ) -> Result<(), ExpansionFailure> {
        let (used, ceiling) = match resource {
            ExpansionResource::TermWork => (&mut self.work, self.limits.max_term_work),
            ExpansionResource::Templates => (&mut self.templates, self.limits.max_templates),
            ExpansionResource::Values => (&mut self.values, self.limits.max_values),
            ExpansionResource::ScalarBytes => {
                (&mut self.scalar_bytes, self.limits.max_scalar_bytes)
            }
            ExpansionResource::Origins => (&mut self.origins, self.limits.max_origin_locations),
            ExpansionResource::Constants | ExpansionResource::MetadataStatements => {
                unreachable!("source occurrence counts are checked directly")
            }
        };
        let observed = used.saturating_add(amount);
        check(resource, observed, ceiling, location)?;
        *used = observed;
        Ok(())
    }
}

pub(crate) fn check(
    resource: ExpansionResource,
    observed: u128,
    ceiling: usize,
    location: Location,
) -> Result<(), ExpansionFailure> {
    if observed > ceiling as u128 {
        Err(ExpansionFailure::Limit {
            resource,
            limit: ceiling as u128,
            observed,
            location,
        })
    } else {
        Ok(())
    }
}
