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
    /// names, and capture-delta capacity growth before allocation. Each join
    /// reuses its delta; retained capacity is also counted by formula support
    /// storage. Captures borrow canonical terms without copying their payload.
    /// Evaluated positive positions reserve copied term cells/text, flat
    /// operations, check instructions and initial distinct required-input slots.
    /// Conditional alternatives additionally charge scoped variable payload,
    /// including required-input copies, binding vectors and source atom/body-element
    /// carriers. Aggregate family summaries reserve inherited slot indices once;
    /// consumer plans borrow those reads and reserve scalar/range input slots,
    /// instruction cells, readiness/producers and temporary scheduling cells.
    /// Ordinary Boolean choice keys additionally reserve their scalar identity payload.
    /// Finite affine binding analysis reserves expression/coefficient frames,
    /// inequalities and endpoint arrays before allocation.
    /// Analyzed-statement ownership additionally reserves statement-ID slots and
    /// named lookup-map payload; it does not copy original statement content.
    /// Ordinary aggregate, choice and conditional scope clones remain
    /// excluded, as do other AST carriers, provenance and allocator overhead.
    /// Original input structure remains bounded separately by admission options.
    pub max_scalar_bytes: usize,
    /// Maximum named logical bytes in one materialized source-alternative family.
    /// Pool constructors, local conditions, dependency projections and fact
    /// intervals check their complete term/text/carrier envelope before cloning.
    /// Repeated families do not accumulate this allowance. It is independent of
    /// cumulative `max_values` and `max_scalar_bytes`; other retained program
    /// state, spare allocation capacity, provenance and allocator overhead remain
    /// outside this envelope. The host signed allocation extent is also checked.
    pub max_family_bytes: usize,
    /// Maximum retained origin-evidence entries. Canonical-program evidence carries a
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
            max_family_bytes: 16_777_216,
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
    /// Named logical bytes of one materialized source-alternative family, not traffic.
    FamilyBytes,
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
    /// Shared cancellation or deadline observed before the next expansion step.
    Interrupted {
        /// The control signal; separate from configured resource ceilings.
        reason: zetesis_cpu::Stop,
        /// Original statement or whole-program site at the stopped boundary.
        location: ProgramSite,
    },
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
            Self::Interrupted { location, .. }
            | Self::Limit { location, .. }
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
            Self::Interrupted { location, .. }
            | Self::Limit { location, .. }
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
            Self::Interrupted { reason, .. } => reason.fmt(f),
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
            Self::Interrupted { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

/// A clone is a tentative budget: charges made on it either become the
/// budget as a whole or are discarded with it, never both.
/// Runtime constraint checkers can additionally attach a shared cumulative
/// scalar receipt. Charges reported to it are never rolled back or refunded;
/// tentative source admission does not attach one. A clone copies enforcement
/// history, but starts with no pending shared charges: the original owns those.
pub(crate) struct Budget {
    limits: ExpansionLimits,
    work: u128,
    templates: u128,
    values: u128,
    scalar_bytes: u128,
    origins: u128,
    allowance: Option<crate::constraint_allowance::Pending>,
    cancellation: Option<zetesis_cpu::Cancellation>,
}

impl Clone for Budget {
    fn clone(&self) -> Self {
        Self {
            limits: self.limits,
            work: self.work,
            templates: self.templates,
            values: self.values,
            scalar_bytes: self.scalar_bytes,
            origins: self.origins,
            allowance: self
                .allowance
                .as_ref()
                .map(crate::constraint_allowance::Pending::fork),
            cancellation: self.cancellation.clone(),
        }
    }
}

impl Budget {
    /// Publish the operation's scalar prefix when it returns or unwinds, even
    /// if the budget remains alive for a later check.
    pub(crate) fn with_settled_receipt<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let active = ScalarReceipt(self);
        action(active.0)
    }

    pub(crate) fn with_cancellation(
        mut self,
        cancellation: Option<zetesis_cpu::Cancellation>,
    ) -> Self {
        self.set_cancellation(cancellation);
        self
    }

    pub(crate) fn set_cancellation(&mut self, cancellation: Option<zetesis_cpu::Cancellation>) {
        self.cancellation = cancellation;
    }

    pub(crate) fn cancellation(&self) -> Option<&zetesis_cpu::Cancellation> {
        self.cancellation.as_ref()
    }

    pub(crate) fn poll(&self, location: ProgramSite) -> Result<(), ExpansionFailure> {
        if let Some(cancellation) = &self.cancellation {
            cancellation
                .poll()
                .map_err(|reason| ExpansionFailure::Interrupted { reason, location })?;
        }
        Ok(())
    }

    /// A nonaccumulating pre-materialization bound; successful checks do not
    /// consume cumulative expansion counters or a constraint scalar receipt.
    pub(crate) fn check_family(
        &self,
        bytes: u128,
        location: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        self.poll(location)?;
        check(
            ExpansionResource::FamilyBytes,
            bytes,
            self.limits.max_family_bytes.min(isize::MAX as usize),
            location,
        )
    }

    pub(crate) fn check_metadata(
        &self,
        count: u128,
        location: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        self.poll(location)?;
        check(
            ExpansionResource::MetadataStatements,
            count,
            self.limits.max_metadata_statements,
            location,
        )
    }
    pub(crate) fn check_constants(
        &self,
        count: usize,
        site: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        self.poll(site)?;
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
            cancellation: None,
        }
    }

    pub(crate) fn with_allowance(mut self, allowance: crate::ConstraintAllowance) -> Self {
        self.allowance = Some(crate::constraint_allowance::Pending::new(allowance));
        self
    }

    pub(crate) fn charge(
        &mut self,
        resource: ExpansionResource,
        amount: u128,
        location: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        self.poll(location)?;
        let (used, ceiling) = match resource {
            ExpansionResource::TermWork => (&mut self.work, self.limits.max_term_work),
            ExpansionResource::Templates => (&mut self.templates, self.limits.max_templates),
            ExpansionResource::Values => (&mut self.values, self.limits.max_values),
            ExpansionResource::ScalarBytes => {
                (&mut self.scalar_bytes, self.limits.max_scalar_bytes)
            }
            ExpansionResource::Origins => (&mut self.origins, self.limits.max_origin_locations),
            ExpansionResource::Constants
            | ExpansionResource::MetadataStatements
            | ExpansionResource::FamilyBytes => {
                unreachable!("source occurrence and family bounds are checked directly")
            }
        };
        let observed = used.saturating_add(amount);
        check(resource, observed, ceiling, location)?;
        if matches!(resource, ExpansionResource::ScalarBytes)
            && let Some(allowance) = &mut self.allowance
        {
            allowance.scalar(amount);
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

/// Settlement does not reset enforcement history or retain runtime control.
struct ScalarReceipt<'a>(&'a mut Budget);

impl Drop for ScalarReceipt<'_> {
    fn drop(&mut self) {
        if let Some(allowance) = &mut self.0.allowance {
            allowance.settle();
        }
    }
}

#[cfg(test)]
mod tests;
