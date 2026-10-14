use super::*;
use zetesis_objective::AdmissionResource;

const FORMULA: [FormulaResource; 30] = [
    FormulaResource::ProjectAtoms,
    FormulaResource::ProjectBytes,
    FormulaResource::ObjectivePresenceEntries,
    FormulaResource::DomainValues,
    FormulaResource::AssignmentValues,
    FormulaResource::GeneratedValues,
    FormulaResource::SupportIndexEntries,
    FormulaResource::SupportBytes,
    FormulaResource::AggregateCacheRows,
    FormulaResource::DisjunctionElements,
    FormulaResource::AggregateCacheKeys,
    FormulaResource::AggregateCacheElements,
    FormulaResource::AggregateCacheRoots,
    FormulaResource::AnalysisNodes,
    FormulaResource::AnalysisEdges,
    FormulaResource::Substitutions,
    FormulaResource::Work,
    FormulaResource::AggregateElements,
    FormulaResource::ObjectiveElements,
    FormulaResource::SupportRounds,
    FormulaResource::ObjectiveFormulaAtoms,
    FormulaResource::ObjectiveFormulaNodes,
    FormulaResource::ObjectiveFormulaOperands,
    FormulaResource::AtomStorageBytes,
    FormulaResource::Atoms,
    FormulaResource::Nodes,
    FormulaResource::Operands,
    FormulaResource::Roots,
    FormulaResource::Origins,
    FormulaResource::Warnings,
];

const EXPANSION: [ExpansionResource; 8] = [
    ExpansionResource::Constants,
    ExpansionResource::TermWork,
    ExpansionResource::Templates,
    ExpansionResource::Values,
    ExpansionResource::ScalarBytes,
    ExpansionResource::FamilyBytes,
    ExpansionResource::Origins,
    ExpansionResource::MetadataStatements,
];

const FORMULA_CORE: [FormulaResource; 2] = [FormulaResource::Variables, FormulaResource::Arity];

const INPUT: [InputLimit; 4] = [
    InputLimit::SourceBytes,
    InputLimit::SyntaxNodes,
    InputLimit::SyntaxDepth,
    InputLimit::BodyElements,
];

/// The input limits bundle admission bounds; loading bounds the source bytes.
const BUNDLE: [InputLimit; 3] = [
    InputLimit::SyntaxNodes,
    InputLimit::SyntaxDepth,
    InputLimit::BodyElements,
];

const CORE: [zetesis_core::AdmissionResource; 5] = [
    zetesis_core::AdmissionResource::Templates,
    zetesis_core::AdmissionResource::PredicateArity,
    zetesis_core::AdmissionResource::Variables,
    zetesis_core::AdmissionResource::PositiveBody,
    zetesis_core::AdmissionResource::DomainValues,
];

const PROGRAM: [ProgramResource; 4] = [
    ProgramResource::Nodes,
    ProgramResource::Depth,
    ProgramResource::BodyElements,
    ProgramResource::TextBytes,
];

const OBSERVATION: [observation::Resource; 8] = [
    observation::Resource::Directives,
    observation::Resource::Nodes,
    observation::Resource::Depth,
    observation::Resource::Bytes,
    observation::Resource::Variables,
    observation::Resource::BodyElements,
    observation::Resource::Arity,
    observation::Resource::Origins,
];

const OBJECTIVE: [AdmissionResource; 7] = [
    AdmissionResource::Templates,
    AdmissionResource::TupleWidth,
    AdmissionResource::Variables,
    AdmissionResource::PositiveBody,
    AdmissionResource::PredicateArity,
    AdmissionResource::Filters,
    AdmissionResource::ConditionNodes,
];

const AGGREGATE: [AggregateErrorKind; 6] = [
    AggregateErrorKind::ElementLimit,
    AggregateErrorKind::NodeLimit,
    AggregateErrorKind::OperandLimit,
    AggregateErrorKind::WorkLimit,
    AggregateErrorKind::StateLimit,
    AggregateErrorKind::SubsetLimit,
];

const LOADING: [BundleResource; 5] = [
    BundleResource::Roots,
    BundleResource::Files,
    BundleResource::FileBytes,
    BundleResource::TotalBytes,
    BundleResource::IncludeDepth,
];

const OBJECTIVE_PLAN: [ObjectiveBoundResource; 10] = [
    ObjectiveBoundResource::Atoms,
    ObjectiveBoundResource::Bindings,
    ObjectiveBoundResource::Keys,
    ObjectiveBoundResource::KeyBytes,
    ObjectiveBoundResource::TupleWidth,
    ObjectiveBoundResource::Nodes,
    ObjectiveBoundResource::Operands,
    ObjectiveBoundResource::Variables,
    ObjectiveBoundResource::BodyAtoms,
    ObjectiveBoundResource::Work,
];

const EVALUATION: [observation::Resource; 9] = [
    observation::Resource::Work,
    observation::Resource::Bindings,
    observation::Resource::Terms,
    observation::Resource::Nodes,
    observation::Resource::Depth,
    observation::Resource::Bytes,
    observation::Resource::OutputBytes,
    observation::Resource::LocalBytes,
    observation::Resource::TermStorageBytes,
];

/// The limit `independent` bounds each resource by. No field of these tables
/// defaults to it, and `independent` checks that: two resources sharing a
/// field that defaulted to it would each still read their default.
const PROBE: u64 = 3;

/// Every pair of resources in `resources`: bounding the first leaves the
/// second at its default, and the first reads back its new limit. A table
/// that named one field for two resources would fail here.
fn independent<R: Copy + std::fmt::Debug + PartialEq, L: Default>(
    resources: &[R],
    field: impl Fn(&mut L, R) -> Field<'_>,
) {
    for &bounded in resources {
        assert_ne!(
            field(&mut L::default(), bounded).get(),
            PROBE,
            "{bounded:?} defaults to the probe"
        );
        let mut limits = L::default();
        field(&mut limits, bounded).set(PROBE);
        for &other in resources {
            let expected = if other == bounded {
                PROBE
            } else {
                field(&mut L::default(), other).get()
            };
            assert_eq!(
                field(&mut limits, other).get(),
                expected,
                "{bounded:?} {other:?}"
            );
        }
    }
}

#[test]
fn each_formula_resource_has_a_field_of_its_own() {
    independent(&FORMULA, formula_field);
}

#[test]
fn each_expansion_resource_has_a_field_of_its_own() {
    independent(&EXPANSION, expansion_field);
}

#[test]
fn each_formula_resource_of_the_core_limits_has_a_field_of_its_own() {
    independent(&FORMULA_CORE, formula_core_field);
}

#[test]
fn each_input_limit_has_a_field_of_its_own() {
    independent(&INPUT, input_field);
}

#[test]
fn each_bundle_input_limit_has_a_field_of_its_own() {
    independent(&BUNDLE, bundle_field);
}

#[test]
fn each_core_admission_resource_has_a_field_of_its_own() {
    independent(&CORE, core_field);
}

#[test]
fn each_program_resource_has_a_field_of_its_own() {
    independent(&PROGRAM, program_field);
}

#[test]
fn each_observation_admission_resource_has_a_field_of_its_own() {
    independent(&OBSERVATION, observation_field);
}

#[test]
fn each_objective_admission_resource_has_a_field_of_its_own() {
    independent(&OBJECTIVE, objective_field);
}

#[test]
fn each_aggregate_ceiling_has_a_field_of_its_own() {
    independent(&AGGREGATE, aggregate_field);
}

#[test]
fn each_bundle_loading_resource_has_a_field_of_its_own() {
    independent(&LOADING, loading_field);
}

#[test]
fn each_objective_plan_resource_has_a_field_of_its_own() {
    independent(&OBJECTIVE_PLAN, objective_plan_field);
}

#[test]
fn each_observation_evaluation_resource_has_a_field_of_its_own() {
    independent(&EVALUATION, evaluation_field);
}

/// Whether every field of `limits` that `field` names for `resources` holds
/// its default.
fn at_defaults<R: Copy, L: Default>(
    limits: &mut L,
    resources: &[R],
    field: impl Fn(&mut L, R) -> Field<'_>,
) -> bool {
    resources
        .iter()
        .all(|&resource| field(limits, resource).get() == field(&mut L::default(), resource).get())
}

#[test]
fn the_formula_forms_bound_only_their_nested_field() {
    let mut observed = formula_observation(observation::Resource::Nodes, 0);
    assert_eq!(observed.observation.max_nodes, 0);
    assert!(at_defaults(
        &mut observed.objective,
        &OBJECTIVE,
        objective_field
    ));
    assert!(at_defaults(
        &mut observed.aggregate,
        &AGGREGATE,
        aggregate_field
    ));
    assert!(at_defaults(&mut observed, &FORMULA, formula_field));
    let mut objective = formula_objective(AdmissionResource::Filters, 0);
    assert_eq!(objective.objective.max_filters, 0);
    assert!(at_defaults(
        &mut objective.observation,
        &OBSERVATION,
        observation_field
    ));
    assert!(at_defaults(
        &mut objective.aggregate,
        &AGGREGATE,
        aggregate_field
    ));
    let mut aggregate = formula_aggregate(AggregateErrorKind::StateLimit, 0);
    assert_eq!(aggregate.aggregate.max_states, 0);
    assert!(at_defaults(
        &mut aggregate.objective,
        &OBJECTIVE,
        objective_field
    ));
    assert!(at_defaults(
        &mut aggregate.observation,
        &OBSERVATION,
        observation_field
    ));
}

#[test]
fn the_admission_forms_bound_only_their_nested_field() {
    let mut templates = admission_core(zetesis_core::AdmissionResource::Templates, 0);
    assert_eq!(templates.core_limits.max_templates, 0);
    assert!(at_defaults(&mut templates, &INPUT, input_field));
    let mut arity = admission_formula(FormulaResource::Arity, 0);
    assert_eq!(arity.core_limits.max_predicate_arity, 0);
    assert!(at_defaults(&mut arity, &INPUT, input_field));
    let mut program = program_core(zetesis_core::AdmissionResource::Templates, 0);
    assert_eq!(program.core_limits.max_templates, 0);
    assert!(at_defaults(&mut program, &PROGRAM, program_field));
}

#[test]
#[should_panic(expected = "is counted against the core admission limits: see admission_formula")]
fn a_core_counted_formula_resource_has_no_formula_field() {
    let _ = formula(FormulaResource::Variables, 1);
}

#[test]
#[should_panic(expected = "is not counted against the core admission limits")]
fn a_formula_bounded_resource_has_no_core_admission_field() {
    let _ = admission_formula(FormulaResource::Atoms, 1);
}

#[test]
#[should_panic(expected = "bounded when a bundle is loaded")]
fn the_source_bytes_have_no_bundle_admission_field() {
    let _ = bundle(InputLimit::SourceBytes, 1);
}

#[test]
#[should_panic(expected = "counted when observations are evaluated")]
fn an_evaluation_resource_has_no_admission_field() {
    let _ = observation(observation::Resource::Work, 1);
}

#[test]
#[should_panic(expected = "fixed capacity of binding analysis")]
fn a_fixed_capacity_has_no_formula_field() {
    let _ = formula(FormulaResource::BindingBoundBits, 1);
}

#[test]
#[should_panic(expected = "fixed width of the source model")]
fn a_fixed_source_width_has_no_loading_field() {
    let _ = bundle_loading(BundleResource::SourceCoordinates, 1);
}

#[test]
#[should_panic(expected = "bounded by prepare_choice_bounds' max_bytes")]
fn the_choice_proof_storage_has_no_plan_field() {
    let _ = objective_plan(ObjectiveBoundResource::ChoiceBytes, 1);
}

#[test]
#[should_panic(expected = "counted when observation directives are admitted")]
fn an_admission_resource_has_no_evaluation_field() {
    let _ = observation_evaluation(observation::Resource::Directives, 1);
}

#[test]
#[should_panic(expected = "bounded by the observation construction limits")]
fn the_construction_bytes_have_no_evaluation_field() {
    let _ = observation_evaluation(observation::Resource::ConstructionBytes, 1);
}
