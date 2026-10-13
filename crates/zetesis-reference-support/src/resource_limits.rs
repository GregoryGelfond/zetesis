//! The limit each typed resource is counted against, set and read by resource.
//!
//! A refusal at a limit reports the resource it met: a formula or expansion
//! resource, an observation or objective admission resource, or an aggregate
//! error kind. A test that sets one such limit, leaving the others at their
//! defaults, names the resource; the field holding that limit, with its
//! default, is named here, once. A test that sets several limits at once, or a
//! limit no typed resource reports (a key-analysis or source-admission option,
//! say), names the fields itself and says why. Limits are read and set as `u64`
//! whatever the field's width, so one threshold search, which starts from the
//! resource's default, serves every resource.

use zetesis_ferraris::{AggregateErrorKind, AggregateLimits};
use zetesis_themelios::observation;
use zetesis_themelios::{ExpansionLimits, ExpansionResource, FormulaLimits, FormulaResource};

/// A limit field, at the width its limits declare.
enum Field<'a> {
    Count(&'a mut u64),
    Size(&'a mut usize),
    Narrow(&'a mut u32),
}

impl Field<'_> {
    /// Bounds the field by `limit`.
    ///
    /// # Panics
    /// Panics if `limit` does not fit the field's width.
    fn set(self, limit: u64) {
        match self {
            Self::Count(field) => *field = limit,
            Self::Size(field) => *field = usize::try_from(limit).expect("a limit within usize"),
            Self::Narrow(field) => *field = u32::try_from(limit).expect("a limit within u32"),
        }
    }

    /// The field's limit.
    ///
    /// # Panics
    /// Panics if the limit does not fit in `u64`, which cannot happen where
    /// `usize` is at most 64 bits.
    fn get(self) -> u64 {
        match self {
            Self::Count(field) => *field,
            Self::Size(field) => u64::try_from(*field).expect("a limit within u64"),
            Self::Narrow(field) => u64::from(*field),
        }
    }
}

/// The field of `limits` that bounds `resource`.
///
/// # Panics
/// Panics for a resource that no field of the formula limits bounds: a fixed
/// capacity of binding analysis, or a source-admission option.
fn formula_field(limits: &mut FormulaLimits, resource: FormulaResource) -> Field<'_> {
    match resource {
        FormulaResource::ProjectAtoms => Field::Size(&mut limits.max_project_atoms),
        FormulaResource::ProjectBytes => Field::Size(&mut limits.max_project_bytes),
        FormulaResource::ObjectivePresenceEntries => {
            Field::Size(&mut limits.max_objective_presence_entries)
        }
        FormulaResource::DomainValues => Field::Size(&mut limits.max_domain_values),
        FormulaResource::AssignmentValues => Field::Size(&mut limits.max_assignment_values),
        FormulaResource::GeneratedValues => Field::Size(&mut limits.max_generated_values),
        FormulaResource::SupportIndexEntries => Field::Size(&mut limits.max_support_index_entries),
        FormulaResource::SupportBytes => Field::Size(&mut limits.max_support_bytes),
        FormulaResource::AggregateCacheRows => Field::Size(&mut limits.max_aggregate_cache_rows),
        FormulaResource::DisjunctionElements => Field::Size(&mut limits.max_disjunction_elements),
        FormulaResource::AggregateCacheKeys => {
            Field::Size(&mut limits.max_aggregate_cache_key_bytes)
        }
        FormulaResource::AggregateCacheElements => {
            Field::Size(&mut limits.max_aggregate_cache_elements)
        }
        FormulaResource::AggregateCacheRoots => Field::Size(&mut limits.max_aggregate_cache_roots),
        FormulaResource::AnalysisNodes => Field::Size(&mut limits.max_analysis_nodes),
        FormulaResource::AnalysisEdges => Field::Size(&mut limits.max_analysis_edges),
        FormulaResource::Substitutions => Field::Count(&mut limits.max_substitutions),
        FormulaResource::Work => Field::Count(&mut limits.max_work),
        FormulaResource::AggregateElements => Field::Size(&mut limits.aggregate.max_elements),
        FormulaResource::ObjectiveElements => Field::Size(&mut limits.objective.max_templates),
        FormulaResource::SupportRounds => Field::Count(&mut limits.max_support_rounds),
        FormulaResource::ObjectiveFormulaAtoms => {
            Field::Size(&mut limits.max_objective_formula_atoms)
        }
        FormulaResource::ObjectiveFormulaNodes => {
            Field::Size(&mut limits.max_objective_formula_nodes)
        }
        FormulaResource::ObjectiveFormulaOperands => {
            Field::Size(&mut limits.max_objective_formula_operands)
        }
        FormulaResource::AtomStorageBytes => Field::Size(&mut limits.max_atom_storage_bytes),
        FormulaResource::Atoms => Field::Size(&mut limits.theory.max_atoms),
        FormulaResource::Nodes => Field::Size(&mut limits.theory.max_nodes),
        FormulaResource::Operands => Field::Size(&mut limits.theory.max_operands),
        FormulaResource::Roots => Field::Size(&mut limits.theory.max_roots),
        FormulaResource::Origins => Field::Size(&mut limits.max_origin_locations),
        FormulaResource::Warnings => Field::Size(&mut limits.max_warnings),
        FormulaResource::BindingCoefficientBits | FormulaResource::BindingBoundBits => {
            panic!("{resource:?} is a fixed capacity of binding analysis, not a formula limit")
        }
        FormulaResource::Variables | FormulaResource::Arity => {
            panic!(
                "{resource:?} is bounded by the source admission options, not the formula limits"
            )
        }
    }
}

/// The field of `limits` that bounds `resource`.
fn expansion_field(limits: &mut ExpansionLimits, resource: ExpansionResource) -> Field<'_> {
    match resource {
        ExpansionResource::Constants => Field::Size(&mut limits.max_constants),
        ExpansionResource::TermWork => Field::Size(&mut limits.max_term_work),
        ExpansionResource::Templates => Field::Size(&mut limits.max_templates),
        ExpansionResource::Values => Field::Size(&mut limits.max_values),
        ExpansionResource::ScalarBytes => Field::Size(&mut limits.max_scalar_bytes),
        ExpansionResource::FamilyBytes => Field::Size(&mut limits.max_family_bytes),
        ExpansionResource::Origins => Field::Size(&mut limits.max_origin_locations),
        ExpansionResource::MetadataStatements => Field::Size(&mut limits.max_metadata_statements),
    }
}

/// The field of `limits` that bounds `resource` when observation directives
/// are admitted.
///
/// # Panics
/// Panics for a resource counted only while observations are evaluated, which
/// the evaluation's own limits bound.
fn observation_field(
    limits: &mut observation::AdmissionLimits,
    resource: observation::Resource,
) -> Field<'_> {
    use observation::Resource;
    match resource {
        Resource::Directives => Field::Narrow(&mut limits.max_directives),
        Resource::Nodes => Field::Narrow(&mut limits.max_nodes),
        Resource::Depth => Field::Narrow(&mut limits.max_depth),
        Resource::Bytes => Field::Narrow(&mut limits.max_bytes),
        Resource::Variables => Field::Narrow(&mut limits.max_variables),
        Resource::BodyElements => Field::Narrow(&mut limits.max_body_elements),
        Resource::Arity => Field::Narrow(&mut limits.max_arity),
        Resource::Origins => Field::Narrow(&mut limits.max_origins),
        Resource::Work
        | Resource::Bindings
        | Resource::Terms
        | Resource::OutputBytes
        | Resource::ConstructionBytes
        | Resource::LocalBytes
        | Resource::TermStorageBytes => {
            panic!("{resource:?} is counted when observations are evaluated, not admitted")
        }
    }
}

/// The field of `limits` that bounds `resource` when objectives are admitted.
fn objective_field(
    limits: &mut zetesis_objective::AdmissionLimits,
    resource: zetesis_objective::AdmissionResource,
) -> Field<'_> {
    use zetesis_objective::AdmissionResource;
    match resource {
        AdmissionResource::Templates => Field::Size(&mut limits.max_templates),
        AdmissionResource::TupleWidth => Field::Size(&mut limits.max_tuple_width),
        AdmissionResource::Variables => Field::Size(&mut limits.max_variables_per_template),
        AdmissionResource::PositiveBody => Field::Size(&mut limits.max_positive_body),
        AdmissionResource::PredicateArity => Field::Size(&mut limits.max_predicate_arity),
        AdmissionResource::Filters => Field::Size(&mut limits.max_filters),
        AdmissionResource::ConditionNodes => Field::Size(&mut limits.max_condition_nodes),
    }
}

/// The field of `limits` whose ceiling an aggregate refusal of `kind` reports.
///
/// # Panics
/// Panics for a kind that reports no ceiling of the aggregate limits: a guard
/// count, a control stop, a malformed input, an allocation failure or an
/// arithmetic overflow.
fn aggregate_field(limits: &mut AggregateLimits, kind: AggregateErrorKind) -> Field<'_> {
    match kind {
        AggregateErrorKind::ElementLimit => Field::Size(&mut limits.max_elements),
        AggregateErrorKind::NodeLimit => Field::Size(&mut limits.max_nodes),
        AggregateErrorKind::OperandLimit => Field::Size(&mut limits.max_operands),
        AggregateErrorKind::WorkLimit => Field::Count(&mut limits.max_work),
        AggregateErrorKind::StateLimit => Field::Size(&mut limits.max_states),
        AggregateErrorKind::SubsetLimit => Field::Count(&mut limits.max_subsets),
        AggregateErrorKind::GuardLimit
        | AggregateErrorKind::Control(_)
        | AggregateErrorKind::InvalidPrefix { .. }
        | AggregateErrorKind::InvalidCondition { .. }
        | AggregateErrorKind::Allocation
        | AggregateErrorKind::ArithmeticOverflow => {
            panic!("{kind:?} reports no ceiling of the aggregate limits")
        }
    }
}

/// The default formula limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a resource no formula limit bounds, or a `limit` that does not
/// fit the field's width.
#[must_use]
pub fn formula(resource: FormulaResource, limit: u64) -> FormulaLimits {
    let mut limits = FormulaLimits::default();
    formula_field(&mut limits, resource).set(limit);
    limits
}

/// The default formula limit of `resource`.
///
/// # Panics
/// Panics for a resource no formula limit bounds, or, where `usize` is wider
/// than 64 bits, for a default beyond `u64`.
#[must_use]
pub fn formula_default(resource: FormulaResource) -> u64 {
    formula_field(&mut FormulaLimits::default(), resource).get()
}

/// The default expansion limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn expansion(resource: ExpansionResource, limit: u64) -> ExpansionLimits {
    let mut limits = ExpansionLimits::default();
    expansion_field(&mut limits, resource).set(limit);
    limits
}

/// The default expansion limit of `resource`.
///
/// # Panics
/// Panics only where `usize` is wider than 64 bits, for a default beyond
/// `u64`: every expansion resource's field is named here.
#[must_use]
pub fn expansion_default(resource: ExpansionResource) -> u64 {
    expansion_field(&mut ExpansionLimits::default(), resource).get()
}

/// The default observation admission limits, with `resource` bounded by
/// `limit`.
///
/// # Panics
/// Panics for a resource counted only while observations are evaluated, or a
/// `limit` beyond `u32`.
#[must_use]
pub fn observation(resource: observation::Resource, limit: u64) -> observation::AdmissionLimits {
    let mut limits = observation::AdmissionLimits::default();
    observation_field(&mut limits, resource).set(limit);
    limits
}

/// The default formula limits, with the observation admission `resource`
/// bounded by `limit`.
///
/// # Panics
/// Panics as [`observation()`] does.
#[must_use]
pub fn formula_observation(resource: observation::Resource, limit: u64) -> FormulaLimits {
    FormulaLimits {
        observation: observation(resource, limit),
        ..FormulaLimits::default()
    }
}

/// The default formula limits, with the objective admission `resource`
/// bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn formula_objective(
    resource: zetesis_objective::AdmissionResource,
    limit: u64,
) -> FormulaLimits {
    let mut limits = FormulaLimits::default();
    objective_field(&mut limits.objective, resource).set(limit);
    limits
}

/// The default aggregate limits, with the ceiling that a refusal of `kind`
/// reports bounded by `limit`.
///
/// # Panics
/// Panics for a kind that reports no ceiling, or a `limit` that does not fit
/// the field's width.
#[must_use]
pub fn aggregate(kind: AggregateErrorKind, limit: u64) -> AggregateLimits {
    let mut limits = AggregateLimits::default();
    aggregate_field(&mut limits, kind).set(limit);
    limits
}

/// The default formula limits, with the aggregate ceiling that a refusal of
/// `kind` reports bounded by `limit`.
///
/// # Panics
/// Panics as [`aggregate`] does.
#[must_use]
pub fn formula_aggregate(kind: AggregateErrorKind, limit: u64) -> FormulaLimits {
    FormulaLimits {
        aggregate: aggregate(kind, limit),
        ..FormulaLimits::default()
    }
}

#[cfg(test)]
mod tests;
