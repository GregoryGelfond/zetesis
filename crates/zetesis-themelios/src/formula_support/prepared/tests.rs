use super::*;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation};
use crate::formula_support::SupportCatalog;
use crate::{ConstraintAllowance, ConstraintCheckLimits, ExpansionLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};
use zetesis_cpu::Cancellation;

fn fixture() -> (CompletedCatalog, RuleIr) {
    let location = Location {
        source: SourceId::new(81),
        span: Span::empty(ByteOffset::new(7)),
    };
    let mut catalog = SupportCatalog::default();
    for (name, value) in [("p", 1), ("p", 2), ("q", 2)] {
        catalog = catalog
            .insert(
                Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(value)]).unwrap(),
                &FormulaLimits::default(),
                &mut Counters::default(),
                location,
            )
            .unwrap();
    }
    let variable = |slot| Expression {
        nodes: vec![Operation::Variable(slot)],
    };
    let mut body: Vec<_> = [("p", 0), ("q", 1)]
        .into_iter()
        .map(|(name, slot)| {
            LiteralIr::Atom(
                DefaultNegation::None,
                AtomPattern::new(Predicate::new(name, 1).unwrap(), vec![Term::Variable(slot)])
                    .unwrap(),
            )
        })
        .collect();
    body.push(LiteralIr::Compare(variable(0), Relation::Lt, variable(1)));
    (
        CompletedCatalog { catalog },
        RuleIr {
            head: HeadIr::Normal(None),
            body,
            body_variables: 2,
            bindings: None,
            variables: 2,
            origins: vec![location],
            location,
        },
    )
}

fn budget() -> Budget {
    Budget::new(ExpansionLimits::default(), usize::MAX)
}

#[test]
fn prepared_rows_restart_without_replanning() {
    let (catalog, rule) = fixture();
    let limits = FormulaLimits::default();
    let mut completed = catalog
        .snapshot(&limits, &mut Counters::default(), rule.location)
        .unwrap();
    let prepared = PreparedRule::new(
        &rule,
        &mut completed,
        &limits,
        &mut budget(),
        &mut Counters::default(),
    )
    .unwrap();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &Counters::default(),
            rule.location,
        )
        .unwrap();
    // No planning work is available: reconstructing the ordinary plan would
    // fail on its first pattern argument, independently of the row evaluator.
    let mut no_planning = Budget::new(
        ExpansionLimits {
            max_term_work: 0,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    );
    assert!(Join::rule(&rule, queries.support(), &mut no_planning).is_err());
    for _ in 0..2 {
        let mut rows = prepared.rows(&queries, None, &mut no_planning).unwrap();
        let row = rows
            .next_row(
                &limits,
                &mut budget(),
                &mut Counters::default(),
                rule.location,
            )
            .unwrap()
            .unwrap();
        assert!(row.passes);
        assert_eq!(
            row.values.slots(),
            &[Some(Value::Number(1)), Some(Value::Number(2))]
        );
        // Drop an unfinished cursor. The next cursor starts at the same row.
    }
}

#[test]
fn prepared_rows_refuse_an_equal_foreign_catalog() {
    let (catalog, rule) = fixture();
    let (foreign, _) = fixture();
    let limits = FormulaLimits::default();
    let mut completed = catalog
        .snapshot(&limits, &mut Counters::default(), rule.location)
        .unwrap();
    let prepared = PreparedRule::new(
        &rule,
        &mut completed,
        &limits,
        &mut budget(),
        &mut Counters::default(),
    )
    .unwrap();
    let other = foreign
        .snapshot(&limits, &mut Counters::default(), rule.location)
        .unwrap();
    let queries = other
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &Counters::default(),
            rule.location,
        )
        .unwrap();
    assert!(
        matches!(prepared.rows(&queries, None, &mut budget()), Err(FormulaFailure::SupportRelation { error: zetesis_core::relation::Failure::Owner, location }) if location == rule.location)
    );
}

#[test]
fn refused_preparation_retains_no_plan_storage() {
    let (catalog, rule) = fixture();
    let limits = FormulaLimits::default();
    let mut completed = catalog
        .snapshot(&limits, &mut Counters::default(), rule.location)
        .unwrap();
    let before = completed.relations.bytes;
    let mut counters = Counters::default();
    let mut planning = budget();
    let prepared =
        PreparedRule::new(&rule, &mut completed, &limits, &mut planning, &mut counters).unwrap();
    drop(prepared);
    for max_work in 0..counters.work {
        let mut completed = catalog
            .snapshot(&limits, &mut Counters::default(), rule.location)
            .unwrap();
        let mut counters = Counters::default();
        let mut planning = budget();
        let refused = FormulaLimits { max_work, ..limits };
        assert!(
            matches!(PreparedRule::new(&rule, &mut completed, &refused, &mut planning, &mut counters), Err(FormulaFailure::Limit { resource: FormulaResource::Work, location, .. }) if location == rule.location)
        );
        assert_eq!(counters.work, max_work);
        assert_eq!(completed.relations.bytes, before);
        PreparedRule::new(&rule, &mut completed, &limits, &mut planning, &mut counters).unwrap();
        assert!(completed.relations.bytes > before);
    }
}

#[test]
fn cancelled_preparation_can_be_retried() {
    let (catalog, rule) = fixture();
    let limits = FormulaLimits::default();
    let mut completed = catalog
        .snapshot(&limits, &mut Counters::default(), rule.location)
        .unwrap();
    let before = completed.relations.bytes;
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let mut counters = Counters::with_allowance(allowance.clone(), &cancellation);
    assert!(
        matches!(PreparedRule::new(&rule, &mut completed, &limits, &mut budget(), &mut counters), Err(FormulaFailure::Interrupted { location, .. }) if location == rule.location)
    );
    assert_eq!(completed.relations.bytes, before);
    let mut counters = Counters::with_allowance(allowance, &Cancellation::default());
    PreparedRule::new(&rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    assert!(completed.relations.bytes > before);
}
