//! Compare canonical provenance and the separate constraint-first Builder boundary.

use super::*;
use crate::formula_binding::Binding;
use crate::formula_facts::Item;
use crate::formula_ground::{Builder, Purpose, Schedule};
use crate::formula_ir::{HeadIr, Prepared};
use crate::formula_support::Support;
use crate::grounding_observer::Profile;
use crate::{ExpansionLimits, FormulaLimits, ProgramSite};
use std::collections::BTreeMap;
use zetesis_core::ValueLimits;
use zetesis_test_support::programs::unary;

const SOURCE: &str =
    "seed. p(0,a). p(1,a). p(1,b). :- p(K,X),p(K,Y),g(K),X!=Y. g(0) :- seed. g(1) :- seed.";

#[derive(Clone, Copy)]
enum Order {
    Canonical,
    ConstraintFirst,
}

struct Origins {
    first: BTreeMap<Atom, ProgramSite>,
    producers: BTreeMap<Atom, ProgramSite>,
    constraint: ProgramSite,
    counts: Counts,
}

fn producer_sites(
    program: &Prepared,
    builder: &mut Builder<'_, '_, '_>,
) -> BTreeMap<Atom, ProgramSite> {
    let mut producers = BTreeMap::new();
    for rule in program.rules_without_facts() {
        let HeadIr::Normal(Some(head)) = rule.head else {
            continue;
        };
        assert_eq!(rule.variables, 0, "this fixture has only ground producers");
        let binding = Binding::new(
            builder.computation,
            builder.limits,
            builder.counters,
            rule.location,
        )
        .unwrap();
        let pattern = builder
            .computation
            .static_pattern(head, builder.limits, builder.counters, rule.location)
            .unwrap();
        let identity = builder
            .computation
            .atom(
                pattern,
                &binding,
                builder.limits,
                builder.counters,
                rule.location,
            )
            .unwrap();
        let atom = builder
            .computation
            .source_atom(&identity, builder.limits, builder.counters, rule.location)
            .unwrap()
            .to_atom(ValueLimits::default())
            .unwrap();
        assert!(producers.insert(atom, rule.location).is_none());
    }
    producers
}

fn emit<'source>(
    order: Order,
    program: &'source Prepared,
    builder: &mut Builder<'_, '_, 'source>,
    support: &Support<'source>,
) {
    match order {
        Order::Canonical => {
            builder
                .instantiate_items(
                    program.items(),
                    None,
                    support,
                    &Profile::new(None),
                    Schedule::Eager(None),
                )
                .unwrap();
        }
        Order::ConstraintFirst => {
            // This deliberately exercises the Builder boundary, not compiler
            // scheduling. The Program's canonical Head order puts literal
            // producers before Falsum constraints, irrespective of text order.
            let (constraint, rule) = program
                .rules_without_facts()
                .iter()
                .enumerate()
                .find(|(_, rule)| matches!(rule.head, HeadIr::Normal(None)))
                .unwrap();
            builder
                .instantiate_rule(rule, constraint, None, support, Schedule::Eager(None))
                .unwrap();
            for item in program.items().iter() {
                match item {
                    Item::Rule(index, rule) if index != constraint => builder
                        .instantiate_rule(rule, index, None, support, Schedule::Eager(None))
                        .unwrap(),
                    Item::Facts(block) => builder.facts(block).unwrap(),
                    Item::Rule(..) => {}
                }
            }
        }
    }
}

fn first_sources(enabled: bool, order: Order) -> Origins {
    let mut result = None;
    let ((), counts) = route::scoped(enabled, || {
        testing::with_completed_source(SOURCE, |program, support, computation, counters| {
            let constraint = program
                .rules_without_facts()
                .iter()
                .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
                .unwrap()
                .location;
            let limits = FormulaLimits::default();
            let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut builder = Builder::empty(
                computation,
                &limits,
                &mut budget,
                counters,
                Purpose::Theory,
                None,
                constraint,
            )
            .unwrap();
            builder.initialize(constraint).unwrap();
            // Collection records fixed identities without naming formula atoms.
            builder.collect_fixed_facts(program.items()).unwrap();
            emit(order, program, &mut builder, support);
            let mut first = BTreeMap::new();
            for index in 0..builder.catalog.len() {
                let atom = builder
                    .catalog
                    .get(
                        index,
                        builder.computation,
                        builder.counters,
                        &limits,
                        constraint,
                    )
                    .unwrap()
                    .to_atom(ValueLimits::default())
                    .unwrap();
                assert!(
                    first
                        .insert(atom, builder.metadata.location(index))
                        .is_none()
                );
            }
            result = Some((first, producer_sites(program, &mut builder), constraint));
        });
    });
    let (first, producers, constraint) = result.unwrap();
    Origins {
        first,
        producers,
        constraint,
        counts,
    }
}

fn same_sources(order: Order) -> Origins {
    let actual = first_sources(true, order);
    let original = first_sources(false, order);
    assert_eq!(actual.counts.accepted, 1);
    assert_eq!(actual.counts.groups, 1);
    assert_eq!(original.counts, Counts::default());
    assert_eq!(actual.first, original.first);
    assert_eq!(actual.producers, original.producers);
    assert_eq!(actual.constraint, original.constraint);
    actual
}

#[test]
fn canonical_items_preserve_each_atoms_first_source() {
    let actual = same_sources(Order::Canonical);
    for key in [0, 1] {
        let atom = unary("g", key);
        assert_eq!(actual.first[&atom], actual.producers[&atom]);
    }
}

#[test]
fn constraint_before_producers_preserves_singleton_origin() {
    let actual = same_sources(Order::ConstraintFirst);
    assert_eq!(
        actual.first[&unary("g", 1)],
        actual.constraint,
        "the two-lane fiber names its common atom in the constraint"
    );
    assert_eq!(
        actual.first[&unary("g", 0)],
        actual.producers[&unary("g", 0)],
        "the singleton fiber waits for its actual producer"
    );
}
