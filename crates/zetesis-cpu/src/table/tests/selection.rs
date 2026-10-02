use std::mem::size_of_val;

use super::*;

fn expected_rows(
    source: &[Atom],
    indices: &[usize],
    scope: &[usize],
    domains: &[Domain<'_>],
) -> Vec<usize> {
    indices
        .iter()
        .enumerate()
        .filter_map(|(position, &index)| {
            let values = source[index].values();
            let permitted =
                values
                    .iter()
                    .zip(scope)
                    .enumerate()
                    .all(|(column, (value, &variable))| {
                        let in_domain = match domains[variable] {
                            Domain::Unrestricted => true,
                            Domain::Singleton(wanted) => wanted == TermRef::from(value),
                            Domain::Finite(wanted) => {
                                wanted.iter().any(|wanted| wanted == TermRef::from(value))
                            }
                        };
                        in_domain
                            && values
                                .iter()
                                .zip(scope)
                                .take(column)
                                .all(|(earlier, &label)| label != variable || value == earlier)
                    });
            permitted.then_some(position)
        })
        .collect()
}

fn assert_domains(
    table: &Table<'_, '_>,
    source: &[Atom],
    indices: &[usize],
    domains: &[Domain<'_>],
) {
    let expected = expected_rows(source, indices, table.scope(), domains);
    let selected = table
        .select(domains, Limits::default(), &Cancellation::default())
        .unwrap();
    let projected = table
        .project_domains(domains, Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(selected.rows().collect::<Vec<_>>(), expected);
    assert_eq!(selected.words(), projected.words());
    for variable in 0..domains.len() {
        let expected_values: BTreeSet<_> = expected
            .iter()
            .flat_map(|&position| {
                source[indices[position]]
                    .values()
                    .iter()
                    .zip(table.scope())
                    .filter_map(|(value, &label)| (label == variable).then_some(value))
            })
            .collect();
        assert_eq!(
            projected.domain(variable).unwrap().collect::<BTreeSet<_>>(),
            expected_values
                .into_iter()
                .map(TermRef::from)
                .collect::<BTreeSet<_>>()
        );
    }
}

#[test]
fn borrowed_domains_match_complete_rows() {
    let predicate = Predicate::new("table", 2).unwrap();
    let universe = [
        numbers(&[0, 0]),
        numbers(&[0, 1]),
        numbers(&[1, 0]),
        numbers(&[1, 1]),
    ];
    let values = numbers(&[0, 1, 2]);
    let domains = [
        Domain::Unrestricted,
        Domain::Finite((&[] as &[Value]).into()),
        Domain::Singleton(TermRef::from(&values[0])),
        Domain::Singleton(TermRef::from(&values[1])),
        Domain::Singleton(TermRef::from(&values[2])),
        Domain::Finite((&values[..2]).into()),
    ];
    for bits in 0..16_u32 {
        let source = atoms(
            &predicate,
            &universe
                .iter()
                .enumerate()
                .filter(|(index, _)| bits & (1 << index) != 0)
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>(),
        );
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let independent = Table::prepare(
            &relation,
            &[0, 1],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let aliased = Table::prepare(
            &relation,
            &[0, 0],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let indices: Vec<_> = (0..source.len()).collect();
        for &left in &domains {
            assert_domains(&aliased, &source, &indices, &[left]);
            for &right in &domains {
                assert_domains(&independent, &source, &indices, &[left, right]);
            }
        }
    }
}

#[test]
fn selection_outlives_its_prepared_index() {
    let predicate = Predicate::with_sign("table", 1, Sign::Negative).unwrap();
    let source = atoms(&predicate, &[numbers(&[1]), numbers(&[2])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let selected = {
        let table =
            Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
        let value = Value::Number(2);
        table
            .select(
                &[Domain::Singleton(TermRef::from(&value))],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
    };
    assert!(selected.relation().same_owner(&relation));
    assert_eq!(selected.rows().collect::<Vec<_>>(), vec![1]);
    assert_eq!(
        selected.relation().row(1).unwrap().value(0),
        Some(TermRef::from(&Value::Number(2)))
    );
}

#[test]
fn advancing_bindings_preserves_prior_selection() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let mut bound = Value::Number(0);
    let first = table
        .select(
            &[Domain::Singleton(TermRef::from(&bound))],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    bound = Value::Number(1);
    let second = table
        .select(
            &[Domain::Singleton(TermRef::from(&bound))],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let restored = table
        .select(
            &[Domain::Unrestricted],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(first.words(), &[1]);
    assert_eq!(second.words(), &[2]);
    assert_eq!(restored.words(), &[3]);
}

#[test]
fn selection_preserves_catalog_occurrence_order() {
    let predicate = Predicate::with_sign("table", 2, Sign::Negative).unwrap();
    let source = atoms(
        &predicate,
        &[numbers(&[0, 0]), numbers(&[0, 1]), numbers(&[1, 1])],
    );
    let indices = [2, 0, 2, 1, 0];
    let relation =
        Relation::from_catalog(&predicate, &source, &indices, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let selected = table
        .select(
            &[Domain::Unrestricted],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(selected.rows().collect::<Vec<_>>(), vec![0, 1, 2, 4]);
    assert_eq!(
        selected
            .rows()
            .map(|row| selected.relation().row(row).unwrap().source_index())
            .collect::<Vec<_>>(),
        vec![2, 0, 2, 0]
    );
}

#[test]
fn selection_skips_empty_mask_words() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(
        &predicate,
        &(0..98)
            .map(|row| numbers(&[i32::from(matches!(row, 0 | 31 | 65 | 97))]))
            .collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(
            &[Domain::Singleton(TermRef::from(&Value::Number(1)))],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    for (from, expected) in [
        (0, Some(0)),
        (1, Some(31)),
        (31, Some(31)),
        (32, Some(65)),
        (65, Some(65)),
        (66, Some(97)),
        (98, None),
        (usize::MAX, None),
    ] {
        assert_eq!(selected.next_row(from), expected);
    }
    let mut rows = selected.rows();
    assert_eq!(rows.next(), Some(0));
    assert_eq!(rows.clone().collect::<Vec<_>>(), vec![31, 65, 97]);
    assert_eq!(rows.collect::<Vec<_>>(), vec![31, 65, 97]);
}

#[test]
fn checked_scan_visits_each_inspected_word() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(
        &predicate,
        &(0..98)
            .map(|row| numbers(&[i32::from(row == 65)]))
            .collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(
            &[Domain::Singleton(TermRef::from(&Value::Number(1)))],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    for (from, expected_row, expected_checks) in [
        (0, Some(65), 3),
        (32, Some(65), 2),
        (65, Some(65), 1),
        (66, None, 2),
        (98, None, 1),
        (128, None, 0),
        (usize::MAX, None, 0),
    ] {
        let mut checks = 0;
        let result = selected.next_row_with(from, || {
            checks += 1;
            Ok::<(), ()>(())
        });
        assert_eq!(result, Ok(expected_row));
        assert_eq!(checks, expected_checks);
    }
}

#[test]
fn checked_scan_error_preserves_retry_position() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(
        &predicate,
        &(0..66)
            .map(|row| numbers(&[i32::from(row == 65)]))
            .collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(
            &[Domain::Singleton(TermRef::from(&Value::Number(1)))],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let mut completed_checks = 0;
    let result = selected.next_row_with(0, || {
        if completed_checks == 1 {
            Err("word budget")
        } else {
            completed_checks += 1;
            Ok(())
        }
    });
    assert_eq!(result, Err("word budget"));
    assert_eq!(completed_checks, 1);
    assert_eq!(selected.words(), &[0, 0, 2]);
    assert_eq!(selected.next_row(0), Some(65));
    assert_eq!(selected.next_row_with(0, || Ok::<(), ()>(())), Ok(Some(65)));
}

#[test]
fn empty_checked_scan_needs_no_word_visit() {
    let predicate = Predicate::new("table", 0).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(&[], Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(
        selected.next_row_with(0, || Err("unexpected word")),
        Ok(None)
    );
}

#[test]
fn nullary_selection_preserves_every_occurrence() {
    let predicate = Predicate::new("table", 0).unwrap();
    for count in [0, 1, 31, 32, 33, 64, 65] {
        let source = atoms(&predicate, &vec![vec![]; count]);
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let table =
            Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
        let selected = table
            .select(&[], Limits::default(), &Cancellation::default())
            .unwrap();
        assert_eq!(
            selected.rows().collect::<Vec<_>>(),
            (0..count).collect::<Vec<_>>()
        );
        assert!(!selected.contains(count));
    }
}

#[test]
fn row_selection_omits_projected_domain_storage() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(
        &predicate,
        &(0..65).map(|value| numbers(&[value])).collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domains = [Domain::Singleton(TermRef::from(&Value::Number(1)))];
    let selected = table
        .select(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    let projected = table
        .project_domains(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(selected.words(), projected.words());
    assert!(selected.statistics().retained_bytes < projected.statistics().retained_bytes);
}

fn comparison_work(left: TermRef<'_>, right: TermRef<'_>) -> u64 {
    let mut steps = 0;
    left.compare_ref_with(right, || {
        steps += 1;
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    steps
}

#[test]
fn first_restriction_preserves_coherent_occurrences() {
    let predicate = Predicate::new("table", 3).unwrap();
    let source = atoms(
        &predicate,
        &(0..97)
            .map(|row| numbers(&[row % 3, 0, (row + i32::from(row % 5 == 0)) % 3]))
            .collect::<Vec<_>>(),
    );
    let indices: Vec<_> = (0..source.len()).rev().chain([0, 1]).collect();
    let relation =
        Relation::from_catalog(&predicate, &source, &indices, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let values = numbers(&[0, 1, 2]);
    for domains in [
        [Domain::Singleton((&values[1]).into()), Domain::Unrestricted],
        [
            Domain::Finite(values.as_slice().into()),
            Domain::Unrestricted,
        ],
        [Domain::Unrestricted, Domain::Finite((&values[..1]).into())],
    ] {
        assert_domains(&table, &source, &indices, &domains);
    }
}

#[test]
fn first_restriction_avoids_redundant_mask_storage() {
    let predicate = Predicate::new("table", 2).unwrap();
    for count in [65, 1025] {
        let source = atoms(&predicate, &vec![numbers(&[1, 1]); count]);
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let table = Table::prepare(
            &relation,
            &[0, 1],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let value = Value::Number(1);
        let singleton = Domain::Singleton(TermRef::from(&value));
        let finite = Domain::Finite(std::slice::from_ref(&value).into());
        let select = |domains: &[Domain<'_>]| {
            table
                .select(domains, Limits::default(), &Cancellation::default())
                .unwrap()
        };
        let unrestricted = select(&[Domain::Unrestricted, Domain::Unrestricted]);
        let single = select(&[singleton, Domain::Unrestricted]);
        let first = select(&[finite, Domain::Unrestricted]);
        let delayed = select(&[Domain::Unrestricted, finite]);
        let later = select(&[singleton, finite]);
        let words = u64::try_from(first.words().len()).unwrap();
        let comparison =
            comparison_work(relation.row(0).unwrap().value(0).unwrap(), (&value).into());
        // The first singleton copies one support instead of copying the base
        // and intersecting it. A first finite domain initializes/merges directly
        // into that result: no coherent copy, final intersection or union owner.
        assert_eq!(unrestricted.statistics().work, words + 2);
        assert_eq!(single.statistics().work, words + 2 + comparison);
        assert_eq!(first.statistics().work, 2 * words + 3 + comparison);
        assert_eq!(delayed.statistics(), first.statistics());
        assert_eq!(
            first.statistics().peak_bytes,
            single.statistics().peak_bytes
        );
        assert_eq!(
            first.statistics().retained_bytes,
            later.statistics().retained_bytes
        );
        assert!(
            later.statistics().peak_bytes
                >= first.statistics().peak_bytes + size_of_val(first.words())
        );
        for result in [&single, &first, &delayed, &later] {
            assert_eq!(result.words(), unrestricted.words());
            assert_eq!(result.rows().count(), count);
        }
        let exact = Limits {
            max_work: first.statistics().work,
            max_bytes: first.statistics().peak_bytes,
            ..Limits::default()
        };
        assert!(
            table
                .select(
                    &[finite, Domain::Unrestricted],
                    exact,
                    &Cancellation::default()
                )
                .is_ok()
        );
        let failure = table
            .select(
                &[finite, Domain::Unrestricted],
                Limits {
                    max_work: exact.max_work - 1,
                    ..exact
                },
                &Cancellation::default(),
            )
            .err()
            .unwrap();
        assert_eq!(failure.work, exact.max_work - 1);
        assert!(
            matches!(failure.cause, Cause::Limit { resource: Resource::Work, observed, .. } if observed == u128::from(exact.max_work))
        );
    }
}

#[test]
fn empty_first_mask_still_refuses_later_union_capacity() {
    let predicate = Predicate::new("table", 2).unwrap();
    let source = atoms(&predicate, &vec![numbers(&[1, 1]); 65]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let empty = Domain::Finite((&[] as &[Value]).into());
    let value = Value::Number(1);
    let finite = Domain::Finite(std::slice::from_ref(&value).into());
    let first = table
        .select(
            &[empty, Domain::Unrestricted],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let limit = first.statistics().peak_bytes;
    let failure = table
        .select(
            &[empty, finite],
            Limits {
                max_bytes: limit,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    // The second descriptor is inspected before its scratch allocation. No
    // scratch initialization or typed value comparison has happened on refusal.
    assert_eq!(failure.work, first.statistics().work);
    assert_eq!(failure.peak_bytes, limit);
    assert_eq!(
        failure.cause,
        Cause::Limit {
            resource: Resource::Bytes,
            observed: (limit + size_of_val(first.words())) as u128,
            limit: limit as u128
        }
    );
    assert_eq!(first.words(), &[0, 0, 0]);
    let later = table
        .select(
            &[empty, finite],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let exact = Limits {
        max_work: later.statistics().work,
        max_bytes: later.statistics().peak_bytes,
        ..Limits::default()
    };
    assert_eq!(
        table
            .select(&[empty, finite], exact, &Cancellation::default())
            .unwrap()
            .words(),
        first.words()
    );
}

#[test]
fn empty_first_mask_preserves_late_typed_comparisons() {
    use zetesis_core::{AtomCatalog, ValueLimits, ValueNode};

    let predicate = Predicate::new("table", 2).unwrap();
    let value = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 1 },
            ValueNode::String("common prefix".repeat(8)),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let source = atoms(&predicate, &[vec![Value::Number(1), value]]);
    let foreign = AtomCatalog::new(source.clone()).unwrap();
    let catalog = AtomCatalog::new(source).unwrap();
    let relation = Relation::from_refs(
        (&predicate).into(),
        catalog.atoms(),
        RelationLimits::default(),
    )
    .unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let supplied = foreign.atoms().at(0).unwrap().values().at(1).unwrap();
    let values = [supplied];
    let second = Domain::Finite(values.as_slice().into());
    let comparison = comparison_work(relation.row(0).unwrap().value(1).unwrap(), supplied);
    assert!(comparison > 8);
    let absent = Value::Number(2);
    for first in [
        Domain::Finite((&[] as &[Value]).into()),
        Domain::Singleton((&absent).into()),
    ] {
        let prefix = table
            .select(
                &[first, Domain::Unrestricted],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let complete = table
            .select(
                &[first, second],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(complete.words(), &[0]);
        // Even an empty result resolves the foreign structured value, with all
        // typed comparison callbacks, and charges the later union/intersection.
        assert_eq!(
            complete.statistics().work,
            prefix.statistics().work + 3 + 1 + comparison
        );
        for limit in prefix.statistics().work..complete.statistics().work {
            let failure = table
                .select(
                    &[first, second],
                    Limits {
                        max_work: limit,
                        ..Limits::default()
                    },
                    &Cancellation::default(),
                )
                .err()
                .unwrap();
            assert_eq!(failure.work, limit);
            assert!(
                matches!(failure.cause, Cause::Limit { resource: Resource::Work, observed, .. } if observed == u128::from(limit) + 1)
            );
        }
        assert_eq!(prefix.words(), &[0]);
    }
}

#[test]
fn selection_work_limits_are_inclusive() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &vec![numbers(&[1]); 65]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(
            &[Domain::Unrestricted],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    // Three original-mask word copies and one unrestricted descriptor inspection.
    assert_eq!(selected.statistics().work, 4);
    let exact = Limits {
        max_work: selected.statistics().work,
        ..Limits::default()
    };
    assert!(
        table
            .select(&[Domain::Unrestricted], exact, &Cancellation::default())
            .is_ok()
    );
    let failure = table
        .select(
            &[Domain::Unrestricted],
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert_eq!(failure.work, exact.max_work - 1);
    assert!(
        matches!(failure.cause, Cause::Limit { resource: Resource::Work, observed, .. } if observed == u128::from(exact.max_work))
    );
    assert_eq!(selected.rows().count(), 65);
}

#[test]
fn selection_byte_limit_includes_prepared_inputs() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[1]), numbers(&[2])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let selected = table
        .select(
            &[Domain::Unrestricted],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let peak = selected.statistics().peak_bytes;
    assert!(peak > relation.storage().retained_bytes + table.statistics().retained_bytes);
    assert!(
        table
            .select(
                &[Domain::Unrestricted],
                Limits {
                    max_bytes: peak,
                    ..Limits::default()
                },
                &Cancellation::default()
            )
            .is_ok()
    );
    let failure = table
        .select(
            &[Domain::Unrestricted],
            Limits {
                max_bytes: peak - 1,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert!(matches!(
        failure.cause,
        Cause::Limit {
            resource: Resource::Bytes,
            ..
        }
    ));
    assert_eq!(selected.words(), &[3]);
}

#[test]
fn cancelled_selection_precedes_zero_limits() {
    let predicate = Predicate::new("table", 0).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    cancellation.cancel();
    let failure = table
        .select(
            &[],
            Limits {
                max_work: 0,
                max_bytes: 0,
                max_entries: 0,
            },
            &cancellation,
        )
        .err()
        .unwrap();
    assert_eq!(failure.cause, Cause::Interrupted(Stop::Cancelled));
    assert_eq!(failure.work, 0);
}

#[test]
fn selection_rejects_wrong_domain_count() {
    let predicate = Predicate::new("table", 1).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let failure = table
        .select(&[], Limits::default(), &Cancellation::default())
        .err()
        .unwrap();
    assert_eq!(failure.cause, Cause::Domains);
}

#[test]
fn selection_rechecks_the_support_entry_limit() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let failure = table
        .select(
            &[Domain::Unrestricted],
            Limits {
                max_entries: 0,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.cause,
        Cause::Limit {
            resource: Resource::Entries,
            observed: 1,
            limit: 0
        }
    );
    assert_eq!(failure.work, 0);
}

#[test]
fn rayon_selection_preserves_query_order() {
    use rayon::prelude::*;
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1]), numbers(&[2])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domains = [
        Domain::Finite((&[] as &[Value]).into()),
        Domain::Unrestricted,
        Domain::Singleton(TermRef::from(&Value::Number(2))),
    ];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let selected: Vec<_> = pool.install(|| {
        domains
            .par_iter()
            .map(|&domain| {
                table
                    .select(&[domain], Limits::default(), &Cancellation::default())
                    .unwrap()
            })
            .collect()
    });
    assert_eq!(
        selected
            .iter()
            .map(|selection| selection.rows().collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![], vec![0, 1, 2], vec![2]]
    );
}
