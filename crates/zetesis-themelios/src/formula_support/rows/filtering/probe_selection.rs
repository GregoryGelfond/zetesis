use super::*;
use crate::formula_support::{Context, RowSelection};

struct Selection<F> {
    select: F,
    reuse: bool,
    single_open: bool,
    visits: Cell<usize>,
}

impl<F: Fn(usize, zetesis_core::relation::Row<'_, '_>) -> RowSelection> RowFilter for Selection<F> {
    fn immutable_selection(&self) -> bool {
        self.reuse
    }

    fn single_open(&self) -> bool {
        self.single_open
    }

    fn resolve(
        &self,
        _: zetesis_core::catalog::Atoms<'_>,
        _: &FormulaLimits,
        _: &mut Counters,
        _: ProgramSite,
    ) -> Result<usize, FormulaFailure> {
        Ok(0)
    }

    fn permits(
        &self,
        _: usize,
        _: zetesis_core::relation::Row<'_, '_>,
        _: &FormulaLimits,
        _: &mut Counters,
        _: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        panic!("selection requires the original occurrence")
    }

    fn select(
        &self,
        _: usize,
        occurrence: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        counters.work(limits, location)?;
        self.visits.set(self.visits.get() + 1);
        Ok((self.select)(occurrence, row))
    }
}

fn fixture() -> Fixture {
    Fixture::from_atoms(
        (0..3)
            .map(|value| atom("p", &[1, value]))
            .chain((0..5).map(|value| atom("q", &[1, value]))),
        location(),
    )
}

fn body(fixture: &mut Fixture) -> Vec<LiteralIr> {
    vec![
        pattern(fixture, "p", &[0, 1]),
        pattern(fixture, "q", &[0, 2]),
    ]
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    values: Vec<Option<Value>>,
    covered: bool,
    pivot: Option<(usize, usize)>,
}

fn selected_run(reuse: bool, single_open: bool, interrupted: bool) -> (Vec<Observed>, usize) {
    let mut fixture = fixture();
    let rule = rule(body(&mut fixture), 3);
    let selection = Selection {
        select: |occurrence, row: zetesis_core::relation::Row<'_, '_>| {
            assert_eq!(occurrence, usize::from(row.predicate().name() == "q"));
            if row.predicate().name() == "p" {
                if single_open && row.position() < 2 {
                    RowSelection::Open {
                        atom: row.position(),
                    }
                } else {
                    RowSelection::Held
                }
            } else {
                match row.position() {
                    0 if single_open => RowSelection::Open { atom: 100 },
                    0 => RowSelection::Possible,
                    1 => RowSelection::Held,
                    2 if single_open => RowSelection::Possible,
                    _ => RowSelection::Rejected,
                }
            }
        },
        reuse,
        single_open,
        visits: Cell::new(0),
    };
    with_fixture(fixture, |support, computation, budget, counters| {
        let limits = FormulaLimits::default();
        let mut rows = Join::filtered_rule(
            &rule,
            support,
            Some(&selection),
            None,
            budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        assert_eq!(
            rows.join
                .plan
                .patterns
                .iter()
                .map(|pattern| pattern.source)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(rows.join.probe_selections.is_empty(), !reuse);
        let mut observed = Vec::new();
        loop {
            if interrupted {
                let stopped = FormulaLimits {
                    max_work: counters.accounting.work,
                    ..limits
                };
                assert!(matches!(
                    rows.next_row(computation, &stopped, budget, counters, location()),
                    Err(FormulaFailure::Limit {
                        resource: crate::FormulaResource::Work,
                        ..
                    })
                ));
            }
            let Some(row) = rows
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
            else {
                break;
            };
            assert!(row.passes);
            observed.push(Observed {
                values: exported(&row.values, computation),
                covered: row
                    .positives
                    .as_ref()
                    .is_some_and(|positives| positives.covers(&rule.body)),
                pivot: row
                    .positives
                    .as_ref()
                    .and_then(|positives| positives.open_in(&rule.body)),
            });
        }
        (observed, selection.visits.get())
    })
}

#[test]
fn repeated_postings_skip_only_rejected_rows() {
    let (reference, ordinary_visits) = selected_run(false, false, false);
    let (selected, reused_visits) = selected_run(true, false, false);
    assert_eq!(selected, reference);
    assert_eq!(selected.len(), 6);
    assert_eq!(ordinary_visits, 18);
    assert_eq!(reused_visits, 12);
    assert_eq!(selected.iter().filter(|row| row.covered).count(), 3);
}

#[test]
fn open_mode_preserves_current_pivot_identity() {
    let (reference, ordinary_visits) = selected_run(false, true, false);
    let (selected, reused_visits) = selected_run(true, true, false);
    assert_eq!(selected, reference);
    assert_eq!(selected.len(), 7);
    assert!(reused_visits < ordinary_visits);
    assert_eq!(
        selected
            .iter()
            .filter_map(|row| row.pivot)
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 100)]
    );
}

#[test]
fn stopped_advances_preserve_selected_evidence() {
    assert_eq!(
        selected_run(true, true, true),
        selected_run(true, true, false)
    );
}

#[test]
fn replay_keeps_scalar_overflow_visible() {
    let run = |reuse| {
        let mut fixture = fixture();
        let maximum = fixture.scalar(&Value::Number(i32::MAX), location());
        let zero = fixture.scalar(&Value::Number(0), location());
        let mut body = body(&mut fixture);
        body.push(LiteralIr::Compare(
            Expression {
                nodes: vec![
                    Operation::Constant(maximum),
                    Operation::Variable(1),
                    Operation::Variable(2),
                    Operation::Binary(BinaryOp::Mul, 1, 2),
                    Operation::Binary(BinaryOp::Add, 0, 3),
                ],
            },
            Relation::Gt,
            Expression {
                nodes: vec![Operation::Constant(zero)],
            },
        ));
        let rule = rule(body, 3);
        let selection = Selection {
            select: |_: usize, row: zetesis_core::relation::Row<'_, '_>| {
                if row.predicate().name() == "p" || row.position() < 2 {
                    RowSelection::Held
                } else {
                    RowSelection::Rejected
                }
            },
            reuse,
            single_open: false,
            visits: Cell::new(0),
        };
        with_fixture(fixture, |support, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let mut rows = Join::filtered_rule(
                &rule,
                support,
                Some(&selection),
                None,
                budget,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap();
            let mut results = Vec::new();
            let failure = loop {
                match rows.next_row(computation, &limits, budget, counters, location()) {
                    Ok(Some(row)) => results.push(exported(&row.values, computation)),
                    Ok(None) => panic!("the overflowing admitted row must be evaluated"),
                    Err(error) => break format!("{error:?}"),
                }
            };
            (results, failure)
        })
    };
    let reference = run(false);
    assert!(reference.1.contains("Overflow"));
    assert_eq!(run(true), reference);
}

#[test]
fn optional_selection_respects_remaining_storage() {
    let mut fixture = fixture();
    let rule = rule(body(&mut fixture), 3);
    let selection = Selection {
        select: |_: usize, _: zetesis_core::relation::Row<'_, '_>| RowSelection::Held,
        reuse: true,
        single_open: false,
        visits: Cell::new(0),
    };
    with_fixture(fixture, |support, computation, budget, counters| {
        let limits = FormulaLimits::default();
        let mut join = Join::rule(&rule, support, computation, &limits, budget, counters).unwrap();
        join.row_filter = Some(&selection);
        let observer = computation.lease();
        let current = limits.max_support_bytes
            - computation
                .allowance(&observer, &limits, location())
                .unwrap();
        let bounded = FormulaLimits {
            max_support_bytes: current,
            ..limits
        };
        join.prepare_selections(Context::new(computation, &bounded, counters, location()))
            .unwrap();
        assert!(join.probe_selections.is_empty());
        let mut rows = crate::formula_support::FilteredRows::new(join);
        let mut count = 0;
        while rows
            .next_row(computation, &limits, budget, counters, location())
            .unwrap()
            .is_some()
        {
            count += 1;
        }
        assert_eq!(count, 15);
        assert_eq!(selection.visits.get(), 18);
    });
}
