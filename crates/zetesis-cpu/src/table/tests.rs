use std::{collections::BTreeSet, time::Instant};

use zetesis_core::{Atom, Predicate, Sign, relation::Limits as RelationLimits};

use super::*;

mod selection;

fn atoms(predicate: &Predicate, rows: &[Vec<Value>]) -> Vec<Atom> {
    rows.iter()
        .cloned()
        .map(|row| Atom::new(predicate.clone(), row).unwrap())
        .collect()
}

fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().copied().map(Value::Number).collect()
}

/// Independent complete-row reference: no equality IDs, bitset index, binary
/// lookup or prior projection participates in the expected result.
fn reference<'a>(
    rows: &'a [Atom],
    indices: &[usize],
    scope: &[usize],
    domains: &[&[Value]],
) -> (Vec<usize>, Vec<BTreeSet<&'a Value>>) {
    let positions: Vec<_> = indices
        .iter()
        .enumerate()
        .filter_map(|(position, &index)| {
            let values = rows[index].values();
            let in_domains = values
                .iter()
                .zip(scope)
                .all(|(value, &variable)| domains[variable].contains(value));
            let coherent =
                values
                    .iter()
                    .zip(scope)
                    .enumerate()
                    .all(|(column, (value, variable))| {
                        values
                            .iter()
                            .zip(scope)
                            .take(column)
                            .all(|(other, label)| label != variable || other == value)
                    });
            (in_domains && coherent).then_some(position)
        })
        .collect();
    let mut supported = vec![BTreeSet::new(); domains.len()];
    for &position in &positions {
        for (value, &variable) in rows[indices[position]].values().iter().zip(scope) {
            supported[variable].insert(value);
        }
    }
    (positions, supported)
}

fn assert_reference(
    table: &Table<'_, '_>,
    source: &[Atom],
    indices: &[usize],
    domains: &[&[Value]],
) {
    let actual = table
        .project(domains, Limits::default(), &Cancellation::default())
        .unwrap();
    let (positions, expected) = reference(source, indices, table.scope(), domains);
    let actual_positions: Vec<_> = (0..indices.len())
        .filter(|&row| actual.contains(row))
        .collect();
    assert_eq!(actual_positions, positions);
    for (variable, expected) in expected.iter().enumerate() {
        assert_eq!(
            &actual.domain(variable).unwrap().collect::<BTreeSet<_>>(),
            expected
        );
    }
}

#[test]
fn exhaustive_binary_tables_match_complete_rows() {
    let predicate = Predicate::new("table", 2).unwrap();
    let universe = [
        numbers(&[0, 0]),
        numbers(&[0, 1]),
        numbers(&[1, 0]),
        numbers(&[1, 1]),
    ];
    let domains = [numbers(&[]), numbers(&[0]), numbers(&[1]), numbers(&[0, 1])];
    for selected in 0..16_u32 {
        let source = atoms(
            &predicate,
            &universe
                .iter()
                .enumerate()
                .filter(|(row, _)| selected & (1 << row) != 0)
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>(),
        );
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let indices: Vec<_> = (0..source.len()).collect();
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
        for left in &domains {
            assert_reference(&aliased, &source, &indices, &[left]);
            for right in &domains {
                assert_reference(&independent, &source, &indices, &[left, right]);
            }
        }
    }
}

#[test]
fn catalog_occurrences_retain_their_row_identity() {
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
    let domain = numbers(&[0, 1]);
    let projection = table
        .project(&[&domain], Limits::default(), &Cancellation::default())
        .unwrap();
    assert!(projection.table().relation().same_owner(&relation));
    assert_eq!(
        (0..indices.len())
            .filter(|&row| projection.contains(row))
            .map(|row| relation.row(row).unwrap().source_index())
            .collect::<Vec<_>>(),
        vec![2, 0, 2, 0]
    );
    assert_eq!(
        projection.table().relation().predicate().sign(),
        Sign::Negative
    );
}

#[test]
fn typed_domains_do_not_conflate_equal_spellings() {
    let predicate = Predicate::new("table", 1).unwrap();
    let values = [
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        Value::Infimum,
        Value::Supremum,
        Value::from_nodes(
            vec![
                zetesis_core::ValueNode::Tuple { arity: 1 },
                zetesis_core::ValueNode::Number(1),
            ],
            zetesis_core::ValueLimits::default(),
        )
        .unwrap(),
    ];
    let source = atoms(
        &predicate,
        &values
            .iter()
            .cloned()
            .map(|value| vec![value])
            .collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    for (index, value) in values.iter().enumerate() {
        let selected = table
            .select(
                &[Domain::Singleton(value)],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(selected.rows().collect::<Vec<_>>(), vec![index]);
        let projection = table
            .project(
                &[std::slice::from_ref(value)],
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            (0..values.len())
                .filter(|&row| projection.contains(row))
                .collect::<Vec<_>>(),
            vec![index]
        );
        let borrowed = projection.domain(0).unwrap().next().unwrap();
        assert!(std::ptr::eq(
            borrowed,
            relation.row(index).unwrap().value(0).unwrap()
        ));
    }
}

#[test]
fn widening_restores_rows_excluded_by_an_earlier_call() {
    let predicate = Predicate::new("table", 2).unwrap();
    let source = atoms(
        &predicate,
        &[numbers(&[0, 1]), numbers(&[1, 0]), numbers(&[1, 1])],
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let small = numbers(&[0]);
    let wide = numbers(&[0, 1]);
    let narrow = table
        .project(
            &[&small, &wide],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(narrow.words(), &[1]);
    let restored = table
        .project(&[&wide, &wide], Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(restored.words(), &[7]);
    assert_eq!(narrow.words(), &[1]);
}

#[test]
fn equal_domain_sizes_do_not_reuse_stale_rows() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let zero = numbers(&[0]);
    let one = numbers(&[1]);
    assert_eq!(
        table
            .project(&[&zero], Limits::default(), &Cancellation::default())
            .unwrap()
            .words(),
        &[1]
    );
    assert_eq!(
        table
            .project(&[&one], Limits::default(), &Cancellation::default())
            .unwrap()
            .words(),
        &[2]
    );
}

#[test]
fn supported_domain_projection_preserves_surviving_rows() {
    let predicate = Predicate::new("table", 3).unwrap();
    let source = atoms(
        &predicate,
        &[
            numbers(&[0, 1, 0]),
            numbers(&[1, 0, 1]),
            numbers(&[1, 1, 0]),
        ],
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let first = numbers(&[0, 1, 2]);
    let second = numbers(&[1, 2]);
    let projected = table
        .project(
            &[&first, &second],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let narrowed: Vec<Vec<_>> = (0..2)
        .map(|variable| projected.domain(variable).unwrap().cloned().collect())
        .collect();
    let again = table
        .project(
            &narrowed.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(again.words(), projected.words());
}

#[test]
fn nullary_projection_preserves_every_occurrence() {
    let predicate = Predicate::new("table", 0).unwrap();
    for count in [0, 1, 31, 32, 33, 64, 65] {
        let source = atoms(&predicate, &vec![vec![]; count]);
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let table =
            Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
        let projection = table
            .project(&[], Limits::default(), &Cancellation::default())
            .unwrap();
        assert_eq!(
            projection
                .words()
                .iter()
                .map(|word| word.count_ones())
                .sum::<u32>() as usize,
            count
        );
        assert!((0..count).all(|row| projection.contains(row)));
        assert!(!projection.contains(count));
        assert!(projection.domain(0).is_none());
    }
}

#[test]
fn domain_duplicates_have_set_meaning() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[1]), numbers(&[2])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domain = numbers(&[1, 1, 7]);
    let projection = table
        .project(&[&domain], Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(projection.words(), &[1]);
    assert_eq!(
        projection.domain(0).unwrap().cloned().collect::<Vec<_>>(),
        numbers(&[1])
    );
}

#[test]
fn construction_limits_are_inclusive() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let stats = table.statistics();
    let exact = Limits {
        max_work: stats.work,
        max_bytes: stats.peak_bytes,
        max_entries: 2,
    };
    assert!(Table::prepare(&relation, &[0], exact, &Cancellation::default()).is_ok());
    for (limits, expected) in [
        (
            Limits {
                max_work: stats.work - 1,
                ..exact
            },
            Resource::Work,
        ),
        (
            Limits {
                max_bytes: stats.peak_bytes - 1,
                ..exact
            },
            Resource::Bytes,
        ),
        (
            Limits {
                max_entries: 1,
                ..exact
            },
            Resource::Entries,
        ),
    ] {
        let failure = Table::prepare(&relation, &[0], limits, &Cancellation::default())
            .err()
            .unwrap();
        assert!(matches!(failure.cause, Cause::Limit { resource, .. } if resource == expected));
        assert!(failure.work <= stats.work);
    }
}

#[test]
fn projection_limits_do_not_replace_an_existing_result() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domain = numbers(&[1]);
    let first = table
        .project(&[&domain], Limits::default(), &Cancellation::default())
        .unwrap();
    let stats = first.statistics();
    let exact = Limits {
        max_work: stats.work,
        max_bytes: stats.peak_bytes,
        ..Limits::default()
    };
    assert!(
        table
            .project(&[&domain], exact, &Cancellation::default())
            .is_ok()
    );
    let failure = table
        .project(
            &[&domain],
            Limits {
                max_work: stats.work - 1,
                ..exact
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert!(matches!(
        failure.cause,
        Cause::Limit {
            resource: Resource::Work,
            ..
        }
    ));
    assert_eq!(first.words(), &[2]);
    assert_eq!(
        table
            .project(&[&domain], exact, &Cancellation::default())
            .unwrap()
            .words(),
        first.words()
    );
}

#[test]
fn cancellation_precedes_zero_resource_allowances() {
    let predicate = Predicate::new("table", 0).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    cancellation.cancel();
    let failure = Table::prepare(
        &relation,
        &[],
        Limits {
            max_bytes: 0,
            max_work: 0,
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
fn expired_deadlines_preserve_the_prepared_owner() {
    let predicate = Predicate::new("table", 0).unwrap();
    let source = atoms(&predicate, &[vec![]]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
    let failure = table
        .project(
            &[],
            Limits::default(),
            &Cancellation::with_deadline(Instant::now()).unwrap(),
        )
        .err()
        .unwrap();
    assert_eq!(failure.cause, Cause::Interrupted(Stop::Deadline));
    assert_eq!(
        table
            .project(&[], Limits::default(), &Cancellation::default())
            .unwrap()
            .words(),
        &[1]
    );
}

#[test]
fn invalid_shapes_are_explicit_refusals() {
    let predicate = Predicate::new("table", 2).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    for scope in [&[][..], &[1, 0], &[0, 2]] {
        assert_eq!(
            Table::prepare(
                &relation,
                scope,
                Limits::default(),
                &Cancellation::default()
            )
            .err()
            .unwrap()
            .cause,
            Cause::Scope
        );
    }
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        table
            .project(&[], Limits::default(), &Cancellation::default())
            .err()
            .unwrap()
            .cause,
        Cause::Domains
    );
}

#[test]
fn indexed_rayon_preserves_independent_projection_order() {
    use rayon::prelude::*;
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1]), numbers(&[2])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domains = [numbers(&[2]), numbers(&[0, 1]), numbers(&[])];
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let results: Vec<_> = pool.install(|| {
        domains
            .par_iter()
            .map(|domain| {
                table
                    .project(&[domain], Limits::default(), &Cancellation::default())
                    .unwrap()
            })
            .collect()
    });
    assert_eq!(
        results.iter().map(Projection::words).collect::<Vec<_>>(),
        vec![&[4][..], &[3], &[0]]
    );
}

#[test]
fn multiword_supports_match_complete_rows() {
    let predicate = Predicate::new("table", 3).unwrap();
    for count in [31, 32, 33, 63, 64, 65, 97] {
        let source = atoms(
            &predicate,
            &(0..count)
                .map(|row| numbers(&[row % 5, row % 3, row % 5]))
                .collect::<Vec<_>>(),
        );
        let relation =
            Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
        let table = Table::prepare(
            &relation,
            &[0, 1, 0],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let first = numbers(&[0, 2, 4]);
        let second = numbers(&[0, 2]);
        assert_reference(
            &table,
            &source,
            &(0..source.len()).collect::<Vec<_>>(),
            &[&first, &second],
        );
    }
}

#[test]
fn projection_byte_ceiling_includes_prepared_inputs() {
    let predicate = Predicate::new("table", 1).unwrap();
    let source = atoms(&predicate, &[numbers(&[0]), numbers(&[1])]);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    let domain = numbers(&[1]);
    let actual = table
        .project(&[&domain], Limits::default(), &Cancellation::default())
        .unwrap();
    let peak = actual.statistics().peak_bytes;
    assert!(peak > relation.storage().retained_bytes + table.statistics().retained_bytes);
    let failure = table
        .project(
            &[&domain],
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
    assert_eq!(actual.words(), &[2]);
}

#[test]
fn empty_relations_remove_every_supplied_value() {
    let predicate = Predicate::new("table", 2).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let domain = numbers(&[1, 2]);
    let actual = table
        .project(
            &[&domain, &domain],
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(actual.words().is_empty());
    for variable in 0..2 {
        assert!(actual.domain(variable).unwrap().next().is_none());
    }
}

#[test]
fn singleton_domains_match_prepared_equality_queries() {
    let predicate = Predicate::new("table", 2).unwrap();
    let source = atoms(
        &predicate,
        &(0..65)
            .map(|row| numbers(&[row % 4, row % 3]))
            .collect::<Vec<_>>(),
    );
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let input = relation.all(RelationLimits::default()).unwrap();
    assert_eq!(table.support_entries(), 7);
    for left in 0..5 {
        for right in 0..4 {
            let left = Value::Number(left);
            let right = Value::Number(right);
            let query = relation
                .query(&[(0, &left), (1, &right)], RelationLimits::default())
                .unwrap();
            let expected = relation
                .select_mask(&query, &input, RelationLimits::default())
                .unwrap();
            let actual = table
                .project(
                    &[std::slice::from_ref(&left), std::slice::from_ref(&right)],
                    Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(actual.words(), expected.words());
        }
    }
}
