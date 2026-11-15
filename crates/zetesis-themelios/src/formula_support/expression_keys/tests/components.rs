//! Explicit component consumers preserve source-ordered witnesses and provenance.

use super::*;
use crate::formula_ground::ground_retained;
use crate::formula_support::component_plan::testing as components;
use crate::{FormulaFailure, FormulaResource};
use zetesis_algebra::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};

const INTERVAL: &str = "{p(3;1;2)}. {q(2)}. {r(a)}. h(Y) :- q(Y),p(X),X<Y,r(Z).";

fn compile(
    source: &str,
    disabled: bool,
    prepared: bool,
) -> (Compiled, GroundingWork, components::Counts) {
    let receipts = Receipts::default();
    let (result, counts) = components::scoped(prepared, || {
        controlled(disabled, || {
            ground(testing::prepare(source), Some(&receipts), None)
        })
    });
    (result.unwrap(), receipts.emission.get(), counts)
}

fn exact_output(left: &Compiled, right: &Compiled) {
    assert_same_program(left, right);
    assert_eq!(left.objective_origins, right.objective_origins);
    assert_eq!(left.objective_declarations, right.objective_declarations);
    assert_eq!(left.objectives.priorities(), right.objectives.priorities());
    assert_eq!(
        left.objectives.templates().iter().collect::<Vec<_>>(),
        right.objectives.templates().iter().collect::<Vec<_>>()
    );
    assert_eq!(left.theory.view().len(), right.theory.view().len());
    for node in 0..left.theory.view().len() {
        // Native payloads are outside the raw descriptor/operand arrays.
        assert_eq!(
            left.theory.view().node(node).unwrap(),
            right.theory.view().node(node).unwrap()
        );
    }
}

#[test]
fn component_ranges_activate_on_both_plan_owners() {
    let (reference, before, _) = compile(INTERVAL, true, true);
    assert_eq!(before.range_probes, Some(0));
    for prepared in [false, true] {
        let (selected, after, counts) = compile(INTERVAL, false, prepared);
        exact_output(&reference, &selected);
        if prepared {
            assert!(counts.preparations > 0, "must actually factor this rule");
            assert!(counts.cursors > 0);
        } else {
            assert_eq!(counts, components::Counts::default());
        }
        assert!(
            after.range_probes.unwrap() > 0,
            "component must select rows"
        );
        assert!(after.numeric_index_builds.unwrap() > 0);
        assert!(after.join_rows.unwrap() < before.join_rows.unwrap());
    }
}

#[test]
fn selected_components_keep_aliases_empty_branches_and_origins() {
    for source in [
        "{p(3,3);p(1,1);p(1,2)}. {q(2)}. {r(a)}. h(Y) :- q(Y),p(X,X),X<Y,r(Z).",
        "{p(1;3);q(2);r(a);a}. h(Y) :- q(Y),p(X),X<Y,r(Z). h(9) :- a.",
        "{p(-2;0;2);q(0);r(a)}. h(Y) :- q(Y),p(X),X<=Y,r(Z).",
    ] {
        let (reference, _, _) = compile(source, true, true);
        let (selected, work, counts) = compile(source, false, true);
        exact_output(&reference, &selected);
        assert!(counts.preparations > 0, "{source}");
        assert!(work.range_probes.unwrap() > 0, "{source}");
    }
}

#[test]
fn unsupported_components_keep_their_ordinary_filters() {
    for source in [
        "{p(1;3);q(2);r(a);n(1)}. h(Y) :- q(Y),p(X),not n(X),X<Y,r(Z).",
        "{p(1;a);q(2);r(a)}. h(Y) :- q(Y),p(X),X<Y,r(Z).",
    ] {
        let (reference, _, _) = compile(source, true, true);
        let (selected, work, counts) = compile(source, false, true);
        exact_output(&reference, &selected);
        assert!(
            counts.preparations > 0,
            "still an ordinary component: {source}"
        );
        assert_eq!(work.range_probes, Some(0), "{source}");
    }
}

fn interpretation(compiled: &Compiled, bits: usize) -> Interpretation {
    Interpretation::new(
        &compiled.theory,
        (0..compiled.theory.atom_count()).filter(|atom| bits & (1 << atom) != 0),
    )
    .unwrap()
}

#[test]
fn selected_component_keeps_every_original_and_frozen_truth_value() {
    let (ordinary, _, _) = compile(INTERVAL, true, true);
    let (selected, work, _) = compile(INTERVAL, false, true);
    exact_output(&ordinary, &selected);
    assert!(work.range_probes.unwrap() > 0);
    assert!(selected.theory.atom_count() <= 6);
    let width = 1 << selected.theory.atom_count();
    let limits = Limits::default();
    let cancellation = Cancellation::default();
    for candidate in 0..width {
        let left = interpretation(&ordinary, candidate);
        let right = interpretation(&selected, candidate);
        assert_eq!(
            models(&ordinary.theory, &left, limits, &cancellation).unwrap(),
            models(&selected.theory, &right, limits, &cancellation).unwrap()
        );
        for tested in 0..width {
            assert_eq!(
                models_reduct(
                    &ordinary.theory,
                    &left,
                    &interpretation(&ordinary, tested),
                    limits,
                    &cancellation,
                )
                .unwrap(),
                models_reduct(
                    &selected.theory,
                    &right,
                    &interpretation(&selected, tested),
                    limits,
                    &cancellation,
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn component_selection_keeps_the_exact_completed_work_boundary() {
    let retained = ground_retained(testing::prepare(INTERVAL), None, false).unwrap();
    let mut exact = testing::prepare(INTERVAL);
    exact.limits.max_work = retained.accounting.work;
    ground_retained(exact, None, false).unwrap();
    let mut short = testing::prepare(INTERVAL);
    short.limits.max_work = retained.accounting.work - 1;
    assert!(matches!(
        ground_retained(short, None, false),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
}

#[test]
fn component_selection_does_not_publish_after_cancellation() {
    for disabled in [false, true] {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut preparation = testing::prepare(INTERVAL);
        preparation.budget = preparation.budget.with_cancellation(Some(cancellation));
        let failure = controlled(disabled, || ground(preparation, None, None)).unwrap_err();
        assert_eq!(
            failure.interruption(),
            Some(zetesis_algebra::Stop::Cancelled)
        );
    }
}
