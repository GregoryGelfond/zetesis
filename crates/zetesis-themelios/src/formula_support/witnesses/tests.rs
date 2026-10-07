//! Witness coordinates and leases follow the same source snapshot and undo.

use super::*;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use crate::{ExpansionLimits, FormulaResource};
use zetesis_core::{Atom, AtomPattern as Pattern, Predicate, Term, Value, ValueLimits};

fn atom(name: &str, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, values.len()).unwrap(),
        values.iter().map(|&x| Value::Number(x)).collect(),
    )
    .unwrap()
}

fn fixture() -> (Fixture, RuleIr) {
    let mut fixture = Fixture::from_atoms(
        [
            atom("p", &[0, 1]),
            atom("p", &[1, 2]),
            atom("p", &[0, 3]),
            atom("q", &[7]),
            atom("q", &[8]),
        ],
        location(),
    );
    let p = fixture.pattern(
        &Pattern::new(
            Predicate::new("p", 2).unwrap(),
            vec![Term::Constant(Value::Number(0)), Term::Variable(0)],
        )
        .unwrap(),
        location(),
    );
    let q = fixture.pattern(
        &Pattern::new(Predicate::new("q", 1).unwrap(), vec![Term::Variable(1)]).unwrap(),
        location(),
    );
    let head = fixture.pattern(
        &Pattern::new(
            Predicate::new("h", 3).unwrap(),
            vec![Term::Variable(1), Term::Variable(0), Term::Variable(1)],
        )
        .unwrap(),
        location(),
    );
    (
        fixture,
        RuleIr {
            head: HeadIr::Normal(Some(head)),
            body: vec![
                LiteralIr::Atom(DefaultNegation::None, p),
                LiteralIr::Atom(DefaultNegation::None, q),
            ],
            variables: 2,
            body_variables: 2,
            bindings: None,
            location: location(),
            origins: vec![location()],
        },
    )
}

#[test]
fn matched_rows_follow_postings_and_undo() {
    witness_sequence(false);
}

#[test]
fn reversed_join_keeps_witness_coordinates() {
    witness_sequence(true);
}

fn select_witness_order(
    join: &mut Join<'_, '_>,
    reversed: bool,
    computation: &Computation<'_, '_>,
    budget: &mut Budget,
    counters: &mut Counters,
) {
    // The constant p argument selects its posting before independent q.
    assert_eq!(
        join.plan
            .patterns
            .iter()
            .map(|pattern| pattern.source)
            .collect::<Vec<_>>(),
        [0, 1],
    );
    if reversed {
        // Keep the previous q-then-p traversal as a second explicit route.
        // No probe has been resolved or row visited yet.
        join.owned_plan().patterns.reverse();
        join.decide(
            computation,
            &FormulaLimits::default(),
            budget,
            counters,
            location(),
        )
        .unwrap();
    }
}

fn witness_sequence(reversed: bool) {
    let (mut fixture, rule) = fixture();
    let (expected_order, expected_positions, expected_heads) = if reversed {
        (
            ["q", "p"],
            [0, 2, 0, 2],
            [[7, 1, 7], [7, 3, 7], [8, 1, 8], [8, 3, 8]],
        )
    } else {
        (
            ["p", "q"],
            [0, 0, 2, 2],
            [[7, 1, 7], [8, 1, 8], [7, 3, 7], [8, 3, 8]],
        )
    };
    // Both rounds start at a new cursor, even though the source rows agree.
    for _ in 0..2 {
        fixture.with(location(), |support, computation, counters| {
            let limits = FormulaLimits::default();
            let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut join =
                Join::rule(&rule, support, computation, &limits, &mut budget, counters).unwrap();
            select_witness_order(&mut join, reversed, computation, &mut budget, counters);
            join.retain_witnesses(Context::new(computation, &limits, counters, location()))
                .unwrap();
            let mut unchanged = Vec::new();
            let mut outputs = Vec::new();
            let mut positions = Vec::new();
            let mut head = None;
            while let Some(row) = join
                .next_witness_row(
                    None,
                    &mut budget,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap()
            {
                unchanged.push(row.unchanged);
                assert_eq!(
                    row.rows
                        .iter()
                        .map(|row| row.predicate().name())
                        .collect::<Vec<_>>(),
                    expected_order,
                    "witness coordinates follow the selected join order",
                );
                let p = row
                    .rows
                    .iter()
                    .find(|row| row.predicate().name() == "p")
                    .unwrap();
                assert_eq!(
                    p.atom().to_atom(ValueLimits::default()).unwrap().values()[0],
                    Value::Number(0)
                );
                positions.push(p.position());
                if head.is_none() {
                    head = Some(
                        RowHead::new(
                            eligible(&rule, &limits, counters).unwrap().unwrap(),
                            &row,
                            computation,
                            &limits,
                            counters,
                            location(),
                        )
                        .unwrap(),
                    );
                }
                let projected = head
                    .as_ref()
                    .unwrap()
                    .atom(&row, computation, &limits, counters, location())
                    .unwrap();
                let projected = computation
                    .source_atom(&projected, &limits, counters, location())
                    .unwrap()
                    .to_atom(ValueLimits::default())
                    .unwrap();
                outputs.push(projected);
            }
            assert_eq!(unchanged, [0, 1, 0, 1]);
            assert_eq!(
                positions, expected_positions,
                "posting positions resolve to actual relation rows"
            );
            assert_eq!(outputs, expected_heads.map(|values| atom("h", &values)),);
        });
    }
}

#[test]
fn witness_capacity_obeys_its_exact_storage_bound() {
    for exact in [false, true] {
        let (mut fixture, rule) = fixture();
        fixture.with(location(), |support, computation, counters| {
            let limits = FormulaLimits::default();
            let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut join =
                Join::rule(&rule, support, computation, &limits, &mut budget, counters).unwrap();
            let before = support.workspace_bytes();
            let cells = rule.body.len() * size_of::<MatchedRow<'_, '_>>();
            let required = support.live_bytes() + cells;
            let bound = FormulaLimits {
                max_support_bytes: required - usize::from(!exact),
                ..limits
            };
            let result =
                join.retain_witnesses(Context::new(computation, &bound, counters, location()));
            if exact {
                result.unwrap();
                assert_eq!(support.workspace_bytes() - before, cells);
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::SupportBytes,
                        ..
                    })
                ));
                assert_eq!(support.workspace_bytes(), before);
                join.retain_witnesses(Context::new(computation, &limits, counters, location()))
                    .unwrap();
                assert_eq!(support.workspace_bytes() - before, cells);
            }
        });
    }
}

#[test]
fn projected_head_metadata_has_one_live_lease() {
    let (mut fixture, rule) = fixture();
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut join =
            Join::rule(&rule, support, computation, &limits, &mut budget, counters).unwrap();
        join.retain_witnesses(Context::new(computation, &limits, counters, location()))
            .unwrap();
        let row = join
            .next_witness_row(
                None,
                &mut budget,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        let before = support.workspace_bytes();
        let head = RowHead::new(
            eligible(&rule, &limits, counters).unwrap().unwrap(),
            &row,
            computation,
            &limits,
            counters,
            location(),
        )
        .unwrap();
        let metadata = size_of::<RowHead<'_>>()
            + rule.body.len() * size_of::<&zetesis_core::relation::Relation<'_>>()
            + 3 * size_of::<Option<zetesis_core::atom_interner::RowColumn>>();
        assert_eq!(support.workspace_bytes() - before, metadata);
        drop(head);
        assert_eq!(support.workspace_bytes(), before);
    });
}

#[test]
fn applicability_obeys_its_work_bound() {
    let (_, rule) = fixture();
    for allowance in [1, 2] {
        let limits = FormulaLimits {
            max_work: allowance,
            ..FormulaLimits::default()
        };
        let mut counters = Counters::default();
        let result = eligible(&rule, &limits, &mut counters);
        if allowance == 2 {
            assert!(result.unwrap().is_some());
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    observed: 2,
                    limit: 1,
                    location: actual,
                }) if actual == rule.location
            ));
        }
        assert_eq!(counters.accounting.work, allowance);
    }
}

#[test]
fn interrupted_applicability_returns_no_certificate() {
    let (_, rule) = fixture();
    let cancellation = zetesis_cpu::Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default().with_cancellation(Some(&cancellation));
    assert!(matches!(
        eligible(&rule, &FormulaLimits::default(), &mut counters),
        Err(FormulaFailure::Interrupted {
            reason: zetesis_cpu::Stop::Cancelled,
            location: actual,
        }) if actual == rule.location
    ));
    assert_eq!(counters.accounting.work, 0);
}
