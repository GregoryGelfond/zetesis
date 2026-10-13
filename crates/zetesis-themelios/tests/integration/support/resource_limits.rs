//! The limit each formula and expansion resource is counted against, set and
//! read by resource.
//!
//! An integration test of this crate that sets one resource's limit, leaving
//! the others at their defaults, names the resource; the field of
//! [`FormulaLimits`] or [`ExpansionLimits`] holding that limit, with its
//! default, is named here. A test that sets several of those fields at once,
//! or one that no formula or expansion resource reports (an observation,
//! objective-admission, aggregate or key-analysis limit), names the fields
//! itself and says why. Limits are read and set as `u64` whatever the field's
//! width, so one threshold search, which starts from the resource's default,
//! serves every resource.

use zetesis_themelios::{ExpansionLimits, ExpansionResource, FormulaLimits, FormulaResource};

/// A limit field, at the width its limits declare.
enum Field<'a> {
    Count(&'a mut u64),
    Size(&'a mut usize),
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
        }
    }
}

/// The field of `limits` that bounds `resource`.
///
/// # Panics
/// Panics for a resource whose field is not named here.
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
        FormulaResource::Atoms => Field::Size(&mut limits.theory.max_atoms),
        FormulaResource::Nodes => Field::Size(&mut limits.theory.max_nodes),
        FormulaResource::Operands => Field::Size(&mut limits.theory.max_operands),
        FormulaResource::Roots => Field::Size(&mut limits.theory.max_roots),
        FormulaResource::Origins => Field::Size(&mut limits.max_origin_locations),
        FormulaResource::Warnings => Field::Size(&mut limits.max_warnings),
        other => panic!("no field is named here for the formula limit of {other:?}"),
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

/// The default formula limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a resource whose field is not named here, or a `limit` that
/// does not fit the field's width.
pub(crate) fn formula(resource: FormulaResource, limit: u64) -> FormulaLimits {
    let mut limits = FormulaLimits::default();
    formula_field(&mut limits, resource).set(limit);
    limits
}

/// The default formula limit of `resource`.
///
/// # Panics
/// Panics for a resource whose field is not named here, or, where `usize` is
/// wider than 64 bits, for a default beyond `u64`.
pub(crate) fn formula_default(resource: FormulaResource) -> u64 {
    formula_field(&mut FormulaLimits::default(), resource).get()
}

/// The default expansion limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
pub(crate) fn expansion(resource: ExpansionResource, limit: u64) -> ExpansionLimits {
    let mut limits = ExpansionLimits::default();
    expansion_field(&mut limits, resource).set(limit);
    limits
}

/// The default expansion limit of `resource`.
///
/// # Panics
/// Panics only where `usize` is wider than 64 bits, for a default beyond
/// `u64`: every expansion resource's field is named here.
pub(crate) fn expansion_default(resource: ExpansionResource) -> u64 {
    expansion_field(&mut ExpansionLimits::default(), resource).get()
}
