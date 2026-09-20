//! The actual schedule preserves source occurrences and bootstrap obligations.

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};

use super::{Variant, variants};
use crate::expansion::Budget;
use crate::formula_ir::{HeadIr, HeadLiteral, HeadOperand, LiteralIr, RuleIr};
use crate::formula_support::{Counters, Join, Support, SupportCatalog};
use crate::{ExpansionLimits, FormulaLimits};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn pattern(name: &str, slots: &[usize]) -> AtomPattern {
    AtomPattern::new(
        Predicate::new(name, slots.len()).unwrap(),
        slots.iter().copied().map(Term::Variable).collect(),
    )
    .unwrap()
}

fn literal(negation: DefaultNegation, name: &str, slots: &[usize]) -> LiteralIr {
    LiteralIr::Atom(negation, pattern(name, slots))
}

fn rule(body: Vec<LiteralIr>, slots: usize) -> RuleIr {
    RuleIr {
        head: HeadIr::Normal(Some(pattern("r", &(0..slots).collect::<Vec<_>>()))),
        body,
        body_variables: slots,
        bindings: None,
        variables: slots,
        origins: vec![location()],
        location: location(),
    }
}

fn catalog() -> SupportCatalog {
    let mut catalog = SupportCatalog::default();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    for (index, value) in [Value::Number(7), Value::String("7".into())]
        .into_iter()
        .enumerate()
    {
        if index == 1 {
            catalog.advance(&limits, &mut counters, location()).unwrap();
        }
        catalog = catalog
            .insert(
                Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
    }
    catalog
}

fn selected(rule: &RuleIr, support: &Support<'_>, pivot: Option<usize>) -> Vec<Vec<Value>> {
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut counters = Counters::default();
    let mut join = Join::rule(rule, support, &mut budget).unwrap();
    let variant = pivot.map_or(super::Variant::Full, super::Variant::Delta);
    join.partition(variant, &mut budget, location()).unwrap();
    let mut bindings = Vec::new();
    while let Some(binding) = join
        .next(
            &FormulaLimits::default(),
            &mut budget,
            &mut counters,
            location(),
        )
        .unwrap()
    {
        bindings.push(
            (0..rule.variables)
                .map(|slot| binding.read(slot, location()).unwrap().clone())
                .collect(),
        );
    }
    bindings
}

#[test]
fn noninputs_preserve_disjoint_source_occurrences() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let rule = rule(
        vec![
            literal(DefaultNegation::Not, "absent", &[0]),
            literal(DefaultNegation::None, "p", &[0]),
            literal(DefaultNegation::NotNot, "absent", &[1]),
            literal(DefaultNegation::None, "p", &[1]),
        ],
        2,
    );
    let mut schedule = variants(&rule, &support, false, &limits, &mut counters).unwrap();
    let mut pivots = Vec::new();
    let mut actual = Vec::new();
    while let Some(variant) = schedule.next(&limits, &mut counters).unwrap() {
        let Variant::Delta(pivot) = variant else {
            panic!("flat negative non-inputs must retain the delta schedule");
        };
        pivots.push(pivot);
        actual.extend(selected(&rule, &support, Some(pivot)));
    }
    assert_eq!(pivots, [1, 3]);
    assert_eq!(actual.len(), 3);
    actual.sort();
    assert!(actual.windows(2).all(|pair| pair[0] != pair[1]));
    let mut expected = selected(&rule, &support, None);
    assert_eq!(expected.len(), 4);
    expected.retain(|row| row != &[Value::Number(7), Value::Number(7)]);
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn negative_only_producers_require_only_bootstrap() {
    let catalog = SupportCatalog::default();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    for negation in [DefaultNegation::Not, DefaultNegation::NotNot] {
        let rule = rule(vec![literal(negation, "absent", &[])], 0);
        let mut first = variants(&rule, &support, true, &limits, &mut counters).unwrap();
        assert!(matches!(
            first.next(&limits, &mut counters).unwrap(),
            Some(Variant::Full)
        ));
        assert!(first.next(&limits, &mut counters).unwrap().is_none());
        let mut later = variants(&rule, &support, false, &limits, &mut counters).unwrap();
        assert!(later.next(&limits, &mut counters).unwrap().is_none());
        assert_eq!(selected(&rule, &support, None), [Vec::<Value>::new()]);
    }
}

#[test]
fn disjunctive_producers_retain_complete_rounds() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut rule = rule(vec![literal(DefaultNegation::None, "p", &[0])], 1);
    rule.head = HeadIr::Disjunction(
        ["r", "s"]
            .map(|name| HeadLiteral {
                negation: DefaultNegation::None,
                operand: HeadOperand::Atom(pattern(name, &[0])),
            })
            .into(),
    );
    let mut schedule = variants(&rule, &support, false, &limits, &mut counters).unwrap();
    assert!(matches!(
        schedule.next(&limits, &mut counters).unwrap(),
        Some(Variant::Full)
    ));
    assert!(schedule.next(&limits, &mut counters).unwrap().is_none());
    assert_eq!(selected(&rule, &support, None).len(), 2);
}
