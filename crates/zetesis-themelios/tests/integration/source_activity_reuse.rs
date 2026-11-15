//! Retained producer lowering preserves source activity and complete scores.

use crate::support::source_cases;
use std::cell::Cell;
use zetesis_algebra::{Cancellation, Stop};
use zetesis_reference_support::{self as reference, BundleFixture, resource_limits};
use zetesis_test_support::thresholds::first_success;
use zetesis_themelios::{
    BundleAdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork, ProgramSite,
    prepare_bundle_formula_with_cancellation,
};

const CASES: &str = include_str!("../fixtures/source-activity-reuse.jsonl");

fn case(name: &str) -> source_cases::Case {
    source_cases::cases(CASES)
        .into_iter()
        .find(|case| case.name == name)
        .expect("named source-activity fixture")
}

fn complete(name: &str) {
    let case = case(name);
    let input = reference::formula(&case.source);
    assert_eq!(reference::exhaustive(&input), case.records, "{name}");
}

#[test]
fn families_keep_distinct_outer_bindings() {
    complete("outer_keys");
}

#[test]
fn remaining_literals_read_each_proposed_value() {
    complete("remaining_literal");
}

#[test]
fn signed_duplicate_tuples_keep_their_disjunction() {
    complete("signed_duplicates");
}

#[test]
fn empty_sum_families_retain_zero() {
    complete("empty_sum");
}

#[test]
fn activity_changes_between_refinement_rounds() {
    complete("cyclic_snapshot");
    let input = reference::formula(&case("cyclic_snapshot").source);
    // The required positive cycle excludes n(0). A stale optional row would
    // keep the spurious priority 1 even though every answer has only n(1).
    assert_eq!(input.objectives().priorities(), [2]);
}

#[test]
fn dependent_families_read_preceding_values() {
    complete("dependent_values");
}

#[test]
fn extremum_cuts_preserve_complete_scores() {
    complete("extremum_cuts");
}

#[test]
fn objective_preparation_preserves_the_reduct_subject() {
    source_cases::objectives_keep_the_original_reduct_subject(CASES);
}

#[test]
fn refused_work_never_publishes_a_partial_program() {
    let case = case("remaining_literal");
    let attempt = |maximum| {
        reference::admit(
            &case.source,
            &resource_limits::formula(FormulaResource::Work, maximum),
        )
    };
    let upper = first_success(
        resource_limits::formula_default(FormulaResource::Work),
        |maximum| match attempt(maximum) {
            Ok(_) => true,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            }) => false,
            Err(FormulaFailure::Aggregate { error, .. })
                if error.kind() == zetesis_ferraris::AggregateErrorKind::WorkLimit =>
            {
                false
            }
            Err(error) => panic!("unexpected refusal: {error}"),
        },
    );
    let error = attempt(upper - 1).unwrap_err();
    match error {
        FormulaFailure::Limit {
            resource: FormulaResource::Work,
            limit,
            observed,
            ..
        } => {
            assert_eq!(limit, u128::from(upper - 1));
            assert!(observed > limit);
        }
        FormulaFailure::Aggregate { error, .. } => {
            assert_eq!(
                error.kind(),
                zetesis_ferraris::AggregateErrorKind::WorkLimit
            );
        }
        other => panic!("unexpected refusal: {other}"),
    }
    assert_eq!(
        reference::exhaustive(&attempt(upper).unwrap()),
        case.records
    );
}

struct CancelObjective {
    cancellation: Cancellation,
    entered: Cell<bool>,
    failed: Cell<bool>,
}

impl GroundingObserver for CancelObjective {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_enter(&self, phase: GroundingPhase, _: Option<ProgramSite>) {
        if phase == GroundingPhase::ObjectiveActivation {
            assert!(!self.entered.replace(true));
            self.cancellation.cancel();
        }
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<ProgramSite>,
        outcome: GroundingOutcome,
        _: GroundingWork,
    ) {
        if phase == GroundingPhase::ObjectiveActivation {
            assert_eq!(outcome, GroundingOutcome::Failed);
            self.failed.set(true);
        }
    }
}

#[test]
fn objective_cancellation_keeps_the_original_source() {
    let case = case("outer_keys");
    let fixture = BundleFixture::new();
    fixture.write("entry.lp", &case.source);
    let cancellation = Cancellation::default();
    let prepared = prepare_bundle_formula_with_cancellation(
        fixture.bundle(),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
        &cancellation,
    )
    .unwrap();
    let observer = CancelObjective {
        cancellation,
        entered: Cell::new(false),
        failed: Cell::new(false),
    };
    let error = prepared.ground_with_observer(Some(&observer)).unwrap_err();
    assert!(observer.entered.get());
    assert!(observer.failed.get());
    assert_eq!(error.error().interruption(), Some(Stop::Cancelled));
    assert_eq!(
        error
            .bundle()
            .get(error.bundle().entry())
            .unwrap()
            .source()
            .text(),
        case.source
    );
    assert_eq!(
        reference::exhaustive(&reference::formula(&case.source)),
        case.records
    );
}
