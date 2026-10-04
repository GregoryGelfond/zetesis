//! Typed, independently bounded opt-in source expansion.

use std::fmt;

use crate::ProgramSite;
use themelios_base::diagnostic::Diagnostic;
use themelios_program::term::EvalError;

use crate::AdmissionFailure;

/// Ceilings for checked source and logical-program expansion. Zero never means
/// unlimited. These bound normalization, independently of candidate enumeration.
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
    /// compiled structural patterns.
    pub max_values: usize,
    /// Maximum scalar payload bytes copied during substitution/fact emission
    /// plus finite-pool cursor positions, copied term cells/text, constructor-plan
    /// storage and constructed value node/spelling/frame reservations. Positive
    /// structural patterns also charge plan/cursor cells, constructor and slot
    /// names, and requested capture-delta cells before allocation. Captures
    /// borrow canonical terms; the delta does not copy captured payload.
    /// Evaluated positive positions reserve copied term cells/text, flat
    /// operations, check instructions and initial distinct required-input slots.
    /// Conditional alternatives additionally charge scoped variable payload,
    /// including required-input copies, binding vectors and source atom/body-element
    /// carriers. Aggregate consumer plans reserve instruction cells, required
    /// outer slots, readiness/producers and temporary scheduling cells.
    /// Ordinary Boolean choice keys additionally reserve their scalar identity payload.
    /// Finite affine binding analysis reserves expression/coefficient frames,
    /// inequalities and endpoint arrays before allocation.
    /// Analyzed-statement ownership additionally reserves statement-ID slots and
    /// named lookup-map payload; it does not copy original statement content.
    /// Ordinary aggregate, choice and conditional scope clones remain
    /// excluded, as do other AST carriers, provenance and allocator overhead.
    /// Original input structure remains bounded separately by admission options.
    pub max_scalar_bytes: usize,
    /// Maximum retained origin-evidence entries. Formula evidence carries a
    /// statement identity and an optional real location; source-only relational
    /// evidence retains its parsed locations.
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

/// Charges accumulated by one admission under [`ExpansionLimits`], in the
/// units of the ceiling each names. Every count is at most its ceiling; a
/// refused admission reports no usage. Plain S0 admission expands nothing
/// and reports zero throughout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExpansionUsage {
    /// Charged term work, under `max_term_work`.
    pub term_work: usize,
    /// Templates charged before core canonicalization, under `max_templates`.
    pub templates: usize,
    /// Charged values, under `max_values`.
    pub values: usize,
    /// Charged scalar payload bytes, under `max_scalar_bytes`.
    pub scalar_bytes: usize,
    /// Charged origin-evidence entries, under `max_origin_locations`.
    pub origin_locations: usize,
}

/// A resource consumed by source expansion, never a semantic UNSAT verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpansionResource {
    /// Constant declarations; source admission also checks authored repetitions.
    Constants,
    /// Dependency/pool scans, bounded source copies and term-fold steps.
    TermWork,
    /// Emitted core templates.
    Templates,
    /// Intermediate alternatives, emitted fact arguments and finite-pool nodes.
    Values,
    /// Copied scalar payload, finite-pool positions, term/plan storage and
    /// constructed-value reservations and borrowed structural-capture delta cells.
    ScalarBytes,
    /// Retained origin-evidence entries in templates and expanded alternatives.
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
        location: ProgramSite,
    },
    /// Multiple declarations define the same constant. Source admission rejects
    /// even textually identical repetitions before canonicalization; canonical
    /// input counts distinct declarations, independently of their provenance.
    DuplicateConstant {
        /// Constant name.
        name: String,
        /// First original definition.
        first: ProgramSite,
        /// Repeated original definition.
        duplicate: ProgramSite,
    },
    /// Annotated default/override policies need a broader configuration model.
    ConstantPolicy {
        /// Original definition.
        location: ProgramSite,
    },
    /// Constant definitions contain a dependency cycle.
    ConstantCycle {
        /// Active dependency chain followed by its repeated endpoint.
        names: Vec<String>,
        /// Definition that closes the cycle.
        location: ProgramSite,
    },
    /// A scalar operation is non-ground, undefined, or outside checked i32.
    Evaluation {
        /// Dependency's checked evaluator refusal.
        error: EvalError,
        /// Original statement containing the expression.
        location: ProgramSite,
    },
}

impl From<AdmissionFailure> for ExpansionFailure {
    fn from(error: AdmissionFailure) -> Self {
        Self::Admission(error)
    }
}

impl ExpansionFailure {
    /// The logical subject and optional real source evidence of this refusal.
    /// Duplicate definitions identify the repeated declaration here; both sites
    /// remain available in the typed variant.
    #[must_use]
    pub fn site(&self) -> Option<ProgramSite> {
        match self {
            Self::Admission(error) => error.site(),
            Self::Limit { location, .. }
            | Self::ConstantPolicy { location }
            | Self::ConstantCycle { location, .. }
            | Self::Evaluation { location, .. } => Some(*location),
            Self::DuplicateConstant { duplicate, .. } => Some(*duplicate),
        }
    }
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

    /// Render diagnostics for actual source coordinates, without discarding the
    /// typed refusal or inventing a span for a logical statement.
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
        location
            .location()
            .map(|location| {
                crate::diagnostic::diagnostic("source-expansion", self.to_string(), location)
            })
            .into_iter()
            .collect()
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
                f.write_str("constant dependency cycle: ")?;
                for (index, name) in names.iter().enumerate() {
                    if index != 0 {
                        f.write_str(" -> ")?;
                    }
                    f.write_str(name)?;
                }
                Ok(())
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

/// A clone is a tentative budget: charges made on it either become the
/// budget as a whole or are discarded with it, never both.
/// Runtime constraint checkers can additionally attach a shared cumulative
/// scalar allowance. Those accepted charges are never rolled back or refunded;
/// tentative source admission does not attach such an allowance.
#[derive(Clone)]
pub(crate) struct Budget {
    limits: ExpansionLimits,
    work: u128,
    templates: u128,
    values: u128,
    scalar_bytes: u128,
    origins: u128,
    allowance: Option<crate::ConstraintAllowance>,
}

impl Budget {
    pub(crate) fn check_constants(
        &self,
        count: usize,
        site: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        check(
            ExpansionResource::Constants,
            count as u128,
            self.limits.max_constants,
            site,
        )
    }
    pub(crate) fn new(mut limits: ExpansionLimits, core_templates: usize) -> Self {
        limits.max_templates = limits.max_templates.min(core_templates);
        Self {
            limits,
            work: 0,
            templates: 0,
            values: 0,
            scalar_bytes: 0,
            origins: 0,
            allowance: None,
        }
    }

    pub(crate) fn with_allowance(mut self, allowance: crate::ConstraintAllowance) -> Self {
        self.allowance = Some(allowance);
        self
    }

    pub(crate) fn charge(
        &mut self,
        resource: ExpansionResource,
        amount: u128,
        location: ProgramSite,
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
        if matches!(resource, ExpansionResource::ScalarBytes)
            && let Some(allowance) = &self.allowance
        {
            allowance.scalar(amount, location)?;
        }
        *used = observed;
        Ok(())
    }

    /// The charges accepted so far. Each is at most its ceiling, so the
    /// narrowing cannot fail.
    /// The term work still allowed under the ceiling, for a reading that
    /// bounds itself by it before it starts.
    pub(crate) fn remaining_term_work(&self) -> u64 {
        u64::try_from((self.limits.max_term_work as u128).saturating_sub(self.work))
            .unwrap_or(u64::MAX)
    }

    pub(crate) fn usage(&self) -> ExpansionUsage {
        let narrow = |charged: u128| {
            usize::try_from(charged).expect("an accepted charge is at most its usize ceiling")
        };
        ExpansionUsage {
            term_work: narrow(self.work),
            templates: narrow(self.templates),
            values: narrow(self.values),
            scalar_bytes: narrow(self.scalar_bytes),
            origin_locations: narrow(self.origins),
        }
    }
}

pub(crate) fn check(
    resource: ExpansionResource,
    observed: u128,
    ceiling: usize,
    location: ProgramSite,
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
