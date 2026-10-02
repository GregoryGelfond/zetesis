use super::*;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation};
use crate::formula_support::testing::budget;
use crate::formula_support::{CompletedCatalog, build, testing};
use crate::{ConstraintAllowance, ConstraintCheckLimits, ExpansionLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueNodeRef};
use zetesis_cpu::Cancellation;

fn fixture() -> (CompletedCatalog, RuleIr) {
    let location = Location {
        source: SourceId::new(81),
        span: Span::empty(ByteOffset::new(7)),
    };
    let mut preparation = testing::prepare("");
    let mut catalog = preparation.catalog;
    for (name, value) in [("p", 1), ("p", 2), ("q", 2)] {
        catalog = catalog
            .insert(
                &Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(value)]).unwrap(),
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
                testing::admit_pattern(
                    &mut catalog,
                    &AtomPattern::new(Predicate::new(name, 1).unwrap(), vec![Term::Variable(slot)])
                        .unwrap(),
                    &mut Counters::default(),
                    location,
                ),
            )
        })
        .collect();
    body.push(LiteralIr::Compare(variable(0), Relation::Lt, variable(1)));
    let completed = build(
        catalog,
        &preparation.program,
        None,
        &FormulaLimits::default(),
        &mut preparation.budget,
        &mut Counters::default(),
        location,
    )
    .unwrap();
    (
        completed,
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

#[test]
fn prepared_rows_restart_without_replanning() {
    let (catalog, rule) = fixture();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(&rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let mut computation = queries.computation(rule.location).unwrap();
    // Reconstructing the ordinary plan must fail at its first pattern argument;
    // a prepared cursor borrows the admitted plan instead.
    let mut no_planning = Budget::new(
        ExpansionLimits {
            max_term_work: 0,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    );
    assert!(
        Join::rule(
            &rule,
            queries.support(),
            &computation,
            &limits,
            &mut no_planning,
            &mut counters
        )
        .is_err()
    );
    for _ in 0..2 {
        let mut rows = prepared
            .rows(
                &queries,
                None,
                &computation,
                &limits,
                &mut no_planning,
                &mut counters,
            )
            .unwrap();
        let row = rows
            .next_row(
                &mut computation,
                &limits,
                &mut budget(),
                &mut counters,
                rule.location,
            )
            .unwrap()
            .unwrap();
        assert!(row.passes);
        assert_eq!(row.values.len(), 2);
        for (slot, value) in [1, 2].into_iter().enumerate() {
            assert_eq!(
                row.values
                    .read(slot, computation.read(), rule.location)
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(value)
            );
        }
        // Drop an unfinished cursor. The next cursor starts at the same row.
    }
}

#[test]
fn prepared_rows_refuse_an_equal_foreign_catalog() {
    let (catalog, rule) = fixture();
    let (foreign, _) = fixture();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(&rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    let other = foreign
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let queries = other
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let computation = queries.computation(rule.location).unwrap();
    assert!(
        matches!(prepared.rows(&queries, None, &computation, &limits, &mut budget(), &mut counters),
        Err(FormulaFailure::SupportRelation { error: zetesis_core::relation::Failure::Owner, location }) if location == rule.location)
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
    for max_work in 0..counters.accounting.work {
        let mut completed = catalog
            .snapshot(&limits, &mut Counters::default(), rule.location)
            .unwrap();
        let mut counters = Counters::default();
        let mut planning = budget();
        let refused = FormulaLimits { max_work, ..limits };
        assert!(
            matches!(PreparedRule::new(&rule, &mut completed, &refused, &mut planning, &mut counters), Err(FormulaFailure::Limit { resource: FormulaResource::Work, location, .. }) if location == rule.location)
        );
        assert!(counters.accounting.work <= max_work);
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

fn computed_fixture(source: &str) -> (CompletedCatalog, crate::formula_ir::Prepared) {
    let preparation = testing::prepare(source);
    let program = preparation.program;
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = build(
        preparation.catalog,
        &program,
        None,
        &limits,
        &mut budget(),
        &mut counters,
        preparation.location,
    )
    .unwrap();
    {
        // Exercise the same append-capable preparation as original capture,
        // including speculative values outside any matching complete row.
        let (completed, mut append) = catalog
            .split(&limits, &mut counters, preparation.location)
            .unwrap();
        let queries = completed
            .queries(
                crate::JoinStrategy::Indexed,
                &limits,
                &counters,
                preparation.location,
            )
            .unwrap();
        let mut computation = Computation::new(&mut append, queries.support());
        let rule = program
            .rules
            .iter()
            .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
            .unwrap();
        let mut join = Join::rule(
            rule,
            queries.support(),
            &computation,
            &limits,
            &mut budget(),
            &mut counters,
        )
        .unwrap();
        assert!(
            join.select_total_constraint(rule, &mut computation, &limits, &mut counters)
                .unwrap()
        );
    }
    (catalog, program)
}

fn values(
    row: &super::super::Row<'_>,
    computation: &Computation<'_, '_>,
    location: Location,
) -> Vec<i32> {
    (0..row.values.len())
        .map(|slot| {
            match row
                .values
                .read(slot, computation.read(), location)
                .unwrap()
                .descriptor()
            {
                ValueNodeRef::Number(value) => value,
                _ => panic!("numeric fixture"),
            }
        })
        .collect()
}

#[test]
fn retained_domains_avoid_repeated_preparation_work() {
    let (catalog, program) = computed_fixture("p(1..32).q(2). :-p(X),q(Y),X/2=Y.");
    let rule = program
        .rules
        .iter()
        .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
        .unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    assert!(prepared.total.is_some());
    let retained = counters.accounting.workspace.bytes();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let mut computation = queries.computation(rule.location).unwrap();
    for _ in 0..2 {
        let before = counters.accounting.work;
        let mut borrowed = prepared
            .rows(
                &queries,
                None,
                &computation,
                &limits,
                &mut budget(),
                &mut counters,
            )
            .unwrap();
        let mut actual = Vec::new();
        while let Some(row) = borrowed
            .next_row(
                &mut computation,
                &limits,
                &mut budget(),
                &mut counters,
                rule.location,
            )
            .unwrap()
        {
            if row.passes {
                actual.push(values(&row, &computation, rule.location));
            }
        }
        let borrowed_work = counters.accounting.work - before;
        drop(borrowed);
        assert_eq!(counters.accounting.workspace.bytes(), retained);
        let before = counters.accounting.work;
        // Hold ordering fixed: only complete-column preparation differs.
        let mut fresh = Join::with_plan(
            &rule.body,
            &Binding::new(&computation, &limits, &mut counters, rule.location).unwrap(),
            rule.variables,
            queries.support(),
            std::borrow::Cow::Borrowed(&prepared.plan),
            None,
            Context::new(&computation, &limits, &mut counters, rule.location),
        )
        .unwrap();
        fresh.configure_rule(rule, &limits, &mut counters).unwrap();
        assert!(
            fresh
                .select_total_constraint(rule, &mut computation, &limits, &mut counters)
                .unwrap()
        );
        let mut expected = Vec::new();
        while let Some(row) = fresh
            .next_row(
                &mut computation,
                &limits,
                &mut budget(),
                &mut counters,
                rule.location,
            )
            .unwrap()
        {
            if row.passes {
                expected.push(values(&row, &computation, rule.location));
            }
        }
        assert_eq!(actual, vec![vec![4, 2], vec![5, 2]]);
        assert_eq!(actual, expected);
        assert!(borrowed_work < counters.accounting.work - before);
    }
}

#[test]
fn refused_totality_releases_preparation_storage() {
    let (catalog, program) = computed_fixture("p(1..32).q(2). :-p(X),q(Y),X/2=Y.");
    let rule = program
        .rules
        .iter()
        .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
        .unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let retained = completed.relations.bytes;
    let before = counters.accounting.work;
    let prepared =
        PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
    let work = counters.accounting.work - before;
    drop(prepared);
    for max_work in 0..work {
        let mut counters = Counters::default();
        let mut completed = catalog
            .snapshot(&limits, &mut counters, rule.location)
            .unwrap();
        let before = counters.accounting.work;
        let workspace = counters.accounting.workspace.bytes();
        let limited = FormulaLimits {
            max_work: before + max_work,
            ..limits
        };
        let result =
            PreparedRule::new(rule, &mut completed, &limited, &mut budget(), &mut counters);
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(completed.relations.bytes, retained);
        assert_eq!(counters.accounting.workspace.bytes(), workspace);
    }
}

#[test]
fn retained_domains_refuse_a_foreign_workspace() {
    for source in [
        "p(1..32).q(2). :-p(X),q(Y),X/2=Y.",
        // A nonbinding guard is total but prepares no ordinary comparison map.
        "p(1..4). :-p(X),not X/2=0.",
    ] {
        let (catalog, program) = computed_fixture(source);
        let rule = program
            .rules
            .iter()
            .find(|rule| matches!(rule.head, HeadIr::Normal(None)))
            .unwrap();
        let limits = FormulaLimits::default();
        let mut counters = Counters::default();
        let mut completed = catalog
            .snapshot(&limits, &mut counters, rule.location)
            .unwrap();
        let prepared =
            PreparedRule::new(rule, &mut completed, &limits, &mut budget(), &mut counters).unwrap();
        assert!(prepared.total.is_some());
        let mut foreign = Counters::default();
        let other = catalog
            .snapshot(&limits, &mut foreign, rule.location)
            .unwrap();
        let queries = other
            .queries(
                crate::JoinStrategy::Indexed,
                &limits,
                &foreign,
                rule.location,
            )
            .unwrap();
        let computation = queries.computation(rule.location).unwrap();
        let result = prepared.rows(
            &queries,
            None,
            &computation,
            &limits,
            &mut budget(),
            &mut foreign,
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::TermAssignment {
                error: zetesis_core::catalog::AssignmentError::Read(
                    zetesis_core::catalog::ReadError::ForeignCatalog
                ),
                ..
            })
        ));
    }
}
