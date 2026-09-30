//! The actual schedule preserves source occurrences and bootstrap obligations.

use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits};

use super::{Variant, variants};
use crate::expansion::Budget;
use crate::formula_ir::{HeadIr, HeadLiteral, HeadOperand, LiteralIr, RuleIr};
use crate::formula_support::components::Pattern;
use crate::formula_support::{Computation, Counters, Join, Support, testing::Fixture};
use crate::test_support::location;
use crate::{ExpansionLimits, FormulaLimits};

fn pattern(fixture: &mut Fixture, name: &str, slots: &[usize]) -> Pattern {
    let owned = AtomPattern::new(
        Predicate::new(name, slots.len()).unwrap(),
        slots.iter().copied().map(Term::Variable).collect(),
    )
    .unwrap();
    fixture.pattern(&owned, location())
}

fn literal(
    fixture: &mut Fixture,
    negation: DefaultNegation,
    name: &str,
    slots: &[usize],
) -> LiteralIr {
    LiteralIr::Atom(negation, pattern(fixture, name, slots))
}

fn rule(fixture: &mut Fixture, body: Vec<LiteralIr>, slots: usize) -> RuleIr {
    RuleIr {
        head: HeadIr::Normal(Some(pattern(fixture, "r", &(0..slots).collect::<Vec<_>>()))),
        body,
        body_variables: slots,
        bindings: None,
        variables: slots,
        origins: vec![location()],
        location: location(),
    }
}

fn catalog() -> Fixture {
    Fixture::from_atoms(
        [Value::Number(7), Value::String("7".into())]
            .map(|value| Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()),
        location(),
    )
}

fn selected(
    rule: &RuleIr,
    support: &Support<'_>,
    pivot: Option<usize>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
) -> Vec<Vec<Value>> {
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
    let variant = pivot.map_or(super::Variant::Full, super::Variant::Delta);
    join.partition(
        variant,
        computation,
        &limits,
        &mut budget,
        counters,
        location(),
    )
    .unwrap();
    let mut bindings = Vec::new();
    while let Some(binding) = join
        .next(computation, &limits, &mut budget, counters, location())
        .unwrap()
    {
        bindings.push(
            (0..rule.variables)
                .map(|slot| {
                    binding
                        .read(slot, computation.read(), location())
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap()
                })
                .collect(),
        );
    }
    bindings
}

#[test]
fn noninputs_preserve_disjoint_source_occurrences() {
    let mut fixture = catalog();
    let body = vec![
        literal(&mut fixture, DefaultNegation::Not, "absent", &[0]),
        literal(&mut fixture, DefaultNegation::None, "p", &[0]),
        literal(&mut fixture, DefaultNegation::NotNot, "absent", &[1]),
        literal(&mut fixture, DefaultNegation::None, "p", &[1]),
    ];
    let rule = rule(&mut fixture, body, 2);
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        let mut schedule = variants(&rule, support, false, &limits, counters).unwrap();
        let mut pivots = Vec::new();
        let mut actual = Vec::new();
        while let Some(variant) = schedule.next(&limits, counters).unwrap() {
            let Variant::Delta(pivot) = variant else {
                panic!("flat negative non-inputs must retain the delta schedule");
            };
            pivots.push(pivot);
            actual.extend(selected(&rule, support, Some(pivot), computation, counters));
        }
        assert_eq!(pivots, [1, 3]);
        assert_eq!(actual.len(), 3);
        actual.sort();
        assert!(actual.windows(2).all(|pair| pair[0] != pair[1]));
        let mut expected = selected(&rule, support, None, computation, counters);
        assert_eq!(expected.len(), 4);
        expected.retain(|row| row != &[Value::Number(7), Value::Number(7)]);
        expected.sort();
        assert_eq!(actual, expected);
    });
}

#[test]
fn negative_only_producers_require_only_bootstrap() {
    let mut fixture = Fixture::default();
    let rules = [DefaultNegation::Not, DefaultNegation::NotNot].map(|negation| {
        let body = vec![literal(&mut fixture, negation, "absent", &[])];
        rule(&mut fixture, body, 0)
    });
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        for rule in &rules {
            let mut first = variants(rule, support, true, &limits, counters).unwrap();
            assert!(matches!(
                first.next(&limits, counters).unwrap(),
                Some(Variant::Full)
            ));
            assert!(first.next(&limits, counters).unwrap().is_none());
            let mut later = variants(rule, support, false, &limits, counters).unwrap();
            assert!(later.next(&limits, counters).unwrap().is_none());
            assert_eq!(
                selected(rule, support, None, computation, counters),
                [Vec::<Value>::new()]
            );
        }
    });
}

#[test]
fn disjunctive_producers_retain_complete_rounds() {
    let mut fixture = catalog();
    let body = vec![literal(&mut fixture, DefaultNegation::None, "p", &[0])];
    let mut rule = rule(&mut fixture, body, 1);
    rule.head = HeadIr::Disjunction(
        ["r", "s"]
            .map(|name| HeadLiteral {
                negation: DefaultNegation::None,
                operand: HeadOperand::Atom(pattern(&mut fixture, name, &[0])),
            })
            .into(),
    );
    let limits = FormulaLimits::default();
    fixture.with(location(), |support, computation, counters| {
        let mut schedule = variants(&rule, support, false, &limits, counters).unwrap();
        assert!(matches!(
            schedule.next(&limits, counters).unwrap(),
            Some(Variant::Full)
        ));
        assert!(schedule.next(&limits, counters).unwrap().is_none());
        assert_eq!(
            selected(&rule, support, None, computation, counters).len(),
            2
        );
    });
}
