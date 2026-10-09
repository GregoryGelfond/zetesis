use super::*;
use crate::formula_support::{Context, RowSelection};

struct Evidence<F>(F);

impl<F: Fn(zetesis_core::relation::Row<'_, '_>) -> RowSelection> RowFilter for Evidence<F> {
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
        source: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        self.select(source, 0, row, limits, counters, location)
            .map(RowSelection::permits)
    }

    fn select(
        &self,
        _: usize,
        _: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        counters.work(limits, location)?;
        Ok(self.0(row))
    }
}

#[test]
fn unmapped_prefixes_do_not_survive_backtracking() {
    let mut fixture = Fixture::from_atoms(
        [
            atom("p", &[1]),
            atom("p", &[2]),
            atom("q", &[1]),
            atom("q", &[2]),
        ],
        location(),
    );
    let rule = rule(
        vec![
            pattern(&mut fixture, "p", &[0]),
            pattern(&mut fixture, "q", &[1]),
        ],
        2,
    );
    let selection = Evidence(|row: zetesis_core::relation::Row<'_, '_>| {
        if row.value(0).unwrap().descriptor() == ValueNodeRef::Number(2) {
            RowSelection::Held
        } else {
            RowSelection::Possible
        }
    });
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
        let mut observed = Vec::new();
        loop {
            // A refused advance cannot leave evidence attached to a different
            // binding. Retrying also exercises the suspended final-depth undo.
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
            let Some(row) = rows
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
            else {
                break;
            };
            assert!(row.passes);
            observed.push((exported(&row.values, computation), row.positives.is_some()));
        }
        observed.sort();
        let expected = [(1, 1, false), (1, 2, false), (2, 1, false), (2, 2, true)]
            .map(|(a, b, proved)| (vec![Some(Value::Number(a)), Some(Value::Number(b))], proved));
        assert_eq!(observed, expected);
        // Both placements of an unmapped occurrence appear, and a later fully
        // held row recovers its proof after undoing an unmapped earlier prefix.
        assert_eq!(counters.accounting.substitutions, 4);
    });
}

#[test]
fn repeated_occurrences_require_their_exact_match() {
    let mut fixture = Fixture::from_atoms(
        [atom("p", &[1, 1]), atom("p", &[1, 2]), atom("p", &[2, 2])],
        location(),
    );
    let rule = rule(
        vec![
            pattern(&mut fixture, "p", &[0, 0]),
            pattern(&mut fixture, "p", &[0, 0]),
        ],
        1,
    );
    let selection = Evidence(|_: zetesis_core::relation::Row<'_, '_>| RowSelection::Held);
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
        let mut observed = Vec::new();
        while let Some(row) = rows
            .next_row(computation, &limits, budget, counters, location())
            .unwrap()
        {
            assert!(row.passes);
            assert!(row.positives.as_ref().unwrap().covers(&rule.body));
            observed.push(exported(&row.values, computation));
        }
        assert_eq!(
            observed,
            [vec![Some(Value::Number(1))], vec![Some(Value::Number(2))]]
        );
    });
}

#[test]
fn structured_matches_establish_positive_evidence() {
    let predicate = Predicate::new("p", 1).unwrap();
    let atoms = [7, 8].map(|last| {
        Atom::new(
            predicate.clone(),
            vec![
                Value::from_nodes(
                    vec![
                        ValueNode::Tuple { arity: 2 },
                        ValueNode::Number(7),
                        ValueNode::Number(last),
                    ],
                    ValueLimits::default(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    });
    let mut fixture = Fixture::from_atoms(atoms, location());
    let tuple = fixture.constructor(ValueNodeRef::Tuple { arity: 2 }, location());
    let rule = rule(
        vec![LiteralIr::PatternAtom(PatternAtom {
            atom: fixture.pattern(
                &AtomPattern::new(predicate, vec![Term::Variable(0)]).unwrap(),
                location(),
            ),
            arguments: vec![ArgumentPattern {
                position: 0,
                nodes: vec![
                    PatternNode::Constructor(tuple),
                    PatternNode::Slot(1),
                    PatternNode::Slot(1),
                ],
            }],
        })],
        2,
    );
    let selection = Evidence(|_: zetesis_core::relation::Row<'_, '_>| RowSelection::Held);
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
        let row = rows
            .next_row(computation, &limits, budget, counters, location())
            .unwrap()
            .unwrap();
        assert!(row.passes);
        assert!(row.positives.is_some());
        assert_eq!(
            exported(&row.values, computation)[1],
            Some(Value::Number(7))
        );
        assert!(
            rows.next_row(computation, &limits, budget, counters, location())
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn failed_scalars_never_lend_positive_evidence() {
    for selected in [RowSelection::Held, RowSelection::Open { atom: 0 }] {
        for divisor in [0, 1] {
            let mut fixture = Fixture::from_atoms([atom("p", &[divisor])], location());
            let one = fixture.scalar(&Value::Number(1), location());
            let expression = Expression {
                nodes: vec![
                    Operation::Constant(one),
                    Operation::Variable(0),
                    Operation::Binary(BinaryOp::Div, 0, 1),
                ],
            };
            let rule = rule(
                vec![
                    pattern(&mut fixture, "p", &[0]),
                    LiteralIr::Compare(
                        expression,
                        Relation::Lt,
                        Expression {
                            nodes: vec![Operation::Constant(one)],
                        },
                    ),
                ],
                1,
            );
            let selection = Evidence(|_: zetesis_core::relation::Row<'_, '_>| selected);
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
                loop {
                    match rows.next_row(computation, &limits, budget, counters, location()) {
                        Ok(Some(row)) => {
                            assert!(!row.passes);
                            assert!(row.positives.is_none());
                        }
                        Ok(None) => break,
                        Err(error) => panic!("unexpected failure: {error:?}"),
                    }
                }
                // Division by zero remains the join's original family diagnostic;
                // the selected-row loan does not turn it into scalar success.
                assert_eq!(rows.join.family.zero.is_some(), divisor == 0);
            });
        }
    }
}

#[test]
fn generated_rows_keep_the_ordinary_truth_path() {
    for selected in [RowSelection::Held, RowSelection::Open { atom: 0 }] {
        let mut fixture = Fixture::from_atoms([atom("p", &[2])], location());
        let one = fixture.scalar(&Value::Number(1), location());
        let rule = rule(
            vec![
                pattern(&mut fixture, "p", &[0]),
                LiteralIr::Range {
                    target: 1,
                    lower: Expression {
                        nodes: vec![Operation::Constant(one)],
                    },
                    upper: Expression {
                        nodes: vec![Operation::Variable(0)],
                    },
                    binder: true,
                },
            ],
            2,
        );
        let selection = Evidence(|_: zetesis_core::relation::Row<'_, '_>| selected);
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
            let mut observed = Vec::new();
            while let Some(row) = rows
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
            {
                assert!(row.passes);
                assert!(row.positives.is_none());

                observed.push(exported(&row.values, computation)[1].clone());
            }
            assert_eq!(observed, [Some(Value::Number(1)), Some(Value::Number(2))]);
        });
    }
}

struct Pivot {
    occurrence: usize,
    first: Cell<Option<usize>>,
}

impl RowFilter for Pivot {
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
        panic!("the join must supply the original occurrence to select")
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
        if self.first.get().is_none() {
            self.first.set(Some(occurrence));
        }
        Ok(if occurrence == self.occurrence {
            RowSelection::Open {
                atom: row.position(),
            }
        } else {
            RowSelection::Held
        })
    }
}

#[test]
fn reordered_pivots_keep_original_occurrences() {
    let mut fixture = Fixture::from_atoms(
        [
            atom("p", &[1]),
            atom("p", &[2]),
            atom("p", &[3]),
            atom("q", &[1]),
        ],
        location(),
    );
    let rule = rule(
        vec![
            pattern(&mut fixture, "p", &[0]),
            pattern(&mut fixture, "q", &[1]),
        ],
        2,
    );
    let filter = Pivot {
        occurrence: 0,
        first: Cell::new(None),
    };
    with_fixture(fixture, |support, computation, budget, counters| {
        let limits = FormulaLimits::default();
        let mut rows = Join::filtered_rule(
            &rule,
            support,
            Some(&filter),
            None,
            budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        assert_eq!(
            rows.join.plan.patterns[0].source, 1,
            "smaller q relation moves before p"
        );
        let mut count = 0;
        while let Some(row) = rows
            .next_row(computation, &limits, budget, counters, location())
            .unwrap()
        {
            assert!(row.passes);
            assert!(!row.positives.as_ref().unwrap().covers(&rule.body));
            let (occurrence, atom) = row.positives.as_ref().unwrap().open_in(&rule.body).unwrap();
            assert_eq!(occurrence, 0);
            assert_eq!(atom, count);
            count += 1;
        }
        assert_eq!(count, 3);
        assert_eq!(filter.first.get(), Some(1));
    });
}

#[test]
fn pivot_identity_follows_backtracking() {
    let mut fixture = Fixture::from_atoms(
        [
            atom("p", &[1]),
            atom("p", &[2]),
            atom("q", &[1]),
            atom("q", &[2]),
        ],
        location(),
    );
    let rule = rule(
        vec![
            pattern(&mut fixture, "p", &[0]),
            pattern(&mut fixture, "q", &[1]),
        ],
        2,
    );
    let selection = Evidence(|row: zetesis_core::relation::Row<'_, '_>| {
        if row.predicate().name() == "p" {
            RowSelection::Open {
                atom: row.position(),
            }
        } else if row.value(0).unwrap().descriptor() == ValueNodeRef::Number(2) {
            RowSelection::Held
        } else {
            RowSelection::Possible
        }
    });
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
        let mut observed = Vec::new();
        loop {
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
            let Some(row) = rows
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
            else {
                break;
            };
            assert!(
                !row.positives
                    .as_ref()
                    .is_some_and(|proof| proof.covers(&rule.body))
            );
            let pivot = row
                .positives
                .as_ref()
                .and_then(|proof| proof.open_in(&rule.body));
            observed.push((exported(&row.values, computation), pivot));
        }
        observed.sort();
        let expected = [
            (1, 1, None),
            (1, 2, Some((0, 0))),
            (2, 1, None),
            (2, 2, Some((0, 1))),
        ]
        .map(|(p, q, proof)| (vec![Some(Value::Number(p)), Some(Value::Number(q))], proof));
        assert_eq!(observed, expected);
    });
}

#[test]
fn aliased_open_occurrences_lend_no_pivot() {
    let mut fixture = Fixture::from_atoms([atom("p", &[1]), atom("p", &[2])], location());
    // Raw IR retains both occurrences, unlike authored identical literals
    // which source normalization may coalesce.
    let rule = rule(
        vec![
            pattern(&mut fixture, "p", &[0]),
            pattern(&mut fixture, "p", &[0]),
        ],
        1,
    );
    let selection = Evidence(
        |row: zetesis_core::relation::Row<'_, '_>| RowSelection::Open {
            atom: row.position(),
        },
    );
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
        let mut observed = Vec::new();
        while let Some(row) = rows
            .next_row(computation, &limits, budget, counters, location())
            .unwrap()
        {
            assert!(row.passes);

            assert!(row.positives.is_none());
            observed.push(exported(&row.values, computation));
        }
        assert_eq!(
            observed,
            [vec![Some(Value::Number(1))], vec![Some(Value::Number(2))]]
        );
    });
}

struct OpenRows {
    bounded: bool,
}

impl RowFilter for OpenRows {
    fn single_open(&self) -> bool {
        self.bounded
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
        source: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        self.select(source, 0, row, limits, counters, location)
            .map(RowSelection::permits)
    }

    fn select(
        &self,
        _: usize,
        _: usize,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<RowSelection, FormulaFailure> {
        counters.work(limits, location)?;
        Ok(if row.predicate().name() == "u" {
            RowSelection::Possible
        } else if row.value(0).unwrap().descriptor() == ValueNodeRef::Number(1) {
            RowSelection::Open {
                atom: row.position(),
            }
        } else {
            RowSelection::Held
        })
    }
}

#[test]
fn an_unmapped_gap_retains_the_open_restriction() {
    for compare in [false, true] {
        let mut fixture = Fixture::from_atoms(
            [
                atom("u", &[1]),
                atom("a", &[1]),
                atom("a", &[2]),
                atom("b", &[1]),
                atom("b", &[2]),
            ],
            location(),
        );
        let mut body = vec![
            pattern(&mut fixture, "u", &[0]),
            pattern(&mut fixture, "a", &[1]),
            pattern(&mut fixture, "b", &[2]),
        ];
        if compare {
            body.push(LiteralIr::Compare(
                Expression {
                    nodes: vec![Operation::Variable(1)],
                },
                Relation::Neq,
                Expression {
                    nodes: vec![Operation::Variable(2)],
                },
            ));
        }
        let rule = rule(body, 3);
        with_fixture(fixture, |support, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let mut outputs = Vec::new();
            for bounded in [false, true] {
                let selection = OpenRows { bounded };
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
                    rows.join.plan.patterns[0].source, 0,
                    "the unmapped row is first"
                );
                let mut selected = Vec::new();
                loop {
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
                    let Some(row) = rows
                        .next_row(computation, &limits, budget, counters, location())
                        .unwrap()
                    else {
                        break;
                    };
                    assert!(
                        row.positives.is_none(),
                        "the unmapped gap forbids a truth loan"
                    );
                    if row.passes {
                        selected.push(exported(&row.values, computation));
                    }
                }
                outputs.push(selected);
            }
            let expected: Vec<_> = outputs[0]
                .iter()
                .filter(|values| {
                    values[1] != Some(Value::Number(1)) || values[2] != Some(Value::Number(1))
                })
                .cloned()
                .collect();
            assert_eq!(outputs[1], expected);
            assert_eq!(outputs[1].len(), if compare { 2 } else { 3 });
        });
    }
}
