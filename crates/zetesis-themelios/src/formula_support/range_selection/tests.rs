//! The range route preserves ordered survivors and original-family obligations.

mod accounting;

use super::*;
use crate::formula::Compiled;
use crate::formula_ground::{ground, ground_hybrid};
use crate::formula_support::{Coverage, Probe, testing};
use crate::{ExpansionFailure, FormulaResource};
use std::cell::Cell;
use themelios_program::term::EvalError;
use zetesis_core::{Value, ValueLimits};

const CLOSED: &str = include_str!("../../../tests/fixtures/range-multiples/closed.lp");
const RESTART: &str = include_str!("../../../tests/fixtures/range-multiples/restart.lp");
const OVERFLOW: &str = include_str!("../../../tests/fixtures/range-multiples/overflow.lp");
const ZERO: &str = include_str!("../../../tests/fixtures/range-multiples/zero.lp");

#[derive(Clone, Copy, Default)]
struct Control {
    disabled: bool,
    selected: usize,
    stepped: usize,
}
thread_local! {
    static CONTROL: Cell<Control> = const { Cell::new(Control { disabled: false, selected: 0, stepped: 0 }) };
}
struct Restore(Control);
impl Drop for Restore {
    fn drop(&mut self) {
        CONTROL.set(self.0);
    }
}
pub(super) fn disabled() -> bool {
    CONTROL.get().disabled
}
pub(super) fn selected() {
    let mut control = CONTROL.get();
    control.selected += 1;
    CONTROL.set(control);
}
pub(super) fn stepped() {
    let mut control = CONTROL.get();
    control.stepped += 1;
    CONTROL.set(control);
}
fn controlled<T>(disabled: bool, run: impl FnOnce() -> T) -> (T, Control) {
    let _restore = Restore(CONTROL.replace(Control {
        disabled,
        ..Control::default()
    }));
    let result = run();
    (result, CONTROL.get())
}

fn range_rule(program: &crate::formula_ir::Prepared) -> &RuleIr {
    program
        .rules_without_facts()
        .iter()
        .find(|rule| {
            rule.body
                .iter()
                .any(|literal| matches!(literal, LiteralIr::Range { binder: true, .. }))
        })
        .unwrap()
}

struct Rows {
    values: Vec<Vec<Value>>,
    offered: usize,
    work: u64,
}
fn collect(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: crate::ProgramSite,
) -> Rows {
    let mut values = Vec::new();
    let mut offered = 0;
    let start = counters.accounting.work;
    while let Some(row) = join
        .next_row(
            computation,
            &FormulaLimits::default(),
            &mut testing::budget(),
            counters,
            location,
        )
        .unwrap()
    {
        offered += 1;
        if row.passes {
            values.push(
                (0..row.values.len())
                    .map(|slot| {
                        row.values
                            .read(slot, computation.read(), location)
                            .unwrap()
                            .to_value(ValueLimits::default())
                            .unwrap()
                    })
                    .collect(),
            );
        }
        assert!(
            !join
                .probes
                .iter()
                .any(|probe| matches!(probe, Some(Probe::Table(_))))
        );
    }
    Rows {
        values,
        offered,
        work: counters.accounting.work - start,
    }
}

fn compare(source: &str) -> (Rows, Rows, Control) {
    let mut result = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = range_rule(program);
        let mut run = |selected| {
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            if selected {
                assert!(
                    join.select_range_multiples(
                        rule,
                        computation,
                        &FormulaLimits::default(),
                        counters
                    )
                    .unwrap()
                );
                assert!(join.range_selection.is_some());
            }
            assert!(!join.certified_total);
            assert!(join.coverage == Coverage::Complete);
            collect(&mut join, computation, counters, rule.location)
        };
        let ordinary = run(false);
        let (selected, control) = controlled(false, || run(true));
        assert_eq!(
            ordinary.values, selected.values,
            "ordered full-binding survivors"
        );
        result = Some((ordinary, selected, control));
    });
    result.unwrap()
}

#[test]
fn closed_range_skips_nonmultiples_before_filtering() {
    let (ordinary, selected, control) = compare(CLOSED);
    assert_eq!(ordinary.offered, 120);
    assert_eq!(selected.offered, 32);
    assert_eq!(selected.values.len(), 32);
    assert_eq!(control.selected, 1);
    assert_eq!(
        control.stepped, 2,
        "both relational rows actually use the stepped cursor"
    );
    assert!(selected.work < ordinary.work);
}

#[test]
fn divisor_one_does_not_hide_a_selective_equality() {
    let source = "p(X) :- X=1..12,X\\1=0,X\\3=0.";
    testing::with_completed_source(source, |program, _, computation, counters| {
        let rule = range_rule(program);
        let mut divisors = Vec::new();
        let limits = FormulaLimits::default();
        let mut context = Context::new(&*computation, &limits, counters, rule.location);
        for literal in &rule.body {
            if let LiteralIr::Compare(left, Relation::Eq, right) = literal {
                for (expression, zero) in [(left, right), (right, left)] {
                    if let Some((_, Divisor::Constant(divisor))) =
                        remainder(expression, zero, &mut context).unwrap()
                    {
                        divisors.push(divisor);
                    }
                }
            }
        }
        assert_eq!(
            divisors,
            [1, 3],
            "the unit divisor must be encountered first"
        );
    });
    let (ordinary, selected, control) = compare(source);
    assert_eq!(ordinary.offered, 12);
    assert_eq!(selected.offered, 4);
    assert_eq!(
        selected.values,
        [3, 6, 9, 12].map(|value| vec![Value::Number(value)])
    );
    assert_eq!(control.selected, 1);
    assert_eq!(control.stepped, 1);
}

#[test]
fn divisor_one_declines_before_scratch_admission() {
    testing::with_completed_source(
        "p(X) :- X=1..12,X\\1=0.",
        |program, support, computation, counters| {
            let rule = range_rule(program);
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            let live = support.live_bytes();
            let limits = FormulaLimits {
                max_support_bytes: live,
                ..Default::default()
            };
            // The old route attempts covered-key/numeric scratch admission even
            // though factors() can never select a progression for this divisor.
            assert!(
                !join
                    .select_range_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(join.range_selection.is_none());
            assert_eq!(support.live_bytes(), live);
            let rows = collect(&mut join, computation, counters, rule.location);
            assert_eq!(rows.offered, 12);
            assert_eq!(
                rows.values,
                (1..=12)
                    .map(|value| vec![Value::Number(value)])
                    .collect::<Vec<_>>()
            );
        },
    );
}

#[test]
fn relational_restart_reads_each_divisor() {
    let (ordinary, selected, control) = compare(RESTART);
    assert_eq!(ordinary.offered, 36);
    assert_eq!(selected.offered, 16);
    assert_eq!(selected.values.len(), 16);
    assert_eq!(
        control.stepped, 2,
        "one keeps the plain cursor; 100 has an empty progression"
    );
}

#[test]
fn maximum_endpoint_is_not_lost_or_repeated() {
    let (_, selected, control) = compare("p(X) :- X=2147483646..2147483647,X\\2147483647=0.");
    assert_eq!(selected.values, [vec![Value::Number(i32::MAX)]]);
    assert_eq!(control.stepped, 1);
}

#[test]
fn full_range_width_is_admitted_before_selection() {
    testing::with_completed_source(CLOSED, |program, support, computation, counters| {
        let rule = range_rule(program);
        let limits = FormulaLimits {
            max_assignment_values: 2,
            ..Default::default()
        };
        let mut join = Join::rule(
            rule,
            support,
            computation,
            &limits,
            &mut testing::budget(),
            counters,
        )
        .unwrap();
        assert!(
            join.select_range_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(matches!(
            join.next_row(
                computation,
                &limits,
                &mut testing::budget(),
                counters,
                rule.location
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::AssignmentValues,
                observed: 60,
                ..
            })
        ));
    });
}

#[test]
fn full_domain_overflow_declines_the_selector() {
    testing::with_completed_source(OVERFLOW, |program, support, computation, counters| {
        let rule = range_rule(program);
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
            !join
                .select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        assert!(join.range_selection.is_none());
    });
    for disabled in [false, true] {
        let (actual, control) =
            controlled(disabled, || ground(testing::prepare(OVERFLOW), None, None));
        assert!(matches!(
            actual,
            Err(FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Overflow,
                ..
            }))
        ));
        assert_eq!(
            control.selected, 0,
            "the original family fails before selected emission"
        );
    }
}

#[test]
fn zero_containing_columns_do_not_borrow_guard_truth() {
    testing::with_completed_source(ZERO, |program, support, computation, counters| {
        let rule = range_rule(program);
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
            !join
                .select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
    });
    let (ordinary, _) = controlled(true, || ground(testing::prepare(ZERO), None, None));
    let (selected, control) = controlled(false, || ground(testing::prepare(ZERO), None, None));
    assert_same_program(&ordinary.unwrap(), &selected.unwrap());
    assert_eq!(control.selected, 0);
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
    assert_eq!(left.origins, right.origins);
}

#[test]
fn selected_emission_preserves_the_original_theory() {
    for source in [CLOSED, "p(X) :- X=1..3,X\\4=0."] {
        let (ordinary, _) = controlled(true, || ground(testing::prepare(source), None, None));
        let (selected, control) =
            controlled(false, || ground(testing::prepare(source), None, None));
        assert!(control.selected > 0);
        assert!(control.stepped > 0);
        // Identical graph/atoms imply identical original and arbitrary frozen
        // satisfaction, including the empty selected family and source origins.
        assert_same_program(&ordinary.unwrap(), &selected.unwrap());
    }
}

#[test]
fn hybrid_selects_producers_without_certifying_constraints() {
    let source = include_str!("../../../tests/fixtures/range-multiples/hybrid.lp");
    let (ordinary, _) = controlled(true, || ground_hybrid(testing::prepare(source), None));
    let (selected, control) = controlled(false, || ground_hybrid(testing::prepare(source), None));
    let (ordinary, ordinary_constraints) = ordinary.unwrap();
    let (selected, selected_constraints) = selected.unwrap();
    assert_eq!(control.selected, 1, "only the normal producer opts in");
    assert!(control.stepped > 0);
    assert!(ordinary_constraints.is_some());
    assert!(selected_constraints.is_some());
    assert_same_program(&ordinary, &selected);
}

#[test]
fn evidence_and_delta_cursors_keep_original_ranges() {
    testing::with_completed_source(CLOSED, |program, support, computation, counters| {
        let rule = range_rule(program);
        for evidence in [true, false] {
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            if evidence {
                join.evidence();
            } else {
                join.delta = Some(0);
            }
            assert!(
                !join
                    .select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                    .unwrap()
            );
            assert!(join.range_selection.is_none());
        }
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
            join.select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                .unwrap()
        );
        join.evidence();
        assert!(join.range_selection.is_none());
        assert_eq!(
            collect(&mut join, computation, counters, rule.location).offered,
            120
        );
    });
}

#[test]
fn unsupported_range_scopes_keep_the_ordinary_cursor() {
    for source in [
        "d(1). :- d(D),not a(D),X=1..3,X\\2=0.",
        "d(3). :- d(D),X=1..D,X\\2=0.",
        "d(1). :- d(D),X=1..3,Y=1..3,X\\2=0,Y>0.",
        "d(1). :- d(D),X=0..3,X\\2=0.",
        "d(1). :- d(D),X=3..1,X\\2=0.",
        "d(1). :- d(D),X=1..a,X\\2=0.",
        "d(2;a). :- d(D),X=1..3,X\\D=0.",
    ] {
        testing::with_completed_source(source, |program, support, computation, counters| {
            let rule = range_rule(program);
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
                !join
                    .select_range_multiples(rule, computation, &FormulaLimits::default(), counters)
                    .unwrap(),
                "{source}"
            );
            assert!(join.range_selection.is_none());
        });
    }
}
