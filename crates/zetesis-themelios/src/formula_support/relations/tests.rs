use zetesis_core::{Sign, Term, ValueLimits, ValueNode, ValueNodeRef};

use super::*;
use crate::formula_support::{Computation, Support, testing};
use crate::test_support::location;

fn insert(catalog: &mut SupportCatalog, atom: &Atom) {
    *catalog = std::mem::take(catalog)
        .insert(
            atom,
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
}

fn atom(values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new("row", values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn contains(support: &Relations<'_>, atom: &Atom) -> bool {
    let pattern = AtomPattern::new(
        atom.predicate().clone(),
        atom.values().iter().cloned().map(Term::Constant).collect(),
    )
    .unwrap();
    support
        .contains(
            &pattern.key(&[] as &[Value]).unwrap(),
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap()
}

#[test]
fn membership_preserves_append_order() {
    let mut catalog = SupportCatalog::default();
    for value in [3, 1, 2] {
        insert(&mut catalog, &atom(&[value]));
    }
    {
        let support = catalog
            .snapshot(
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
        for value in [1, 2, 3] {
            assert!(contains(&support, &atom(&[value])));
        }
        assert!(!contains(&support, &atom(&[0])));
        let rows: Vec<_> = support
            .rows(atom(&[0]).predicate())
            .map(|row| (row.source_index(), row.value(0).unwrap().descriptor()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (0, ValueNodeRef::Number(3)),
                (1, ValueNodeRef::Number(1)),
                (2, ValueNodeRef::Number(2))
            ]
        );
    }
    insert(&mut catalog, &atom(&[0]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert_eq!(
        support
            .row(atom(&[0]).predicate(), 3)
            .unwrap()
            .value(0)
            .map(TermRef::descriptor),
        Some(ValueNodeRef::Number(0))
    );
    assert_eq!(
        support
            .row(atom(&[3]).predicate(), 0)
            .unwrap()
            .value(0)
            .map(TermRef::descriptor),
        Some(ValueNodeRef::Number(3))
    );
}

#[test]
fn snapshots_reuse_columns_without_row_work() {
    let mut catalog = SupportCatalog::default();
    for value in 0..64 {
        insert(&mut catalog, &atom(&[value]));
    }
    let mut counters = Counters::default();
    let first = catalog
        .snapshot(&FormulaLimits::default(), &mut counters, location())
        .unwrap();
    let first_work = counters.accounting.work;
    let second = catalog
        .snapshot(&FormulaLimits::default(), &mut counters, location())
        .unwrap();
    assert_eq!(first_work, 1);
    assert_eq!(counters.accounting.work, 2);
    let predicate = atom(&[0]);
    let left = first.find(predicate.predicate().into()).unwrap();
    let right = second.find(predicate.predicate().into()).unwrap();
    assert!(std::ptr::eq(left.columns, right.columns));
    assert!(std::ptr::eq(
        left.relation.column(0).unwrap(),
        right.relation.column(0).unwrap()
    ));
}

#[test]
fn nullary_membership_retains_predicate_sign() {
    let positive = Atom::new(Predicate::new("p", 0).unwrap(), Vec::new()).unwrap();
    let negative = Atom::new(
        Predicate::with_sign("p", 0, Sign::Negative).unwrap(),
        Vec::new(),
    )
    .unwrap();
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &negative);
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    assert!(contains(&support, &negative));
    assert!(!contains(&support, &positive));
    let row = support.row(negative.predicate(), 0).unwrap();
    assert_eq!(row.predicate(), PredicateRef::from(negative.predicate()));
    assert_eq!(row.value(0), None);
    assert_eq!(support.row_count(positive.predicate()), 0);
}

#[test]
fn column_lookup_preserves_the_shortest_posting() {
    let original: Vec<_> = [[2, 1], [1, 2], [2, 2], [3, 1]]
        .iter()
        .map(|row| atom(row))
        .collect();
    let mut catalog = SupportCatalog::default();
    for row in &original {
        insert(&mut catalog, row);
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        original[0].predicate().clone(),
        vec![Term::Variable(0), Term::Variable(1)],
    )
    .unwrap();
    for left in [None, Some(-1), Some(1), Some(2), Some(3)] {
        for right in [None, Some(-1), Some(1), Some(2), Some(3)] {
            let values = [left.map(Value::Number), right.map(Value::Number)];
            let postings: Vec<Vec<usize>> = values
                .iter()
                .enumerate()
                .filter_map(|(column, value)| {
                    value.as_ref().map(|value| {
                        original
                            .iter()
                            .enumerate()
                            .filter_map(|(row, atom)| {
                                (atom.values()[column] == *value).then_some(row)
                            })
                            .collect()
                    })
                })
                .collect();
            let expected = postings
                .iter()
                .min_by_key(|rows| rows.len())
                .map(Vec::as_slice);
            let selected = support
                .probe(
                    &pattern,
                    &values,
                    &FormulaLimits::default(),
                    &mut Counters::default(),
                    location(),
                )
                .unwrap();
            assert_eq!(selected, expected);
        }
    }
}

#[test]
fn column_lookup_retains_whole_typed_keys() {
    let tuple = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
        ValueLimits::default(),
    )
    .unwrap();
    let values = [
        Value::Infimum,
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
        tuple,
        Value::Supremum,
    ];
    let predicate = Predicate::new("row", 1).unwrap();
    let mut catalog = SupportCatalog::default();
    for value in &values {
        insert(
            &mut catalog,
            &Atom::new(predicate.clone(), vec![value.clone()]).unwrap(),
        );
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    for (position, value) in values.iter().enumerate() {
        let pattern =
            AtomPattern::new(predicate.clone(), vec![Term::Constant(value.clone())]).unwrap();
        assert_eq!(
            support
                .probe(
                    &pattern,
                    &[],
                    &FormulaLimits::default(),
                    &mut Counters::default(),
                    location()
                )
                .unwrap(),
            Some([position].as_slice())
        );
    }
}

#[test]
fn snapshot_bytes_include_the_borrowed_owner() {
    let mut catalog = SupportCatalog::default();
    for values in [[2, 1], [1, 2], [2, 2]] {
        insert(&mut catalog, &atom(&values));
    }
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let exact = support.bytes;
    drop(support);
    let limits = FormulaLimits {
        max_support_bytes: exact,
        ..FormulaLimits::default()
    };
    assert!(
        catalog
            .snapshot(&limits, &mut Counters::default(), location())
            .is_ok()
    );
    let below = FormulaLimits {
        max_support_bytes: exact - 1,
        ..limits
    };
    assert!(matches!(
        catalog.snapshot(&below, &mut Counters::default(), location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            limit,
            observed,
            location: actual_location,
        }) if limit == (exact - 1) as u128
            && observed == exact as u128
            && actual_location == location()
    ));
}

#[test]
fn indexed_probes_need_no_key_or_equality_buffers() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1, 2]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[1, 2]).predicate().clone(),
        vec![Term::Constant(Value::Number(1)), Term::Variable(0)],
    )
    .unwrap();
    let binding = [Some(Value::Number(2))];
    // The owner alone fits; any temporary key or equality buffer would refuse.
    let bytes = support.bytes;
    let exact = FormulaLimits {
        max_support_bytes: bytes,
        ..FormulaLimits::default()
    };
    assert_eq!(
        support
            .probe(
                &pattern,
                &binding,
                &exact,
                &mut Counters::default(),
                location()
            )
            .unwrap(),
        Some([0].as_slice())
    );
    let below = FormulaLimits {
        max_support_bytes: bytes - 1,
        ..exact
    };
    assert!(matches!(
        support.probe(&pattern, &binding, &below, &mut Counters::default(), location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, location: found,
        }) if observed == bytes as u128 && limit == (bytes - 1) as u128 && found == location()
    ));
}

#[test]
fn a_missing_dictionary_key_does_not_hide_a_later_invalid_slot() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1, 2]));
    let limits = FormulaLimits::default();
    let support = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[1, 2]).predicate().clone(),
        vec![Term::Constant(Value::Number(99)), Term::Variable(1)],
    )
    .unwrap();
    let mut counters = Counters::default();
    let result = support.probe(&pattern, &[None], &limits, &mut counters, location());
    assert!(
        matches!(result, Err(FormulaFailure::UnsafeVariable { variable: 1, location: found }) if found == location())
    );
    assert!(
        counters.accounting.work > 2,
        "the missing lookup prefix remains charged"
    );
}

#[test]
fn probe_work_includes_typed_query_resolution() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1, 2]));
    let support = catalog
        .snapshot(
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(),
        )
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[1, 2]).predicate().clone(),
        vec![Term::Constant(Value::Number(1)), Term::Variable(0)],
    )
    .unwrap();
    let binding = [Some(Value::Number(2))];
    let initial_work = 7;
    let counters_at_entry = || Counters {
        accounting: super::super::Accounting {
            work: initial_work,
            ..Default::default()
        },
        ..Counters::default()
    };
    let mut counters = counters_at_entry();
    support
        .probe(
            &pattern,
            &binding,
            &FormulaLimits::default(),
            &mut counters,
            location(),
        )
        .unwrap();
    assert!(counters.accounting.work > initial_work + pattern.terms().len() as u64);
    let exact = FormulaLimits {
        max_work: counters.accounting.work,
        ..FormulaLimits::default()
    };
    assert_eq!(
        support
            .probe(
                &pattern,
                &binding,
                &exact,
                &mut counters_at_entry(),
                location()
            )
            .unwrap(),
        Some([0].as_slice())
    );
    for maximum in initial_work..counters.accounting.work {
        let below = FormulaLimits {
            max_work: maximum,
            ..exact
        };
        let mut failed = counters_at_entry();
        assert!(matches!(
            support.probe(&pattern, &binding, &below, &mut failed, location()),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work, observed, limit, location: found,
            }) if observed == u128::from(maximum) + 1
                && limit == u128::from(maximum) && found == location()
        ));
        // The outer term scan and the inner dictionary prefix are both spent,
        // even though no posting is returned. Mapping must not add the prefix
        // twice to the refusal's original cumulative amount.
        assert_eq!(failed.accounting.work, maximum);
    }
}

#[test]
fn relation_refusal_retains_the_source_location() {
    let error = failure(Failure::Allocation, location());
    let FormulaFailure::SupportRelation {
        error: Failure::Allocation,
        location: found,
    } = error
    else {
        panic!("typed relation refusal");
    };
    assert_eq!(found, location());
}

#[test]
fn membership_preserves_cumulative_work_limits() {
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[3]));
    let limits = FormulaLimits::default();
    let support = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let pattern =
        AtomPattern::new(atom(&[3]).predicate().clone(), vec![Term::Variable(0)]).unwrap();
    let values = [Some(Value::Number(3))];
    let key = pattern.key(values.as_slice()).unwrap();
    let initial = 7;
    let fresh = || Counters {
        accounting: super::super::Accounting {
            work: initial,
            ..Default::default()
        },
        ..Counters::default()
    };
    let mut counters = fresh();
    assert!(
        support
            .contains(&key, &limits, &mut counters, location())
            .unwrap()
    );
    assert!(counters.accounting.work > initial);
    let exact = FormulaLimits {
        max_work: counters.accounting.work,
        ..limits
    };
    assert!(
        support
            .contains(&key, &exact, &mut fresh(), location())
            .unwrap()
    );
    let short = FormulaLimits {
        max_work: counters.accounting.work - 1,
        ..exact
    };
    assert!(
        matches!(support.contains(&key, &short, &mut fresh(), location()), Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location: found }) if observed == u128::from(counters.accounting.work) && limit == u128::from(counters.accounting.work - 1) && found == location())
    );
}

#[test]
fn shared_probe_refusal_preserves_its_dictionary_prefix() {
    let mut catalog = SupportCatalog::default();
    for value in 0..8 {
        insert(&mut catalog, &atom(&[value]));
    }
    let limits = FormulaLimits::default();
    let support = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[7]).predicate().clone(),
        vec![Term::Constant(Value::Number(7))],
    )
    .unwrap();
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut counters =
        Counters::with_allowance(allowance.clone(), &zetesis_cpu::Cancellation::default());
    let check = FormulaLimits {
        max_work: 2,
        ..limits
    };
    let result = support.probe(&pattern, &[], &check, &mut counters, location());
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 3,
            limit: 2,
            ..
        })
    ));
    // Predicate resolution now precedes the key and dictionary probes.
    // Every admitted prefix reaches the shared receipt exactly.
    assert_eq!(counters.accounting.work, 2);
    assert_eq!(allowance.statistics().work, 2);
}

#[test]
fn shared_probe_matches_local_execution() {
    let mut catalog = SupportCatalog::default();
    for value in 0..8 {
        insert(&mut catalog, &atom(&[value]));
    }
    let limits = FormulaLimits::default();
    let support = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let pattern = AtomPattern::new(
        atom(&[7]).predicate().clone(),
        vec![Term::Constant(Value::Number(7))],
    )
    .unwrap();
    let mut local = Counters::default();
    let expected = support
        .probe(&pattern, &[], &limits, &mut local, location())
        .unwrap();
    assert_eq!(expected, Some([7].as_slice()));
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut shared =
        Counters::with_allowance(allowance.clone(), &zetesis_cpu::Cancellation::default());
    let actual = support
        .probe(&pattern, &[], &limits, &mut shared, location())
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(shared.accounting.work, local.accounting.work);
    assert_eq!(allowance.statistics().work, local.accounting.work);
}

#[test]
fn round_heads_share_one_discovery_without_entering_the_borrowed_snapshot() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1]));
    let predicate = Predicate::new("row", 1).unwrap();
    let pattern = AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap();
    let values = [Value::Number(2)];
    let key = pattern.key(values.as_slice()).unwrap();
    let pattern = testing::admit_pattern(&mut catalog, &pattern, &mut counters, location());
    {
        let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
        {
            let mut computation = Computation::new(&mut append, &support);
            let pattern = computation
                .static_pattern(pattern, &limits, &mut counters, location())
                .unwrap();
            let binding = testing::binding(
                &[Some(Value::Number(2))],
                &mut computation,
                &mut counters,
                location(),
            );
            let first = computation
                .atom(pattern, &binding, &limits, &mut counters, location())
                .unwrap();
            computation
                .support(&first, &limits, &mut counters, location())
                .unwrap();
            let retained = relations.current_bytes();
            let repeated = computation
                .atom(pattern, &binding, &limits, &mut counters, location())
                .unwrap();
            computation
                .support(&repeated, &limits, &mut counters, location())
                .unwrap();
            assert_eq!(first.position, repeated.position);
            assert!(first.scope.same(&repeated.scope));
            assert_eq!(relations.current_bytes(), retained);
            assert!(
                computation
                    .contains_pattern(pattern, &binding, &limits, &mut counters, location())
                    .unwrap()
            );
        }
        assert_eq!(append.atoms().count(), 1);
        assert!(
            !relations
                .contains(&key, &limits, &mut counters, location())
                .unwrap()
        );
        assert_eq!(relations.row_count(&predicate), 1);
    }
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    assert_eq!(relations.row_count(&predicate), 2);
    assert_eq!(relations.old_rows(&predicate), 1);
    assert!(
        relations
            .contains(&key, &limits, &mut counters, location())
            .unwrap()
    );
}

#[test]
fn round_publication_orders_new_rows_by_typed_identity() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let predicate = Predicate::new("row", 1).unwrap();
    let pattern = AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap();
    let pattern = testing::admit_pattern(&mut catalog, &pattern, &mut counters, location());
    {
        let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
        {
            let mut computation = Computation::new(&mut append, &support);
            let pattern = computation
                .static_pattern(pattern, &limits, &mut counters, location())
                .unwrap();
            for value in [3, 1, 2] {
                let binding = testing::binding(
                    &[Some(Value::Number(value))],
                    &mut computation,
                    &mut counters,
                    location(),
                );
                let head = computation
                    .atom(pattern, &binding, &limits, &mut counters, location())
                    .unwrap();
                computation
                    .support(&head, &limits, &mut counters, location())
                    .unwrap();
            }
        }
        append.order(&limits, &mut counters, location()).unwrap();
    }
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let actual: Vec<_> = relations
        .rows(&predicate)
        .map(|row| row.value(0).unwrap().descriptor())
        .collect();
    assert_eq!(
        actual,
        vec![
            ValueNodeRef::Number(1),
            ValueNodeRef::Number(2),
            ValueNodeRef::Number(3)
        ]
    );
}

fn signed_row(name: &str, sign: Sign, value: i32) -> Atom {
    Atom::new(
        Predicate::with_sign(name, 1, sign).unwrap(),
        vec![Value::Number(value)],
    )
    .unwrap()
}

#[test]
fn predicate_runs_survive_new_directory_insertions() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    for name in ["b", "z"] {
        insert(&mut catalog, &signed_row(name, Sign::Positive, 0));
    }
    {
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        for (name, sign) in [
            ("z", Sign::Positive),
            ("b", Sign::Negative),
            ("c", Sign::Positive),
            ("b", Sign::Positive),
            ("a", Sign::Positive),
        ] {
            for value in [2, 1] {
                append
                    .atom(
                        (&signed_row(name, sign, value)).into(),
                        &limits,
                        &mut counters,
                        location(),
                    )
                    .unwrap();
            }
        }
        append.order(&limits, &mut counters, location()).unwrap();
    }
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    for (name, sign, old) in [
        ("a", Sign::Positive, 0),
        ("b", Sign::Positive, 1),
        ("b", Sign::Negative, 0),
        ("c", Sign::Positive, 0),
        ("z", Sign::Positive, 1),
    ] {
        let predicate = Predicate::with_sign(name, 1, sign).unwrap();
        let expected = if old == 0 {
            vec![ValueNodeRef::Number(1), ValueNodeRef::Number(2)]
        } else {
            vec![
                ValueNodeRef::Number(0),
                ValueNodeRef::Number(1),
                ValueNodeRef::Number(2),
            ]
        };
        assert_eq!(relations.old_rows(&predicate), old);
        assert_eq!(
            relations
                .rows(&predicate)
                .map(|row| row.value(0).unwrap().descriptor())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(relations.predicates().count(), 5);
}

fn publication_work(grouped: bool) -> u64 {
    let limits = FormulaLimits::default();
    let mut catalog = SupportCatalog::default();
    for name in ["a", "z"] {
        insert(&mut catalog, &signed_row(name, Sign::Positive, 0));
    }
    {
        let mut counters = Counters::default();
        let (_, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
        for name in ["a", "z"] {
            for value in [1, 2] {
                append
                    .atom(
                        (&signed_row(name, Sign::Positive, value)).into(),
                        &limits,
                        &mut counters,
                        location(),
                    )
                    .unwrap();
            }
        }
    }
    if !grouped {
        // Change only cross-predicate publication order: each relation still
        // receives 1 then 2, with identical canonical identities and capacities.
        catalog.pending.swap(1, 2);
    }
    let mut counters = Counters::default();
    catalog.publish(&limits, &mut counters, location()).unwrap();
    let work = counters.accounting.work;
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    for name in ["a", "z"] {
        let predicate = Predicate::new(name, 1).unwrap();
        assert_eq!(
            relations
                .rows(&predicate)
                .map(|row| row.value(0).unwrap().descriptor())
                .collect::<Vec<_>>(),
            vec![
                ValueNodeRef::Number(0),
                ValueNodeRef::Number(1),
                ValueNodeRef::Number(2)
            ]
        );
    }
    work
}

#[test]
fn predicate_runs_avoid_repeated_directory_searches() {
    // A per-row directory search performs the same total work for these two
    // populations. Reusing each contiguous run removes those repeated searches.
    assert!(publication_work(true) < publication_work(false));
}

#[test]
fn append_capacity_remains_charged_to_live_snapshot_queries() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    insert(&mut catalog, &atom(&[1]));
    let predicate = Predicate::new("row", 1).unwrap();
    let pattern = AtomPattern::new(predicate, vec![Term::Variable(0)]).unwrap();
    let compiled = testing::admit_pattern(&mut catalog, &pattern, &mut counters, location());
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let values = [Value::String("new payload".repeat(100))];
    let key = pattern.key(values.as_slice()).unwrap();
    let workspace = support.workspace_bytes();
    {
        let mut computation = Computation::new(&mut append, &support);
        let pattern = computation
            .static_pattern(compiled, &limits, &mut counters, location())
            .unwrap();
        let binding = testing::binding(
            &[Some(values[0].clone())],
            &mut computation,
            &mut counters,
            location(),
        );
        let head = computation
            .atom(pattern, &binding, &limits, &mut counters, location())
            .unwrap();
        computation
            .support(&head, &limits, &mut counters, location())
            .unwrap();
    }
    // Only retained owner growth remains: transient argument and assignment
    // frames cannot make the subsequent live-snapshot refusal pass by accident.
    assert_eq!(support.workspace_bytes(), workspace);
    assert!(relations.current_bytes() > relations.bytes);
    let bounded = FormulaLimits {
        max_support_bytes: relations.bytes,
        ..limits
    };
    assert!(matches!(
        support.contains(&key, &bounded, &mut counters, location()),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
}

mod storage;
