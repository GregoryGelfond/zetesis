//! Exact stops of the new emitter and the existing whole-bound transaction.

use super::*;
use zetesis_algebra::Stop;

fn same_parts(actual: &FormulaParts, expected: &FormulaParts) {
    assert_eq!(actual.nodes(), expected.nodes());
    assert_eq!(actual.operands(), expected.operands());
    assert_eq!(actual.rows(), expected.rows());
    assert_eq!(actual.thresholds(), expected.thresholds());
    assert_eq!(actual.extremum_cuts(), expected.extremum_cuts());
    assert_eq!(actual.conditions(), expected.conditions());
    assert_eq!(actual.numeric_values(), expected.numeric_values());
    assert_eq!(actual.occurrences(), expected.occurrences());
}

fn same_theory(actual: &Theory, expected: &Theory) {
    assert_eq!(actual.atom_count(), expected.atom_count());
    assert_eq!(actual.roots(), expected.roots());
    same_parts(actual.parts(), expected.parts());
}

#[test]
fn late_layer_work_refusal_keeps_the_entire_exact_bound() {
    for strict in [false, true] {
        let (mut plan, required) = fixture::plan(6, true);
        let incumbent = Score::from_costs(vec![(1, 9), (0, 0)]).unwrap();
        let exact = fixture::bound(&plan, &incumbent, strict);
        let cancellation = Cancellation::default();
        plan.prepare_choice_bounds(
            &required,
            ObjectivePlanLimits::default(),
            u128::MAX,
            &cancellation,
        )
        .unwrap();
        let complete = fixture::bound(&plan, &incumbent, strict);
        assert!(complete.statistics().work > exact.statistics().work);
        let limits = ObjectiveBoundLimits {
            max_work: complete.statistics().work - 1,
            ..ObjectiveBoundLimits::default()
        };
        let refused = if strict {
            plan.strict_bound(&incumbent, limits, &cancellation)
        } else {
            plan.bound(&incumbent, limits, &cancellation)
        }
        .unwrap();
        let failure = refused.choice_failure().expect("late optional refusal");
        assert_eq!(failure.kind(), Kind::Limit(Resource::Work));
        assert_eq!(failure.statistics(), refused.statistics());
        assert!(refused.statistics().work > exact.statistics().work);
        same_theory(refused.theory(), exact.theory());
    }
}

#[test]
fn layer_node_and_operand_refusals_keep_the_exact_performed_prefix() {
    let (plan, required) = fixture::plan(6, true);
    let groups = fixture::checked_groups(&plan, &required);
    for (resource, expected_work, expected_nodes, expected_falsum) in [
        (Resource::Nodes, 6, 8, None),
        (Resource::Operands, 7, 9, Some(8)),
    ] {
        let cancellation = Cancellation::default();
        let mut limits = ObjectivePlanLimits::default();
        match resource {
            Resource::Nodes => limits.max_nodes = 8,
            Resource::Operands => limits.max_operands = 0,
            _ => unreachable!("fixture has two explicit independent ceilings"),
        }
        let mut work = Work::new(limits, &cancellation);
        let mut nodes = fixture::atoms();
        let mut terms = work.reserve(groups[0].keys.len()).unwrap();
        let mut falsum = None;
        let failure = super::super::super::layers::append_group(
            &plan.levels[&1],
            &groups[0],
            6,
            &mut nodes,
            &mut terms,
            &mut falsum,
            &mut work,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), Kind::Limit(resource));
        assert_eq!(failure.statistics().work, expected_work);
        assert_eq!(failure.statistics(), work.statistics());
        assert_eq!(nodes.view().len(), expected_nodes);
        assert_eq!(nodes.parts().occurrences(), 0);
        assert_eq!(falsum, expected_falsum);
        assert_eq!(
            terms,
            [AggregateElement {
                weight: 1,
                condition: 6
            }]
        );
    }
}

#[test]
fn cancellation_between_layer_groups_keeps_completed_nodes_and_terms() {
    let (plan, required) = fixture::plan(6, true);
    let groups = fixture::checked_groups(&plan, &required);
    let cancellation = Cancellation::default();
    let mut work = Work::new(ObjectivePlanLimits::default(), &cancellation);
    let mut nodes = fixture::atoms();
    let mut terms = work.reserve(plan.levels[&1].len()).unwrap();
    let mut falsum = None;
    super::super::super::layers::append_group(
        &plan.levels[&1],
        &groups[0],
        6,
        &mut nodes,
        &mut terms,
        &mut falsum,
        &mut work,
    )
    .unwrap();
    let before_nodes = nodes.parts().try_clone().unwrap();
    let before_terms = terms.clone();
    let before_work = work.statistics();
    let before_falsum = falsum;
    assert_eq!(terms.len(), 3);
    cancellation.cancel();
    let failure = super::super::super::layers::append_group(
        &plan.levels[&1],
        &groups[1],
        6,
        &mut nodes,
        &mut terms,
        &mut falsum,
        &mut work,
    )
    .unwrap_err();
    assert_eq!(failure.kind(), Kind::Control(Stop::Cancelled));
    assert_eq!(failure.statistics(), before_work);
    assert_eq!(work.statistics(), before_work);
    assert_eq!(terms, before_terms);
    assert_eq!(falsum, before_falsum);
    same_parts(nodes.parts(), &before_nodes);
}

#[test]
fn final_choice_admission_uses_the_inclusive_completed_work_boundary() {
    let (plan, _) = fixture::plan(6, true);
    let limits = ObjectiveBoundLimits::default();
    let mut expected = fixture::atoms();
    let mut transaction = expected.transaction();
    let root = push(&mut transaction, NodeView::And(&[0, 1]));
    transaction.commit();
    let admission = expected.prepare_admission(
        8,
        vec![root],
        AdmissionLimits {
            max_atoms: 8,
            max_roots: 1,
            max_nodes: limits.aggregate.max_nodes,
            max_operands: limits.aggregate.max_operands,
        },
    );
    // One And node plus its two validated edges precede final admission.
    let completed = 3 + u64::try_from(admission.work()).unwrap();
    let expected = admission.admit().unwrap();
    let cancellation = Cancellation::default();
    let mut short = Work::new(
        ObjectivePlanLimits {
            max_work: completed - 1,
            ..ObjectivePlanLimits::default()
        },
        &cancellation,
    );
    let failure =
        super::super::super::admit(&plan, 0, 1, fixture::atoms(), limits, &mut short).unwrap_err();
    assert_eq!(failure.kind(), Kind::Limit(Resource::Work));
    assert_eq!(failure.statistics().work, 3);
    assert_eq!(short.statistics(), failure.statistics());
    let mut inclusive = Work::new(
        ObjectivePlanLimits {
            max_work: completed,
            ..ObjectivePlanLimits::default()
        },
        &cancellation,
    );
    let actual =
        super::super::super::admit(&plan, 0, 1, fixture::atoms(), limits, &mut inclusive).unwrap();
    assert_eq!(inclusive.statistics().work, completed);
    same_theory(&actual, &expected);
}
