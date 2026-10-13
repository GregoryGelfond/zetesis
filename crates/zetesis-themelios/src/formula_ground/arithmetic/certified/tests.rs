//! Source admission comparisons retain the ordinary family scan as a reference.

use std::cell::RefCell;

use super::*;
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_ir::{HeadIr, LiteralIr, RuleIr};
use crate::formula_support::testing;
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{ExpansionFailure, FormulaResource, GroundingPhase, GroundingWork};
use themelios_program::program::DefaultNegation;
use zetesis_core::ValueLimits;
use zetesis_cpu::{Cancellation, Stop};

mod accounting;

const TOTAL: &str = include_str!("../../../../tests/fixtures/arithmetic-family-totality/total.lp");
const SIBLINGS: &str =
    include_str!("../../../../tests/fixtures/arithmetic-family-totality/siblings.lp");

struct Control {
    disabled: bool,
    proofs: usize,
    cancel: Option<Cancellation>,
}
thread_local! {
    static CONTROL: RefCell<Control> = const { RefCell::new(Control { disabled: false, proofs: 0, cancel: None }) };
}
struct Restore(Control);
impl Drop for Restore {
    fn drop(&mut self) {
        CONTROL.with(|control| std::mem::swap(&mut *control.borrow_mut(), &mut self.0));
    }
}
pub(super) fn disabled() -> bool {
    CONTROL.with(|control| control.borrow().disabled)
}
pub(super) fn proved() {
    CONTROL.with(|control| {
        let mut control = control.borrow_mut();
        control.proofs += 1;
        if let Some(cancellation) = &control.cancel {
            cancellation.cancel();
        }
    });
}
fn controlled<T>(
    disabled: bool,
    cancel: Option<Cancellation>,
    run: impl FnOnce() -> T,
) -> (T, usize) {
    let _restore = Restore(CONTROL.replace(Control {
        disabled,
        proofs: 0,
        cancel,
    }));
    let value = run();
    (value, CONTROL.with(|control| control.borrow().proofs))
}

fn partial_rule(prepared: &Prepared) -> &RuleIr {
    prepared
        .rules_without_facts()
        .iter()
        .find(|rule| crate::formula_support::family::partial(&rule.body))
        .unwrap()
}

fn scan(
    source: &str,
    disabled: bool,
) -> (
    Result<Vec<crate::FormulaWarning>, FormulaFailure>,
    GroundingWork,
) {
    let mut result = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        *counters = Counters::resume(std::mem::take(counters).into_accounting(), profile.work());
        let value = profile
            .phase(GroundingPhase::SupportCompletion, None, || {
                controlled(disabled, None, || {
                    super::super::prepare(
                        program,
                        support,
                        computation,
                        &FormulaLimits::default(),
                        &mut testing::budget(),
                        counters,
                    )
                })
                .0
            })
            .map(crate::formula_support::family::Warnings::into_values);
        result = Some((value, observer.0.get()));
    });
    result.unwrap()
}

fn assert_same_program(left: &Compiled, right: &Compiled) {
    let atoms = |compiled: &Compiled| {
        compiled
            .atoms
            .atoms()
            .iter()
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(left), atoms(right));
    assert_eq!(left.warnings, right.warnings);
    assert_eq!(left.theory.nodes(), right.theory.nodes());
    assert_eq!(left.theory.operands(), right.theory.operands());
    assert_eq!(left.theory.roots(), right.theory.roots());
}

#[test]
fn total_columns_remove_the_family_cartesian_scan() {
    let (ordinary, before) = scan(TOTAL, true);
    let (certified, after) = scan(TOTAL, false);
    assert_eq!(ordinary.unwrap(), certified.unwrap());
    assert_eq!(before.binding_snapshots, Some(10));
    assert_eq!(after.binding_snapshots, Some(0));
    assert_eq!(after.join_rows, Some(0));
    for source in [
        TOTAL,
        include_str!("../../../../tests/fixtures/arithmetic-family-totality/unary.lp"),
    ] {
        let (ordinary, _) = controlled(true, None, || ground(testing::prepare(source), None, None));
        let (actual, proofs) =
            controlled(false, None, || ground(testing::prepare(source), None, None));
        assert!(proofs > 0);
        assert_same_program(&ordinary.unwrap(), &actual.unwrap());
    }
}

#[test]
fn one_partial_sibling_keeps_the_whole_family_scan() {
    testing::with_completed_source(SIBLINGS, |program, support, computation, counters| {
        let location = partial_rule(program).location;
        let siblings: Vec<_> = program
            .rules_without_facts()
            .iter()
            .filter(|rule| rule.location == location)
            .collect();
        assert_eq!(siblings.len(), 2, "one pooled original source family");
        let mut outcomes = Vec::new();
        for rule in siblings {
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            outcomes.push(
                join.total_family_fragment(rule, computation, &FormulaLimits::default(), counters)
                    .unwrap(),
            );
        }
        outcomes.sort_unstable();
        assert_eq!(outcomes, [false, true]);
        assert!(
            !family(
                program,
                location,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters
            )
            .unwrap()
        );
    });
    let (ordinary, before) = scan(SIBLINGS, true);
    let (actual, after) = scan(SIBLINGS, false);
    let ordinary = ordinary.unwrap();
    assert_eq!(ordinary.len(), 1);
    assert_eq!(actual.unwrap(), ordinary);
    assert_eq!(after.binding_snapshots, before.binding_snapshots);
    assert!(after.binding_snapshots.unwrap() > 0);
}

#[test]
fn zero_and_overflow_keep_their_typed_diagnostics() {
    let mixed = include_str!("../../../../tests/fixtures/arithmetic-family-totality/zero.lp");
    let (ordinary, before) = scan(mixed, true);
    let (actual, after) = scan(mixed, false);
    let ordinary = ordinary.unwrap();
    assert_eq!(ordinary.len(), 1);
    assert_eq!(actual.unwrap(), ordinary);
    assert_eq!(after.binding_snapshots, before.binding_snapshots);
    for (source, expected) in [
        (
            include_str!("../../../../tests/fixtures/arithmetic-family-totality/all-zero.lp"),
            crate::observation::EvaluationError::Undefined,
        ),
        (
            include_str!("../../../../tests/fixtures/arithmetic-family-totality/overflow.lp"),
            crate::observation::EvaluationError::Overflow,
        ),
        (
            include_str!(
                "../../../../tests/fixtures/arithmetic-family-totality/overflow-sibling.lp"
            ),
            crate::observation::EvaluationError::Overflow,
        ),
    ] {
        let (ordinary, _) = scan(source, true);
        let (actual, _) = scan(source, false);
        let ordinary = ordinary.unwrap_err();
        let actual = actual.unwrap_err();
        assert_eq!(actual.site(), ordinary.site());
        for failure in [ordinary, actual] {
            assert!(
                matches!(failure, FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if error == expected)
            );
        }
    }
}

#[test]
fn empty_columns_do_not_invent_defined_witnesses() {
    let source = include_str!("../../../../tests/fixtures/arithmetic-family-totality/empty.lp");
    testing::with_completed_source(source, |program, support, computation, counters| {
        assert!(
            !family(
                program,
                partial_rule(program).location,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters
            )
            .unwrap()
        );
    });
    let (ordinary, _) = scan(source, true);
    let (actual, _) = scan(source, false);
    assert_eq!(actual.unwrap(), ordinary.unwrap());
}

#[test]
fn uncovered_head_and_negative_keys_decline() {
    let source = include_str!("../../../../tests/fixtures/arithmetic-family-totality/keys.lp");
    testing::with_completed_source(source, |program, support, computation, counters| {
        let original = partial_rule(program);
        let limits = FormulaLimits::default();
        let HeadIr::Normal(head) = &original.head else {
            panic!("plain normal fixture")
        };
        let mut named = |negation, name| {
            original
                .body
                .iter()
                .find_map(|literal| {
                    let LiteralIr::Atom(found, atom) = literal else {
                        return None;
                    };
                    let pattern = computation
                        .static_pattern(*atom, &limits, counters, original.location)
                        .unwrap();
                    (*found == negation && pattern.predicate().name() == name)
                        .then_some((*atom, pattern))
                })
                .unwrap()
        };
        let (positive, num) = named(DefaultNegation::None, "num");
        let (_, divisor) = named(DefaultNegation::None, "divisor");
        let (negative, blocked) = named(DefaultNegation::Not, "blocked");
        let variable = |pattern: zetesis_core::PatternRef<'_>| {
            assert_eq!(pattern.terms().len(), 1);
            let Some(zetesis_core::TemplateTerm::Variable(slot)) = pattern.terms().at(0) else {
                panic!("one variable argument")
            };
            slot
        };
        let present = variable(num);
        let missing = variable(divisor);
        assert_ne!(present, missing);
        assert_eq!(variable(blocked), missing);
        let result = computation
            .static_pattern(head.unwrap(), &limits, counters, original.location)
            .unwrap();
        assert_eq!(result.predicate().name(), "result");
        assert_eq!(result.terms().len(), 2);
        for (column, slot) in [present, missing].into_iter().enumerate() {
            assert!(matches!(result.terms().at(column),
                Some(zetesis_core::TemplateTerm::Variable(found)) if found == slot));
        }
        assert_eq!(original.body_variables, 2);
        for head_only in [false, true] {
            // Retain num(X), never the incidental first normalized positive atom.
            // D is used only by blocked(D) or result(X,D), with no provider.
            let mut body = vec![LiteralIr::Atom(DefaultNegation::None, positive)];
            if !head_only {
                body.push(LiteralIr::Atom(DefaultNegation::Not, negative));
            }
            let rule = RuleIr {
                head: HeadIr::Normal(if head_only { *head } else { None }),
                body,
                body_variables: original.body_variables,
                bindings: None,
                variables: original.variables,
                origins: original.origins.clone(),
                location: original.location,
            };
            let mut join = Join::rule(
                &rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            assert!(
                !join
                    .total_family_fragment(&rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(matches!(
                join.next_owned_row(
                    computation,
                    &limits,
                    &mut testing::budget(),
                    counters,
                    rule.location,
                ),
                Err(FormulaFailure::UnsafeVariable { variable, location })
                    if variable == missing && location == rule.location
            ));
        }
    });
}

#[test]
fn unsupported_heads_keep_false_body_suppression() {
    let source =
        include_str!("../../../../tests/fixtures/arithmetic-family-totality/false-head.lp");
    let (ordinary, _) = controlled(true, None, || ground(testing::prepare(source), None, None));
    let (actual, proofs) = controlled(false, None, || ground(testing::prepare(source), None, None));
    assert_eq!(proofs, 0);
    assert_same_program(&ordinary.unwrap(), &actual.unwrap());
}

#[test]
fn successful_proof_does_not_select_evidence_rows() {
    testing::with_completed_source(TOTAL, |program, support, computation, counters| {
        let rule = partial_rule(program);
        let mut join = Join::rule(
            rule,
            support,
            computation,
            &FormulaLimits::default(),
            &mut testing::budget(),
            counters,
        )
        .unwrap();
        assert!(
            join.total_family_fragment(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        let evidence = join.take_family();
        assert!(!evidence.defined);
        assert!(evidence.zero.is_none());
        let mut rows = 0;
        while join
            .next_owned_row(
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
                rule.location,
            )
            .unwrap()
            .is_some()
        {
            rows += 1;
        }
        assert_eq!(
            rows, 10,
            "a proof does not turn this cursor into a selected family scan"
        );
    });
}

#[test]
fn total_siblings_are_each_proved_once() {
    let source =
        include_str!("../../../../tests/fixtures/arithmetic-family-totality/total-siblings.lp");
    let (ordinary, before) = scan(source, true);
    let (actual, after) = scan(source, false);
    assert_eq!(actual.unwrap(), ordinary.unwrap());
    assert_eq!(before.binding_snapshots, Some(4));
    assert_eq!(after.binding_snapshots, Some(0));
    let (ordinary, _) = controlled(true, None, || ground(testing::prepare(source), None, None));
    let (actual, proofs) = controlled(false, None, || ground(testing::prepare(source), None, None));
    assert_eq!(proofs, 2, "each sibling proved once for this original site");
    assert_same_program(&ordinary.unwrap(), &actual.unwrap());
}
