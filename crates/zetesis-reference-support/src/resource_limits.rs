//! The limit each typed resource is counted against, set by resource, for the
//! limits an admission takes and for those of three other operations whose
//! tests span crates.
//!
//! A refusal at a limit reports the resource it met. The view holds a family
//! of limits when a `zetesis-themelios` admission takes it, directly or nested
//! in the limits it takes: the admission of a source, of a bundle or of a
//! logical program, formula preparation and expansion included. Those are the
//! formula and expansion resources; the input limits of a source or a bundle;
//! the core admission resources of a source or a logical program, and the
//! logical program resources; the observation and objective admission
//! resources; and the aggregate ceilings an aggregate error kind reports.
//! Formula admission counts template variables and predicate arity against the
//! core admission limits, so those two formula resources are set through
//! [`admission_formula`].
//!
//! The view also holds the limits of three operations other than admission,
//! because the tests of more than one crate set them one at a time: loading a
//! bundle ([`bundle_loading`]), building an objective plan
//! ([`objective_plan`]), and evaluating an admitted program's observations
//! ([`observation_evaluation`]). Evaluation counts `Nodes`, `Depth` and `Bytes`
//! per constructed symbol against fields of its own, while [`observation()`]
//! pairs those three resources with their admission fields and refuses the
//! resources counted only while observations are evaluated.
//!
//! The limits of operations that only one crate's tests set are paired in that
//! crate's `tests/integration/support`. In `zetesis-themelios` these are
//! compiling a program's metadata through its own door
//! (`metadata_resources`), planning counts (`count_plan_resources`), bounding
//! an objective (`objective_bound_resources`, which sets its aggregate ceilings
//! through [`aggregate`]), and constructing observation symbols
//! (`observation_construction`).
//!
//! The view stands above `zetesis-themelios`, so it serves the tests that use
//! that crate as a dependency, its own integration tests among them. A unit
//! test of `zetesis-themelios` would link a second copy of the crate, and a
//! test of a crate below it would make that crate depend on its own
//! dependents, so the tests of the crates that define these limits (the unit
//! tests of `zetesis-themelios`, and the tests of `zetesis-core`,
//! `zetesis-ferraris` and `zetesis-objective`) name the fields they set
//! themselves. The view's own tests check that each resource it names has a
//! field of its own; that the field is the one a refusal reports is checked
//! only where a test the view serves meets that refusal.
//!
//! A test the view serves that sets one such limit, leaving the others at
//! their defaults, names the resource; the field holding that limit is named
//! here, once. A test that sets several limits at once, or varies one limit of
//! a configuration that already sets others, names the fields, and its code
//! shows why. A test that sets a field of these limits that no typed resource
//! reports, such as the formula limits' key-analysis work, says why where it
//! sets it. Formula and expansion limits are also read by resource, so a
//! threshold search over one of them starts from its default. Limits are read
//! and set as `u64` whatever the field's width.

use zetesis_ferraris::{AggregateErrorKind, AggregateLimits};
use zetesis_themelios::objective_bound::{ObjectiveBoundResource, ObjectivePlanLimits};
use zetesis_themelios::observation;
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionOptions, BundleLimits, BundleResource, ExpansionLimits,
    ExpansionResource, FormulaLimits, FormulaResource, InputLimit, ProgramAdmissionOptions,
    ProgramResource,
};

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
/// capacity of binding analysis, or a resource counted against the core
/// admission limits.
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
                "{resource:?} is counted against the core admission limits: see admission_formula"
            )
        }
    }
}

/// The field of `limits` that bounds the formula `resource`, among the core
/// admission limits that formula admission also checks.
///
/// # Panics
/// Panics for a formula resource that the formula limits bound, or a fixed
/// capacity of binding analysis.
fn formula_core_field(
    limits: &mut zetesis_core::AdmissionLimits,
    resource: FormulaResource,
) -> Field<'_> {
    match resource {
        FormulaResource::Variables => {
            core_field(limits, zetesis_core::AdmissionResource::Variables)
        }
        FormulaResource::Arity => {
            core_field(limits, zetesis_core::AdmissionResource::PredicateArity)
        }
        _ => panic!("{resource:?} is not counted against the core admission limits"),
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

/// The field of `options` that bounds the input `resource` of one source.
fn input_field(options: &mut AdmissionOptions, resource: InputLimit) -> Field<'_> {
    match resource {
        InputLimit::SourceBytes => Field::Size(&mut options.max_source_bytes),
        InputLimit::SyntaxNodes => Field::Size(&mut options.max_syntax_nodes),
        InputLimit::SyntaxDepth => Field::Size(&mut options.max_syntax_depth),
        InputLimit::BodyElements => Field::Size(&mut options.max_body_elements),
    }
}

/// The field of `options` that bounds the input `resource` across a bundle.
///
/// # Panics
/// Panics for the source bytes, which the bundle's loading limits bound.
fn bundle_field(options: &mut BundleAdmissionOptions, resource: InputLimit) -> Field<'_> {
    match resource {
        InputLimit::SyntaxNodes => Field::Size(&mut options.max_syntax_nodes),
        InputLimit::SyntaxDepth => Field::Size(&mut options.max_syntax_depth),
        InputLimit::BodyElements => Field::Size(&mut options.max_body_elements),
        InputLimit::SourceBytes => {
            panic!("{resource:?} is bounded when a bundle is loaded, not admitted")
        }
    }
}

/// The field of `limits` that bounds the core admission `resource`.
fn core_field(
    limits: &mut zetesis_core::AdmissionLimits,
    resource: zetesis_core::AdmissionResource,
) -> Field<'_> {
    use zetesis_core::AdmissionResource;
    match resource {
        AdmissionResource::Templates => Field::Size(&mut limits.max_templates),
        AdmissionResource::PredicateArity => Field::Size(&mut limits.max_predicate_arity),
        AdmissionResource::Variables => Field::Size(&mut limits.max_variables_per_template),
        AdmissionResource::PositiveBody => Field::Size(&mut limits.max_positive_body),
        AdmissionResource::DomainValues => Field::Size(&mut limits.max_domain_values),
    }
}

/// The field of `options` that bounds the logical program `resource`.
fn program_field(options: &mut ProgramAdmissionOptions, resource: ProgramResource) -> Field<'_> {
    match resource {
        ProgramResource::Nodes => Field::Size(&mut options.max_nodes),
        ProgramResource::Depth => Field::Size(&mut options.max_depth),
        ProgramResource::BodyElements => Field::Size(&mut options.max_body_elements),
        ProgramResource::TextBytes => Field::Size(&mut options.max_text_bytes),
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

/// The field of `limits` that bounds `resource` when a bundle is loaded.
///
/// # Panics
/// Panics for a fixed width of the source model, which no loading limit
/// bounds.
fn loading_field(limits: &mut BundleLimits, resource: BundleResource) -> Field<'_> {
    match resource {
        BundleResource::Roots => Field::Size(&mut limits.max_roots),
        BundleResource::Files => Field::Size(&mut limits.max_files),
        BundleResource::FileBytes => Field::Size(&mut limits.max_file_bytes),
        BundleResource::TotalBytes => Field::Size(&mut limits.max_total_bytes),
        BundleResource::IncludeDepth => Field::Size(&mut limits.max_include_depth),
        BundleResource::SourceCoordinates | BundleResource::SourceIdentities => {
            panic!("{resource:?} is a fixed width of the source model, not a loading limit")
        }
    }
}

/// The field of `limits` that bounds `resource` when an objective plan is
/// built.
///
/// # Panics
/// Panics for the choice proof's storage, which the `max_bytes` that
/// `prepare_choice_bounds` takes bounds.
fn objective_plan_field(
    limits: &mut ObjectivePlanLimits,
    resource: ObjectiveBoundResource,
) -> Field<'_> {
    match resource {
        ObjectiveBoundResource::Atoms => Field::Size(&mut limits.max_atoms),
        ObjectiveBoundResource::Bindings => Field::Count(&mut limits.max_bindings),
        ObjectiveBoundResource::Keys => Field::Size(&mut limits.max_keys),
        ObjectiveBoundResource::KeyBytes => Field::Size(&mut limits.max_key_bytes),
        ObjectiveBoundResource::TupleWidth => Field::Size(&mut limits.max_tuple_width),
        ObjectiveBoundResource::Nodes => Field::Size(&mut limits.max_nodes),
        ObjectiveBoundResource::Operands => Field::Size(&mut limits.max_operands),
        ObjectiveBoundResource::Variables => Field::Size(&mut limits.max_variables),
        ObjectiveBoundResource::BodyAtoms => Field::Size(&mut limits.max_body_atoms),
        ObjectiveBoundResource::Work => Field::Count(&mut limits.max_work),
        ObjectiveBoundResource::ChoiceBytes => {
            panic!("{resource:?} is bounded by prepare_choice_bounds' max_bytes, not a plan limit")
        }
    }
}

/// The field of `limits` that bounds `resource` while an admitted program's
/// observations are evaluated. Evaluation counts `Nodes`, `Depth` and `Bytes`
/// per constructed symbol, against these fields rather than the admission's.
///
/// # Panics
/// Panics for a resource counted only when observation directives are
/// admitted, or for the construction bytes, which the construction limits
/// bound.
fn evaluation_field(
    limits: &mut observation::Limits,
    resource: observation::Resource,
) -> Field<'_> {
    use observation::Resource;
    match resource {
        Resource::Work => Field::Count(&mut limits.max_work),
        Resource::Bindings => Field::Count(&mut limits.max_bindings),
        Resource::Terms => Field::Size(&mut limits.max_terms),
        Resource::Nodes => Field::Size(&mut limits.max_symbol_nodes),
        Resource::Depth => Field::Size(&mut limits.max_symbol_depth),
        Resource::Bytes => Field::Size(&mut limits.max_symbol_bytes),
        Resource::OutputBytes => Field::Size(&mut limits.max_output_bytes),
        Resource::LocalBytes => Field::Size(&mut limits.max_local_bytes),
        Resource::TermStorageBytes => Field::Size(&mut limits.max_term_storage_bytes),
        Resource::ConstructionBytes => {
            panic!("{resource:?} is bounded by the observation construction limits")
        }
        Resource::Directives
        | Resource::Variables
        | Resource::BodyElements
        | Resource::Arity
        | Resource::Origins => {
            panic!(
                "{resource:?} is counted when observation directives are admitted, not evaluated"
            )
        }
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
/// Panics for a resource no formula limit bounds (a fixed capacity, or a
/// resource [`admission_formula`] bounds), or a `limit` that does not fit the
/// field's width.
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

/// The default admission options of one source, with the input `resource`
/// bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn admission(resource: InputLimit, limit: u64) -> AdmissionOptions {
    let mut options = AdmissionOptions::default();
    input_field(&mut options, resource).set(limit);
    options
}

/// The default admission options of one source, with the core admission
/// `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn admission_core(resource: zetesis_core::AdmissionResource, limit: u64) -> AdmissionOptions {
    let mut options = AdmissionOptions::default();
    core_field(&mut options.core_limits, resource).set(limit);
    options
}

/// The default admission options of one source, with the formula `resource`
/// bounded by `limit`: formula admission counts template variables and
/// predicate arity against the core admission limits.
///
/// # Panics
/// Panics for a formula resource that the formula limits bound ([`formula`]),
/// a fixed capacity, or a `limit` that does not fit the field's width.
#[must_use]
pub fn admission_formula(resource: FormulaResource, limit: u64) -> AdmissionOptions {
    let mut options = AdmissionOptions::default();
    formula_core_field(&mut options.core_limits, resource).set(limit);
    options
}

/// The default bundle admission options, with the input `resource` bounded by
/// `limit` across the bundle.
///
/// # Panics
/// Panics for the source bytes, which the bundle's loading limits bound, or a
/// `limit` that does not fit the field's width.
#[must_use]
pub fn bundle(resource: InputLimit, limit: u64) -> BundleAdmissionOptions {
    let mut options = BundleAdmissionOptions::default();
    bundle_field(&mut options, resource).set(limit);
    options
}

/// The default logical program admission options, with `resource` bounded by
/// `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn program(resource: ProgramResource, limit: u64) -> ProgramAdmissionOptions {
    let mut options = ProgramAdmissionOptions::default();
    program_field(&mut options, resource).set(limit);
    options
}

/// The default logical program admission options, with the core admission
/// `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a `limit` that does not fit the field's width.
#[must_use]
pub fn program_core(
    resource: zetesis_core::AdmissionResource,
    limit: u64,
) -> ProgramAdmissionOptions {
    let mut options = ProgramAdmissionOptions::default();
    core_field(&mut options.core_limits, resource).set(limit);
    options
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

/// The default bundle loading limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for a fixed width of the source model, or a `limit` that does not
/// fit the field's width.
#[must_use]
pub fn bundle_loading(resource: BundleResource, limit: u64) -> BundleLimits {
    let mut limits = BundleLimits::default();
    loading_field(&mut limits, resource).set(limit);
    limits
}

/// The default objective plan limits, with `resource` bounded by `limit`.
///
/// # Panics
/// Panics for the choice proof's storage, or a `limit` that does not fit the
/// field's width.
#[must_use]
pub fn objective_plan(resource: ObjectiveBoundResource, limit: u64) -> ObjectivePlanLimits {
    let mut limits = ObjectivePlanLimits::default();
    objective_plan_field(&mut limits, resource).set(limit);
    limits
}

/// The default observation evaluation limits, with `resource` bounded by
/// `limit`.
///
/// # Panics
/// Panics for a resource counted only when observation directives are
/// admitted, for the construction bytes, or for a `limit` that does not fit the
/// field's width.
#[must_use]
pub fn observation_evaluation(resource: observation::Resource, limit: u64) -> observation::Limits {
    let mut limits = observation::Limits::default();
    evaluation_field(&mut limits, resource).set(limit);
    limits
}

#[cfg(test)]
mod tests;
