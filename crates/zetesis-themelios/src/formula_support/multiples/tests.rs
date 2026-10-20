use super::*;
use crate::ProgramSite;
use crate::formula_support::{Counters, keys, testing};
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::{FormulaLimits, GroundingPhase, GroundingWork as ObservedWork};
use zetesis_core::ValueLimits;

mod accounting;
mod delta;
mod prepared;

const DENSE: &str = include_str!("../../../tests/fixtures/divisibility-join/dense.lp");

struct Collected {
    values: Vec<Vec<Value>>,
    owned: bool,
    work: ObservedWork,
}

fn rule(program: &crate::formula_ir::Prepared) -> &RuleIr {
    program
        .rules_without_facts()
        .iter()
        .find(|rule| {
            rule.body
                .iter()
                .any(|literal| matches!(literal, LiteralIr::Compare(..)))
        })
        .unwrap()
}

fn collect(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
) -> Collected {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    counters.observed = profile.work();
    let (values, owned) = profile
        .phase(GroundingPhase::RuleInstantiation, Some(location), || {
            let mut values = Vec::new();
            let mut owned = false;
            while let Some(row) = join.next_row(
                computation,
                &FormulaLimits::default(),
                &mut testing::budget(),
                counters,
                location,
            )? {
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
                owned |= join
                    .probes
                    .iter()
                    .any(|probe| matches!(probe, Some(Probe::Owned(_))));
            }
            Ok::<_, FormulaFailure>((values, owned))
        })
        .unwrap();
    Collected {
        values,
        owned,
        work: observer.0.get(),
    }
}

fn compare(source: &str, expect_prepared: bool) -> (Collected, Collected) {
    let mut result = None;
    testing::with_completed_source(source, |program, support, computation, counters| {
        let rule = rule(program);
        let limits = FormulaLimits::default();
        let mut run = |enabled: bool| {
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            join.select_total_body(rule, computation, &limits, counters)
                .unwrap();
            assert_eq!(join.multiples.is_some(), expect_prepared);
            if !enabled {
                join.multiples = None;
            }
            collect(&mut join, computation, counters, rule.location)
        };
        let ordinary = run(false);
        let selected = run(true);
        assert_eq!(
            selected.values, ordinary.values,
            "same fixed-plan survivor order"
        );
        result = Some((ordinary, selected));
    });
    result.unwrap()
}

#[test]
fn bounded_divisor_visits_only_existing_multiples() {
    let (ordinary, selected) = compare(DENSE, true);
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 8);
    assert_eq!(ordinary.work.join_rows, Some(25));
    assert_eq!(selected.work.join_rows, Some(9));
    assert_eq!(selected.work.table_preparations, Some(0));
}

#[test]
fn full_tuple_multiplicity_and_order_are_preserved() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/repeated-values.lp"),
        true,
    );
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 12);
}

#[test]
fn repeated_columns_remain_with_the_matcher() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/repeated-columns.lp"),
        true,
    );
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 4);
}

#[test]
fn sparse_intervals_keep_the_existing_probe() {
    let (ordinary, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/sparse.lp"),
        true,
    );
    assert!(!selected.owned);
    assert_eq!(ordinary.work.join_rows, selected.work.join_rows);
}

#[test]
fn a_better_existing_posting_is_not_displaced() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/selective.lp"),
        true,
    );
    assert!(!selected.owned);
    assert_eq!(selected.values.len(), 1);
}

#[test]
fn nonpositive_columns_keep_ordinary_scalar_semantics() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/negative.lp"),
        false,
    );
    assert!(!selected.owned);
    assert_eq!(selected.values.len(), 5);
}

#[test]
fn small_positive_intervals_match_exhaustive_rows() {
    // Generated finite populations exercise every divisor, endpoint and absent
    // multiple independently of the production interval enumeration.
    for low in 1..=5 {
        for high in low..=12 {
            for divisor in 1..=15 {
                let source = format!(
                    "num({low}..{high}).divisor({divisor}). :- divisor(D),num(X),D>0,0=X\\D."
                );
                let (_, selected) = compare(&source, true);
                assert_eq!(
                    selected.values.len(),
                    (low..=high).filter(|value| value % divisor == 0).count()
                );
            }
        }
    }
}

#[test]
fn evidence_cursors_discard_the_prepared_selection() {
    testing::with_completed_source(DENSE, |program, support, computation, counters| {
        let rule = rule(program);
        let limits = FormulaLimits::default();
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
            join.select_total_body(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.multiples.is_some());
        join.evidence();
        assert!(join.multiples.is_none());
        let actual = collect(&mut join, computation, counters, rule.location);
        assert!(!actual.owned);
        assert_eq!(actual.work.join_rows, Some(25));
    });
}

#[test]
fn divisor_after_dividend_keeps_the_fixed_plan() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/unbound-order.lp"),
        false,
    );
    assert!(!selected.owned);
}

#[test]
fn additional_total_comparisons_remain_residual() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/extra-selector.lp"),
        true,
    );
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 3);
}

#[test]
fn the_largest_source_integer_needs_no_overflowing_step() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/large-endpoint.lp"),
        true,
    );
    assert!(selected.owned);
    assert_eq!(selected.values.len(), 1);
}

#[test]
fn negative_keys_require_positive_binding_coverage() {
    testing::with_completed_source(
        include_str!("../../../tests/fixtures/divisibility-join/key-coverage.lp"),
        |program, support, computation, counters| {
            let rule = rule(program);
            let limits = FormulaLimits::default();
            let mut join = Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut testing::budget(),
                counters,
            )
            .unwrap();
            // Remove one provider from this private malformed plan. The negative
            // atom still names its slot; scalar totality cannot establish key safety.
            join.owned_plan()
                .patterns
                .retain(|occurrence| occurrence.atom().predicate().name() != "optional");
            assert!(
                !keys::covered(
                    rule,
                    &join.plan,
                    &mut Context::new(&*computation, &limits, counters, rule.location)
                )
                .unwrap()
            );
        },
    );
}

#[test]
fn delta_cursors_cannot_use_completed_source_selection() {
    testing::with_completed_source(DENSE, |program, support, computation, counters| {
        let rule = rule(program);
        let limits = FormulaLimits::default();
        let mut join = Join::variant_rule(
            rule,
            crate::formula_support::delta::Variant::Delta(0),
            support,
            None,
            &mut testing::budget(),
            Context::new(&*computation, &limits, counters, rule.location),
        )
        .unwrap();
        assert!(
            !join
                .select_total_body(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.multiples.is_none());
    });
}

#[test]
fn a_computed_domain_keeps_priority_over_multiples() {
    let (_, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/computed-selector.lp"),
        true,
    );
    assert!(!selected.owned);
    assert_eq!(selected.values.len(), 1);
    assert!(selected.work.table_probes.unwrap_or(0) > 0);
}

#[test]
fn target_value_fanout_does_not_displace_a_smaller_posting() {
    // One possible multiple is fewer than the two ordinary bound-column rows,
    // but that multiple's ten tuples are more than the ordinary posting.
    let (ordinary, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/fanout.lp"),
        true,
    );
    assert!(!selected.owned);
    assert_eq!(selected.values.len(), 1);
    assert_eq!(selected.work.join_rows, ordinary.work.join_rows);
}

#[test]
fn proper_divisor_selection_composes_with_ordering() {
    let (ordinary, selected) = compare(
        include_str!("../../../tests/fixtures/divisibility-join/proper-divisors.lp"),
        true,
    );
    assert!(selected.owned);
    assert_eq!(
        selected.values.len(),
        (2..=7).map(|d| 60 / d - 1).sum::<usize>()
    );
    assert!(selected.work.join_rows < ordinary.work.join_rows);
}

#[test]
fn negative_divisors_keep_the_checked_scalar_path() {
    let (_, selected) = compare(
        "num(1..24).divisor(-3).divisible(X):-num(X),divisor(D),X\\D=0.",
        true,
    );
    assert!(!selected.owned);
    assert_eq!(selected.values.len(), 8);
}

#[test]
fn uncertified_domains_cannot_prepare_multiples() {
    for source in [
        "num(1..4).divisor(0;2). :-num(X),divisor(D),X\\D=0.",
        "num(1..4).divisor(0;2). :-num(X),divisor(D),D>0,X\\D=0.",
        "num(-2147483647-1).divisor(-1). :-num(X),divisor(D),X\\D=0.",
        "num(1;two).divisor(2). :-num(X),divisor(D),X\\D=0.",
    ] {
        testing::with_completed_source(source, |program, support, computation, counters| {
            let rule = rule(program);
            let limits = FormulaLimits::default();
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
                !join
                    .select_total_body(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(join.multiples.is_none());
        });
    }
}

mod readiness;

mod ranges;

mod composed;

mod canonical_ties;
