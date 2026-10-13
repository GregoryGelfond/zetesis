//! The limit each formula and expansion resource is counted against, set and
//! read by resource.
//!
//! A test that varies one resource's limit names the resource, and the field
//! holding that limit, with its default, is stated once here. Limits are read
//! and set as `u64` whatever the field's width, so one threshold search, which
//! starts from the resource's default, serves every resource.

use zetesis_themelios::{ExpansionLimits, ExpansionResource, FormulaLimits, FormulaResource};

/// A limit field, at the width its limits declare.
enum Field<'a> {
    Count(&'a mut u64),
    Size(&'a mut usize),
}

impl Field<'_> {
    fn set(self, limit: u64) {
        match self {
            Self::Count(field) => *field = limit,
            Self::Size(field) => *field = usize::try_from(limit).expect("a limit within usize"),
        }
    }

    fn get(self) -> u64 {
        match self {
            Self::Count(field) => *field,
            Self::Size(field) => u64::try_from(*field).expect("a limit within u64"),
        }
    }
}

/// The field of `limits` that bounds `resource`.
///
/// # Panics
/// Panics for a resource no test varies, whose field is not named here.
fn formula_field(limits: &mut FormulaLimits, resource: FormulaResource) -> Field<'_> {
    match resource {
        FormulaResource::ObjectivePresenceEntries => {
            Field::Size(&mut limits.max_objective_presence_entries)
        }
        FormulaResource::AssignmentValues => Field::Size(&mut limits.max_assignment_values),
        FormulaResource::SupportBytes => Field::Size(&mut limits.max_support_bytes),
        FormulaResource::AggregateCacheRows => Field::Size(&mut limits.max_aggregate_cache_rows),
        FormulaResource::AggregateCacheRoots => Field::Size(&mut limits.max_aggregate_cache_roots),
        FormulaResource::AnalysisNodes => Field::Size(&mut limits.max_analysis_nodes),
        FormulaResource::AnalysisEdges => Field::Size(&mut limits.max_analysis_edges),
        FormulaResource::Substitutions => Field::Count(&mut limits.max_substitutions),
        FormulaResource::Work => Field::Count(&mut limits.max_work),
        FormulaResource::AggregateElements => Field::Size(&mut limits.aggregate.max_elements),
        FormulaResource::SupportRounds => Field::Count(&mut limits.max_support_rounds),
        FormulaResource::ObjectiveFormulaAtoms => {
            Field::Size(&mut limits.max_objective_formula_atoms)
        }
        FormulaResource::ObjectiveFormulaNodes => {
            Field::Size(&mut limits.max_objective_formula_nodes)
        }
        FormulaResource::Atoms => Field::Size(&mut limits.theory.max_atoms),
        FormulaResource::Nodes => Field::Size(&mut limits.theory.max_nodes),
        other => panic!("no test varies the formula limit of {other:?}"),
    }
}

/// The field of `limits` that bounds `resource`.
///
/// # Panics
/// Panics for a resource no test varies, whose field is not named here.
fn expansion_field(limits: &mut ExpansionLimits, resource: ExpansionResource) -> Field<'_> {
    match resource {
        ExpansionResource::TermWork => Field::Size(&mut limits.max_term_work),
        ExpansionResource::Values => Field::Size(&mut limits.max_values),
        ExpansionResource::ScalarBytes => Field::Size(&mut limits.max_scalar_bytes),
        ExpansionResource::Origins => Field::Size(&mut limits.max_origin_locations),
        other => panic!("no test varies the expansion limit of {other:?}"),
    }
}

/// The default formula limits, with `resource` bounded by `limit`.
pub(crate) fn formula(resource: FormulaResource, limit: u64) -> FormulaLimits {
    let mut limits = FormulaLimits::default();
    formula_field(&mut limits, resource).set(limit);
    limits
}

/// The default formula limit of `resource`.
pub(crate) fn formula_default(resource: FormulaResource) -> u64 {
    formula_field(&mut FormulaLimits::default(), resource).get()
}

/// The default expansion limits, with `resource` bounded by `limit`.
pub(crate) fn expansion(resource: ExpansionResource, limit: u64) -> ExpansionLimits {
    let mut limits = ExpansionLimits::default();
    expansion_field(&mut limits, resource).set(limit);
    limits
}

/// The default expansion limit of `resource`.
pub(crate) fn expansion_default(resource: ExpansionResource) -> u64 {
    expansion_field(&mut ExpansionLimits::default(), resource).get()
}
