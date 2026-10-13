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

/// Every pair of resources in `resources`: bounding the first leaves the
/// second at its default, and the first reads back its new limit. A table
/// that named one field for two resources would fail here.
fn independent<R: Copy + std::fmt::Debug + PartialEq, L: Default>(
    resources: &[R],
    field: impl Fn(&mut L, R) -> Field<'_>,
) {
    for &bounded in resources {
        let mut limits = L::default();
        field(&mut limits, bounded).set(3);
        for &other in resources {
            let expected = if other == bounded {
                3
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
#[should_panic(expected = "counted when observations are evaluated")]
fn an_evaluation_resource_has_no_admission_field() {
    let _ = observation(observation::Resource::Work, 1);
}

#[test]
#[should_panic(expected = "fixed capacity of binding analysis")]
fn a_fixed_capacity_has_no_formula_field() {
    let _ = formula(FormulaResource::BindingBoundBits, 1);
}
