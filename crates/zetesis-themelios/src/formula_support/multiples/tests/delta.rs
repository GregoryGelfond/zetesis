//! Snapshot-local selection retains the original first-new partition.

use std::collections::BTreeSet;

use super::*;
use crate::formula_support::producers::Registration;
use crate::formula_support::{SupportCatalog, delta::Variant};
use zetesis_core::{Atom, Predicate};

/// The registration of `rule` alone, prepared under its own counters so the
/// measured counters see only the variants' work.
fn registered<'a>(
    rule: &'a crate::formula_ir::RuleIr,
    support: &crate::formula_support::Support<'_>,
) -> Registration<'a> {
    Registration::prepare(
        std::slice::from_ref(rule),
        support.components(),
        0,
        &FormulaLimits::default(),
        &mut crate::formula_support::Counters::default(),
        rule.location,
    )
    .unwrap()
}

const FIRST: &str = include_str!("../../../../tests/fixtures/divisibility-join/delta-first.lp");
const LAST: &str = include_str!("../../../../tests/fixtures/divisibility-join/delta-last.lp");
const COMPUTED: &str =
    include_str!("../../../../tests/fixtures/divisibility-join/delta-computed.lp");

struct Snapshot {
    program: crate::formula_ir::Prepared,
    catalog: SupportCatalog,
    counters: Counters,
    location: ProgramSite,
}

impl Snapshot {
    fn new(source: &str) -> Self {
        let mut preparation = testing::prepare(source);
        // Conjunction order is immaterial for these flat, covered bodies. Give
        // the cursor a noncanonical occurrence order so its partition cannot
        // accidentally rely on the chosen execution depth or predicate order.
        for rule in preparation.program.rules_without_facts_mut() {
            rule.body.reverse();
        }
        Self {
            program: preparation.program,
            catalog: preparation.catalog,
            counters: Counters::resume(
                preparation.accounting,
                crate::grounding_observer::Work::default(),
            ),
            location: preparation.location,
        }
    }

    fn publish(&mut self, atoms: impl IntoIterator<Item = Atom>) {
        let limits = FormulaLimits::default();
        {
            let (_, mut append) = self
                .catalog
                .split(&limits, &mut self.counters, self.location)
                .unwrap();
            for atom in atoms {
                append
                    .atom((&atom).into(), &limits, &mut self.counters, self.location)
                    .unwrap();
            }
        }
        self.catalog
            .publish(&limits, &mut self.counters, self.location)
            .unwrap();
    }

    fn with<T>(
        &mut self,
        action: impl FnOnce(&RuleIr, &Support<'_>, &mut Computation<'_, '_>, &mut Counters) -> T,
    ) -> T {
        let limits = FormulaLimits::default();
        let (relations, mut append) = self
            .catalog
            .split(&limits, &mut self.counters, self.location)
            .unwrap();
        let support = Support::indexed(&relations, &limits, &self.counters, self.location).unwrap();
        let mut computation = Computation::new(&mut append, &support);
        action(
            rule(&self.program),
            &support,
            &mut computation,
            &mut self.counters,
        )
    }
}

fn atom(name: &str, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, values.len()).unwrap(),
        testing::numbers(values),
    )
    .unwrap()
}

fn occurrence(
    rule: &RuleIr,
    computation: &Computation<'_, '_>,
    counters: &mut Counters,
    dividend: bool,
) -> usize {
    let limits = FormulaLimits::default();
    let (slot, _) = comparison(
        rule,
        &mut Context::new(computation, &limits, counters, rule.location),
    )
    .unwrap()
    .unwrap();
    for (occurrence, literal) in rule.body.iter().enumerate() {
        let LiteralIr::Atom(themelios_program::program::DefaultNegation::None, atom) = literal
        else {
            continue;
        };
        let pattern = computation
            .static_pattern(*atom, &limits, counters, rule.location)
            .unwrap();
        if if dividend {
            (0..pattern.terms().len()).any(|column| matches!(pattern.terms().at(column), Some(TemplateTerm::Variable(variable)) if variable == slot))
        } else {
            pattern.predicate().name() == "d"
        } {
            return occurrence;
        }
    }
    panic!("fixture contains the requested positive occurrence");
}

fn population(divisor: i32, tag: i32) -> Vec<Atom> {
    std::iter::once(atom("d", &[divisor]))
        .chain((1..=12).map(|value| atom("n", &[value, tag])))
        .collect()
}

fn run(
    rule: &RuleIr,
    support: &Support<'_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    variant: Variant,
    selected: bool,
) -> Collected {
    let limits = FormulaLimits::default();
    let mut join = Join::variant_rule(
        rule,
        variant,
        support,
        None,
        &mut testing::budget(),
        Context::new(computation, &limits, counters, rule.location),
    )
    .unwrap();
    if selected {
        let coverage = join.coverage;
        assert!(
            join.select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(!join.certified_total);
        assert!(join.coverage == coverage);
        let target = join.multiples.unwrap().depth;
        // The planner binds the smaller divisor relation first. The deliberately
        // reversed body ensures execution is not source-occurrence order.
        assert!(
            !join
                .plan
                .patterns
                .iter()
                .map(|pattern| pattern.source)
                .is_sorted()
        );
        assert!(target > 0);
    }
    let result = collect(&mut join, computation, counters, rule.location);
    if selected {
        assert_eq!(result.work.table_preparations, Some(0));
    }
    result
}

fn partition(source: &str) -> BTreeSet<std::cmp::Ordering> {
    let mut snapshot = Snapshot::new(source);
    snapshot.publish(population(3, 0));
    let old = snapshot.with(|rule, support, computation, counters| {
        run(rule, support, computation, counters, Variant::Full, false).values
    });
    let old: BTreeSet<_> = old.into_iter().collect();
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let full = run(rule, support, computation, counters, Variant::Full, false).values;
        let expected: BTreeSet<_> = full.into_iter().filter(|row| !old.contains(row)).collect();
        let registration = registered(rule, support);
        let mut variants = registration
            .variants(0, rule, support, false, &FormulaLimits::default(), counters)
            .unwrap();
        let mut all = Vec::new();
        let target = occurrence(rule, computation, counters, true);
        let mut pivots = Vec::new();
        let mut positions = BTreeSet::new();
        let mut owned = false;
        let mut ordinary_work = 0;
        let mut selected_work = 0;
        while let Some(variant) = variants.next(&FormulaLimits::default(), counters).unwrap() {
            let Variant::Delta(pivot) = variant else {
                panic!("flat producer delta");
            };
            pivots.push(pivot);
            positions.insert(target.cmp(&pivot));
            let ordinary = run(rule, support, computation, counters, variant, false);
            let selected = run(rule, support, computation, counters, variant, true);
            assert_eq!(
                selected.values, ordinary.values,
                "ordered survivors at pivot {pivot}"
            );
            ordinary_work += ordinary.work.join_rows.unwrap();
            selected_work += selected.work.join_rows.unwrap();
            owned |= selected.owned;
            all.extend(selected.values);
        }
        assert_eq!(pivots.len(), 3);
        assert!(owned, "an actual leased multiple posting must run");
        assert!(selected_work < ordinary_work);
        let distinct: BTreeSet<_> = all.iter().cloned().collect();
        assert_eq!(distinct.len(), all.len(), "first-new variants are disjoint");
        assert_eq!(
            distinct, expected,
            "all current bindings minus all-old bindings"
        );
        positions
    })
}

#[test]
fn delta_multiples_preserve_first_new_occurrences() {
    // Target before/at/after the pivot; one repeated predicate has value
    // postings spanning both rounds and distinct full tuples per value.
    let positions: BTreeSet<_> = [FIRST, LAST].into_iter().flat_map(partition).collect();
    assert_eq!(
        positions,
        BTreeSet::from([
            std::cmp::Ordering::Less,
            std::cmp::Ordering::Equal,
            std::cmp::Ordering::Greater,
        ]),
        "the selected occurrence must exercise old, new and unrestricted intervals"
    );
}

#[test]
fn delta_multiples_do_not_enable_computed_tables() {
    partition(COMPUTED);
}

#[test]
fn a_later_zero_divisor_declines_snapshot_selection() {
    let mut snapshot = Snapshot::new(FIRST);
    snapshot.publish(population(3, 0));
    // An interior first-new pivot needs old rows at earlier occurrences.
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let pivot = occurrence(rule, computation, counters, true);
        let result = run(
            rule,
            support,
            computation,
            counters,
            Variant::Delta(pivot),
            true,
        );
        assert!(result.owned);
    });
    snapshot.publish([atom("d", &[0])]);
    snapshot.with(|rule, support, computation, counters| {
        let pivot = occurrence(rule, computation, counters, false);
        let limits = FormulaLimits::default();
        let mut join = Join::variant_rule(
            rule,
            Variant::Delta(pivot),
            support,
            None,
            &mut testing::budget(),
            Context::new(computation, &limits, counters, rule.location),
        )
        .unwrap();
        assert!(
            !join
                .select_round_multiples(rule, computation, &limits, counters)
                .unwrap()
        );
        assert!(join.multiples.is_none());
        assert!(!join.certified_total);
        let selected = collect(&mut join, computation, counters, rule.location);
        let evidence = join.take_family();
        assert!(!evidence.defined);
        let zero = evidence
            .zero
            .expect("the new snapshot retains zero-divisor evidence");
        assert_eq!(zero.site(), Some(rule.location));
        let ordinary = run(
            rule,
            support,
            computation,
            counters,
            Variant::Delta(pivot),
            false,
        );
        assert_eq!(selected.values, ordinary.values);
        assert!(!selected.owned);
    });
}

#[test]
fn tiny_delta_intervals_keep_the_ordinary_probe() {
    let mut snapshot = Snapshot::new(include_str!(
        "../../../../tests/fixtures/divisibility-join/delta-tiny.lp"
    ));
    snapshot.publish(population(3, 0));
    snapshot.publish([atom("n", &[12, 1])]);
    snapshot.with(|rule, support, computation, counters| {
        let pivot = occurrence(rule, computation, counters, true);
        let ordinary = run(
            rule,
            support,
            computation,
            counters,
            Variant::Delta(pivot),
            false,
        );
        let limits = FormulaLimits::default();
        let mut join = Join::variant_rule(
            rule,
            Variant::Delta(pivot),
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
        assert_eq!(join.multiples.unwrap().depth, 0);
        assert!(!join.certified_total);
        let selected = collect(&mut join, computation, counters, rule.location);
        assert!(
            !selected.owned,
            "one new target row beats generating four multiples"
        );
        assert_eq!(selected.values, ordinary.values);
        assert_eq!(selected.work.join_rows, ordinary.work.join_rows);
    });
}

#[test]
fn failed_snapshot_proofs_keep_independent_overflow() {
    let mut snapshot = Snapshot::new(include_str!(
        "../../../../tests/fixtures/divisibility-join/delta-fatal.lp"
    ));
    snapshot.publish(population(3, 0));
    // An interior first-new pivot needs old rows at earlier occurrences.
    snapshot.publish(population(4, 1));
    snapshot.with(|rule, support, computation, counters| {
        let pivot = occurrence(rule, computation, counters, true);
        for selected in [false, true] {
            let limits = FormulaLimits::default();
            let mut join = Join::variant_rule(rule, Variant::Delta(pivot), support, None, &mut testing::budget(), Context::new(computation, &limits, counters, rule.location)).unwrap();
            if selected {
                assert!(!join.select_round_multiples(rule, computation, &limits, counters).unwrap());
                assert!(join.multiples.is_none());
            }
            assert!(matches!(
                join.next_row(computation, &limits, &mut testing::budget(), counters, rule.location),
                Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Overflow,
                    location,
                })) if location == rule.location
            ));
        }
    });
}

mod accounting;
mod round;

mod ranges;
