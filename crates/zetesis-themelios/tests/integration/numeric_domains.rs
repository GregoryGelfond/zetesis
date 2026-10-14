//! Numeric IR bounds preserve canonical grounding and original failures.

mod census;
mod diagnostic_requests;
mod observation;

use observation::{Argument, Observation};
use zetesis_domain::numeric::{Bound, Integrality, Interval, Status};
use zetesis_reference_support::prepare;
use zetesis_themelios::{
    AdmittedFormula, DomainLimits, ExpansionFailure, FormulaFailure, GroundingOptions,
    JoinStrategy, SourceAdmission,
};

const REGISTRATION: &str = include_str!("../fixtures/numeric-domains/registration-order.lp");

fn ground(
    source: &str,
    strategy: JoinStrategy,
    domains: Option<DomainLimits>,
    observer: &Observation,
) -> Result<AdmittedFormula, FormulaFailure> {
    // Do not inspect the preparation's analyzed/original Program or Debug view.
    prepare(source)?
        .with_grounding_options(GroundingOptions { joins: strategy })
        .with_domain_analysis(domains)
        .ground_with_observer(Some(observer))
}

fn equal(left: &AdmittedFormula, right: &AdmittedFormula) {
    assert_eq!(left.atoms(), right.atoms());
    assert_eq!(left.theory().nodes(), right.theory().nodes());
    assert_eq!(left.theory().operands(), right.theory().operands());
    assert_eq!(left.theory().roots(), right.theory().roots());
    assert_eq!(left.formula_origins(), right.formula_origins());
    assert_eq!(left.metadata(), right.metadata());
    assert_eq!(left.projection(), right.projection());
    assert_eq!(left.warnings(), right.warnings());
}

fn compared(source: &str, strategy: JoinStrategy) -> (AdmittedFormula, Observation) {
    let ordinary = ground(source, strategy, None, &Observation::default()).unwrap();
    let observation = Observation::default();
    let analyzed = ground(
        source,
        strategy,
        Some(DomainLimits::default()),
        &observation,
    )
    .unwrap();
    equal(&ordinary, &analyzed);
    observation.completed();
    assert_eq!(observation.formula_phases.get(), 1);
    (analyzed, observation)
}

fn numeric(low: i32, high: i32) -> Bound {
    Bound {
        integrality: Integrality::Integer,
        interval: Interval::Bounded { low, high },
    }
}

fn values(argument: &Argument, expected: &[&str]) {
    let actual: Vec<_> = argument
        .values
        .as_ref()
        .unwrap()
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(actual, expected);
}

fn atoms(formula: &AdmittedFormula, predicate: &str) -> Vec<String> {
    let mut result: Vec<_> = formula
        .atoms()
        .iter()
        .filter(|atom| atom.predicate().name() == predicate)
        .map(zetesis_reference_support::canonical)
        .collect();
    result.sort();
    result
}

#[test]
fn finite_meets_follow_registration_identity() {
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let (formula, observation) = compared(REGISTRATION, strategy);
        // These are deliberately not in semantic term order. A semantic-order
        // merge of this array with q's singleton loses the valid value 1.
        values(&observation.argument("p", 1, 0), &["2", "1"]);
        values(&observation.argument("q", 1, 0), &["1"]);
        values(&observation.argument("r", 1, 0), &["1"]);
        assert_eq!(atoms(&formula, "r"), ["r(1)"]);
        assert!(observation.work.get().domain_guard_rows.unwrap() > 0);
    }
}

#[test]
fn every_positive_head_contributes_bounds() {
    let source = include_str!("../fixtures/numeric-domains/all-producers.lp");
    let (formula, observation) = compared(source, JoinStrategy::Indexed);
    let p = observation.argument("p", 1, 0);
    assert_eq!(p.bound, numeric(1, 4));
    values(&p, &["1", "2", "3", "4"]);
    assert_eq!(
        p.producers.len(),
        3,
        "choice, normal and disjunctive producers"
    );
    values(&observation.argument("out", 1, 0), &["1", "2", "3", "4"]);
    assert_eq!(
        atoms(&formula, "out"),
        ["out(1)", "out(2)", "out(3)", "out(4)"]
    );
    assert_eq!(
        observation.work.get().domain_guard_rows,
        Some(0),
        "the broader analysis does not broaden finite-guard eligibility"
    );
}

#[test]
fn conditional_slots_exclude_outer_head_suffixes() {
    let source = include_str!("../fixtures/numeric-domains/conditional-slots.lp");
    let (formula, observation) = compared(source, JoinStrategy::Indexed);
    // The outer u range's generated slot aliases local X's coordinate. Its
    // disjoint [100,101] must never meet X's [1,2] inside the conditional head.
    assert_eq!(observation.argument("u", 1, 0).bound, numeric(100, 101));
    assert_eq!(observation.argument("h", 1, 0).bound, numeric(11, 12));
    assert_eq!(atoms(&formula, "h"), ["h(11)", "h(12)"]);
}

#[test]
fn choice_elements_have_independent_private_slots() {
    let source = include_str!("../fixtures/numeric-domains/choice-slots.lp");
    let (formula, observation) = compared(source, JoinStrategy::Indexed);
    // The first element's generated head suffix and the second element's
    // condition slot reuse a coordinate but belong to different frames.
    assert_eq!(observation.argument("a", 1, 0).bound, numeric(101, 101));
    assert_eq!(observation.argument("b", 1, 0).bound, numeric(17, 17));
    assert_eq!(atoms(&formula, "a"), ["a(101)"]);
    assert_eq!(atoms(&formula, "b"), ["b(17)"]);
}

#[test]
fn mixed_values_do_not_acquire_numeric_guards() {
    let source = include_str!("../fixtures/numeric-domains/mixed-values.lp");
    let (formula, observation) = compared(source, JoinStrategy::Indexed);
    let p = observation.argument("p", 1, 0);
    assert_eq!(
        p.bound,
        Bound {
            integrality: Integrality::Mixed,
            interval: Interval::Bounded { low: 1, high: 1 },
        }
    );
    values(&p, &["1", "a"]);
    assert_eq!(atoms(&formula, "p"), ["p(a)"]);
}

#[test]
fn nested_captures_keep_independent_numeric_bounds() {
    let source = include_str!("../fixtures/numeric-domains/nested-captures.lp");
    let (formula, observation) = compared(source, JoinStrategy::Indexed);
    let output = observation.argument("out", 1, 0);
    assert_eq!(output.bound, numeric(1, 2));
    values(&output, &["1", "2"]);
    assert_eq!(atoms(&formula, "out"), ["out(1)"]);
}

#[test]
fn unary_finite_guards_keep_their_exclusion_receipt() {
    let source = include_str!("../fixtures/numeric-domains/finite-guard.lp");
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let (formula, observation) = compared(source, strategy);
        assert_eq!(atoms(&formula, "p"), ["p(1)", "p(2)"]);
        assert_eq!(observation.work.get().domain_excluded_values, Some(6));
        assert!(observation.work.get().domain_guard_rows.unwrap() > 0);
        // Bound comparison filtering may discard rows before domain membership;
        // this proposition counts the six values excluded during preparation.
    }
}

#[test]
fn reached_arithmetic_errors_keep_their_source_site() {
    let source = include_str!("../fixtures/numeric-domains/fatal-arithmetic.lp");
    for strategy in [JoinStrategy::Indexed, JoinStrategy::Table] {
        let off = ground(source, strategy, None, &Observation::default()).unwrap_err();
        let observation = Observation::default();
        let on = ground(
            source,
            strategy,
            Some(DomainLimits::default()),
            &observation,
        )
        .unwrap_err();
        observation.completed();
        let FormulaFailure::Expansion(ExpansionFailure::Evaluation {
            error: expected,
            location: expected_location,
        }) = off
        else {
            panic!("unexpected complete-join refusal: {off}");
        };
        let FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location }) = on else {
            panic!("unexpected analyzed refusal: {on}");
        };
        assert_eq!(error, expected);
        assert_eq!(location, expected_location);
    }
}

#[test]
fn stopped_analysis_publishes_no_argument_prefix() {
    let ordinary = ground(
        REGISTRATION,
        JoinStrategy::Indexed,
        None,
        &Observation::default(),
    )
    .unwrap();
    let observation = Observation::default();
    let stopped = ground(
        REGISTRATION,
        JoinStrategy::Indexed,
        Some(DomainLimits {
            max_work: 1,
            ..DomainLimits::default()
        }),
        &observation,
    )
    .unwrap();
    equal(&ordinary, &stopped);
    assert_eq!(observation.callbacks.get(), 1);
    assert!(matches!(observation.status.get(), Some(Status::Stopped(_))));
    assert_eq!(observation.argument_count(), 0);
    assert_eq!(observation.work.get().domain_guard_rows, Some(0));
}

#[test]
fn exact_source_evaluation_uses_the_same_ir_bounds() {
    let observation = Observation::default();
    let admitted = prepare(REGISTRATION)
        .unwrap()
        .with_domain_analysis(Some(DomainLimits::default()))
        .try_source_with_observer(Some(&observation))
        .unwrap();
    let SourceAdmission::Admitted(source) = admitted else {
        panic!("acyclic fixture must enter exact source evaluation");
    };
    observation.completed();
    assert_eq!(observation.formula_phases.get(), 0);
    values(&observation.argument("r", 1, 0), &["1"]);
    let catalog = source.atom_catalog().atoms();
    let mut selected: Vec<_> = source
        .selected_positions()
        .unwrap()
        .iter()
        .map(|&position| zetesis_reference_support::canonical(catalog.at(position).unwrap()))
        .collect();
    selected.sort();
    assert_eq!(selected, ["a(2)", "p(1)", "p(2)", "q(1)", "r(1)"]);
}
