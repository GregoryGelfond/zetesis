//! A round retains the existing successful map, never completed-source authority.

use super::*;
use crate::FormulaResource;
use crate::formula_support::projections::Projections;

struct RoundRun {
    rows: Vec<Vec<Value>>,
    work: u64,
    borrowed: usize,
    owned_probe: bool,
}

fn run_variants(source: &str, reuse: bool) -> RoundRun {
    let mut snapshot = Snapshot::new(source);
    snapshot.publish(population(3, 0));
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut variants =
            crate::formula_support::delta::variants(rule, support, false, &limits, counters)
                .unwrap();
        let start = counters.accounting.work;
        let initial_bytes = support.workspace_bytes();
        let mut preparation = None;
        let mut map = None;
        let mut result = RoundRun {
            rows: Vec::new(),
            work: 0,
            borrowed: 0,
            owned_probe: false,
        };
        let mut next = variants.next(&limits, counters).unwrap();
        while let Some(variant) = next {
            if let Some(preparation) = &preparation {
                let mut join = RoundPreparation::rows(
                    preparation,
                    variant,
                    None,
                    &mut testing::budget(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
                let Projections::Prepared(values) = &join.projections else {
                    panic!("later cursors must borrow the one retained map");
                };
                // Observe identity without retaining a typed pointer whose
                // ProjectionValues lifetime would extend this cursor borrow.
                // The address is compared only, never used to access storage.
                let address = std::ptr::from_ref(*values).addr();
                if let Some(previous) = map {
                    assert_eq!(
                        previous, address,
                        "the same map supplies every later variant"
                    );
                }
                map = Some(address);
                let coverage = join.coverage;
                assert!(
                    join.select_round_multiples(rule, computation, &limits, counters)
                        .unwrap()
                );
                assert!(!join.certified_total);
                assert!(join.coverage == coverage);
                let selected = collect(&mut join, computation, counters, rule.location);
                assert_eq!(selected.work.table_preparations, Some(0));
                result.borrowed += 1;
                result.owned_probe |= selected.owned;
                result.rows.extend(selected.values);
                drop(join);
                next = variants.next(&limits, counters).unwrap();
                continue;
            }
            let mut join = Join::variant_rule(
                rule,
                variant,
                support,
                None,
                &mut testing::budget(),
                Context::new(computation, &limits, counters, rule.location),
            )
            .unwrap();
            assert!(
                join.select_round_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            let selected = collect(&mut join, computation, counters, rule.location);
            result.owned_probe |= selected.owned;
            result.rows.extend(selected.values);
            next = variants.next(&limits, counters).unwrap();
            if reuse && next.is_some() {
                preparation = Some(
                    RoundPreparation::retain(
                        rule,
                        join,
                        Context::new(computation, &limits, counters, rule.location),
                    )
                    .unwrap(),
                );
            }
        }
        result.work = counters.accounting.work - start;
        if reuse {
            assert!(
                support.workspace_bytes() > initial_bytes,
                "retained storage stays admitted"
            );
        }
        drop(preparation);
        assert_eq!(
            support.workspace_bytes(),
            initial_bytes,
            "the round releases its leases"
        );
        result
    })
}

#[test]
fn variants_borrow_one_map_and_spend_less_total_work() {
    // This includes plan construction, totality, retained-owner admission and
    // full cursor traversal; moving proof work outside a measured block cannot
    // satisfy the work comparison. X/2 produces a nonempty expression map.
    let ordinary = run_variants(COMPUTED, false);
    let shared = run_variants(COMPUTED, true);
    assert_eq!(shared.rows, ordinary.rows);
    assert_eq!(shared.borrowed, 2);
    assert_eq!(ordinary.borrowed, 0);
    assert!(shared.owned_probe && ordinary.owned_probe);
    assert!(
        shared.work < ordinary.work,
        "{} must be less than {}",
        shared.work,
        ordinary.work
    );
}

fn with_preparation(
    run: impl for<'rule, 'source> FnOnce(
        &'rule RuleIr,
        &'rule Support<'source>,
        &RoundPreparation<'rule, 'source>,
        &mut Computation<'_, '_>,
        &mut Counters,
        Variant,
    ),
) {
    let mut snapshot = Snapshot::new(COMPUTED);
    snapshot.publish(population(3, 0));
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let limits = FormulaLimits::default();
        let pivot = occurrence(rule, computation, counters, true);
        let variant = Variant::Delta(pivot);
        let mut join = Join::variant_rule(
            rule,
            variant,
            support,
            None,
            &mut testing::budget(),
            Context::new(computation, &limits, counters, rule.location),
        )
        .unwrap();
        assert!(
            join.select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        collect(&mut join, computation, counters, rule.location);
        let preparation = RoundPreparation::retain(
            rule,
            join,
            Context::new(computation, &limits, counters, rule.location),
        )
        .unwrap();
        run(rule, support, &preparation, computation, counters, variant);
    });
}

#[test]
fn retained_preparation_rechecks_the_live_storage_limit() {
    with_preparation(
        |rule, _support, preparation, computation, counters, variant| {
            let limits = FormulaLimits {
                max_support_bytes: 0,
                ..Default::default()
            };
            assert!(matches!(
                preparation.rows(variant, None, &mut testing::budget(), Context::new(computation, &limits, counters, rule.location)),
                Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, location, .. }) if location == rule.location
            ));
            let limits = FormulaLimits::default();
            let mut join = preparation
                .rows(
                    variant,
                    None,
                    &mut testing::budget(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
            assert!(
                join.select_round_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(collect(&mut join, computation, counters, rule.location).owned);
        },
    );
}

#[test]
fn cancelled_borrow_retains_the_preparation_for_retry() {
    with_preparation(
        |rule, _support, preparation, computation, counters, variant| {
            let limits = FormulaLimits::default();
            let cancellation = zetesis_cpu::Cancellation::default();
            cancellation.cancel();
            counters.cancellation = Some(cancellation);
            assert!(matches!(
                preparation.rows(variant, None, &mut testing::budget(), Context::new(computation, &limits, counters, rule.location)),
                Err(FormulaFailure::Interrupted { reason: zetesis_cpu::Stop::Cancelled, location }) if location == rule.location
            ));
            counters.cancellation = None;
            let mut join = preparation
                .rows(
                    variant,
                    None,
                    &mut testing::budget(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
            assert!(
                join.select_round_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            join.evidence();
            assert!(!join.certified_total);
            assert!(join.multiples.is_none());
            assert!(!collect(&mut join, computation, counters, rule.location).owned);
        },
    );
}

#[test]
fn a_round_owner_rejects_a_different_rule() {
    let mut snapshot = Snapshot::new(FIRST);
    snapshot.publish(population(3, 0));
    snapshot.publish(population(4, 1));
    let other = testing::prepare(FIRST);
    snapshot.with(|rule, support, computation, counters| {
        let limits = FormulaLimits::default();
        let pivot = occurrence(rule, computation, counters, true);
        let mut join = Join::variant_rule(rule, Variant::Delta(pivot), support, None, &mut testing::budget(), Context::new(computation, &limits, counters, rule.location)).unwrap();
        assert!(join.select_round_multiples(rule, computation, &limits, counters).unwrap());
        let other = super::super::rule(&other.program);
        assert!(matches!(
            RoundPreparation::retain(other, join, Context::new(computation, &limits, counters, other.location)),
            Err(FormulaFailure::SupportRelation { error: zetesis_core::relation::Failure::Owner, location }) if location == other.location
        ));
    });
}

#[test]
fn each_variant_rechecks_its_own_divisor_readiness() {
    const SOURCE: &str =
        include_str!("../../../../../tests/fixtures/divisibility-join/delta-plan.lp");
    let mut snapshot = Snapshot::new(SOURCE);
    // Use canonical occurrence order here: d precedes n, independently of the
    // cardinality-based execution order that changes between these two pivots.
    for rule in snapshot.program.rules_without_facts_mut() {
        rule.body.reverse();
    }
    snapshot.publish(
        (3..=8)
            .map(|value| atom("d", &[value]))
            .chain((1..=12).map(|value| atom("n", &[value, 0]))),
    );
    snapshot.publish([atom("d", &[9]), atom("n", &[12, 1])]);
    snapshot.with(|rule, support, computation, counters| {
        let limits = FormulaLimits::default();
        let divisor = occurrence(rule, computation, counters, false);
        let target = occurrence(rule, computation, counters, true);
        assert!(divisor < target);
        let mut first = Join::variant_rule(
            rule,
            Variant::Delta(divisor),
            support,
            None,
            &mut testing::budget(),
            Context::new(computation, &limits, counters, rule.location),
        )
        .unwrap();
        assert_eq!(first.plan.patterns[0].source, divisor);
        assert!(
            first
                .select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(collect(&mut first, computation, counters, rule.location).owned);
        let preparation = RoundPreparation::retain(
            rule,
            first,
            Context::new(computation, &limits, counters, rule.location),
        )
        .unwrap();
        let ordinary = run(
            rule,
            support,
            computation,
            counters,
            Variant::Delta(target),
            false,
        );
        let mut second = preparation
            .rows(
                Variant::Delta(target),
                None,
                &mut testing::budget(),
                Context::new(computation, &limits, counters, rule.location),
            )
            .unwrap();
        assert!(matches!(second.projections, Projections::Prepared(_)));
        assert_eq!(second.plan.patterns[0].source, target);
        assert!(
            !second
                .select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(second.multiples.is_none());
        assert!(!second.certified_total);
        let selected = collect(&mut second, computation, counters, rule.location);
        assert!(!selected.owned);
        assert_eq!(selected.values, ordinary.values);
        assert_eq!(selected.work.table_preparations, Some(0));
    });
}

#[test]
fn a_stopped_borrow_publishes_no_selected_cursor() {
    with_preparation(
        |rule, support, preparation, computation, counters, variant| {
            let limits = FormulaLimits::default();
            let initial_bytes = support.workspace_bytes();
            let start = counters.accounting.work;
            let mut join = preparation
                .rows(
                    variant,
                    None,
                    &mut testing::budget(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
            assert!(
                join.select_round_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            let work = counters.accounting.work - start;
            drop(join);
            for allowance in [0, work - 1] {
                let limits = FormulaLimits {
                    max_work: counters.accounting.work + allowance,
                    ..Default::default()
                };
                let result = preparation
                    .rows(
                        variant,
                        None,
                        &mut testing::budget(),
                        Context::new(computation, &limits, counters, rule.location),
                    )
                    .and_then(|mut join| {
                        join.select_round_multiples(rule, computation, &limits, counters)?;
                        Ok(join)
                    });
                assert!(matches!(result, Err(FormulaFailure::Limit {
                resource: FormulaResource::Work, location, ..
            }) if location == rule.location));
                assert_eq!(support.workspace_bytes(), initial_bytes);
            }
            let mut retry = preparation
                .rows(
                    variant,
                    None,
                    &mut testing::budget(),
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
            assert!(
                retry
                    .select_round_multiples(rule, computation, &limits, counters)
                    .unwrap()
            );
            assert!(collect(&mut retry, computation, counters, rule.location).owned);
        },
    );
}

fn derive(source: &str, shared: bool) -> (BTreeSet<Atom>, u64) {
    let mut snapshot = Snapshot::new(source);
    snapshot.publish(population(3, 0));
    snapshot.publish(population(4, 1));
    let limits = FormulaLimits::default();
    let start = snapshot.counters.accounting.work;
    {
        let (relations, mut append) = snapshot
            .catalog
            .split(&limits, &mut snapshot.counters, snapshot.location)
            .unwrap();
        let support =
            Support::indexed(&relations, &limits, &snapshot.counters, snapshot.location).unwrap();
        let rule = super::super::rule(&snapshot.program);
        let counters = &mut snapshot.counters;
        {
            let mut computation = Computation::new(&mut append, &support);
            let support = &support;
            let computation = &mut computation;
            let mut variants =
                crate::formula_support::delta::variants(rule, support, false, &limits, counters)
                    .unwrap();
            let bytes = support.workspace_bytes();
            let mut budget = testing::budget();
            if shared {
                // Call the real support-production consumer, not the local helper
                // used above to inspect individual borrowed cursors.
                crate::formula_support::derive_variants(
                    rule,
                    &mut variants,
                    None,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, rule.location),
                )
                .unwrap();
            } else {
                while let Some(variant) = variants.next(&limits, counters).unwrap() {
                    let mut join = Join::variant_rule(
                        rule,
                        variant,
                        support,
                        None,
                        &mut budget,
                        Context::new(computation, &limits, counters, rule.location),
                    )
                    .unwrap();
                    assert!(
                        join.select_round_multiples(rule, computation, &limits, counters)
                            .unwrap()
                    );
                    crate::formula_support::derive_rule(
                        rule,
                        &mut join,
                        support,
                        computation,
                        &limits,
                        &mut budget,
                        counters,
                    )
                    .unwrap();
                }
            }
            assert_eq!(support.workspace_bytes(), bytes);
        }
        drop(support);
        // The real round boundary orders pending discoveries only after every
        // cursor releases its snapshot, then catalog publication admits them.
        assert!(
            crate::formula_support::advance_round(
                &mut append,
                None,
                GroundingWork::new(&limits, counters, rule.location)
            )
            .unwrap()
        );
    }
    snapshot
        .catalog
        .publish(&limits, &mut snapshot.counters, snapshot.location)
        .unwrap();
    let work = snapshot.counters.accounting.work - start;
    let heads = snapshot.with(|_, support, _, _| {
        support
            .source_atoms()
            .filter(|(predicate, _)| predicate.name() == "out")
            .flat_map(|(_, atoms)| atoms.iter())
            .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
            .collect()
    });
    (heads, work)
}

#[test]
fn possible_head_production_uses_the_shared_preparation() {
    let (ordinary, ordinary_work) = derive(COMPUTED, false);
    let (shared, shared_work) = derive(COMPUTED, true);
    assert!(!ordinary.is_empty());
    assert_eq!(shared, ordinary);
    assert!(
        shared_work < ordinary_work,
        "{shared_work} must be less than {ordinary_work}"
    );
}

#[test]
fn a_single_variant_creates_no_unused_owner() {
    let source = include_str!("../../../../../tests/fixtures/divisibility-join/delta-tiny.lp");
    let ordinary = derive(source, false);
    let shared = derive(source, true);
    assert!(!ordinary.0.is_empty());
    assert_eq!(
        shared, ordinary,
        "same heads and work without a later consumer"
    );
}
