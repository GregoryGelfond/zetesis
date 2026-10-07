use super::*;
use crate::atom_interner::AtomInterner;
use crate::catalog::TermAssignment;
use crate::test_support::PERMIT;
use crate::{
    Atom, Predicate, Sign, TemplateComponents, TemplateTerm, Value, ValueLimits, ValueNode,
};
use std::convert::Infallible;

fn limits() -> Limits {
    Limits {
        max_atoms: 128,
        max_bytes: 1 << 22,
    }
}

fn unrestricted() -> TermLimits {
    TermLimits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}

fn columns() -> Vec<Option<RowColumn>> {
    vec![
        Some(RowColumn {
            input: 0,
            column: 0,
        }),
        None,
        Some(RowColumn {
            input: 0,
            column: 0,
        }),
    ]
}

struct Fixture {
    owner: AtomInterner,
    components: TemplateComponents,
    count: usize,
}

impl Fixture {
    fn new(values: &[Value]) -> Self {
        let mut owner = AtomInterner::new();
        let source = Predicate::new("source", 2).unwrap();
        let unused = Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(99)],
            ValueLimits::default(),
        )
        .unwrap();
        for value in values {
            let atom = Atom::new(source.clone(), vec![value.clone(), unused.clone()]).unwrap();
            owner
                .entry_atom_with(&atom, limits(), PERMIT)
                .unwrap()
                .insert_with(limits(), PERMIT)
                .unwrap();
        }
        let mut append = owner.appender();
        let head = append
            .declare_predicate_with(
                &Predicate::with_sign("head", 3, Sign::Negative).unwrap(),
                limits(),
                PERMIT,
            )
            .unwrap();
        let constant = append
            .import_term_with((&Value::Number(7)).into(), unrestricted(), limits(), PERMIT)
            .unwrap();
        let read = append.read();
        let terms = [
            TemplateTerm::Variable(1),
            TemplateTerm::Constant(read.term(&constant).unwrap()),
            TemplateTerm::Variable(1),
        ];
        let pattern = PatternRef::from_parts(read.predicate(&head).unwrap(), &terms).unwrap();
        let mut components = TemplateComponents::new(read, usize::MAX).unwrap();
        components
            .append_pattern_with(read, pattern, usize::MAX, PERMIT)
            .unwrap();
        owner.commit_with(limits(), PERMIT).unwrap();
        Self {
            owner,
            components,
            count: values.len(),
        }
    }

    fn run<R>(
        &mut self,
        run: impl FnOnce(&mut AtomAppender<'_>, PatternRef<'_>, &Relation<'_>) -> R,
    ) -> R {
        self.run_pair(|append, pattern, source, _| run(append, pattern, source))
    }

    fn run_pair<R>(
        &mut self,
        run: impl FnOnce(&mut AtomAppender<'_>, PatternRef<'_>, &Relation<'_>, &Relation<'_>) -> R,
    ) -> R {
        let (prefix, mut append) = self.owner.split();
        let indices: Vec<_> = (0..self.count).collect();
        let relation = Relation::from_catalog_refs(
            prefix.get(0).unwrap().predicate(),
            prefix.atoms(),
            &indices,
            crate::relation::Limits::default(),
        )
        .unwrap();
        let other = Relation::from_catalog_refs(
            prefix.get(0).unwrap().predicate(),
            prefix.atoms(),
            &indices,
            crate::relation::Limits::default(),
        )
        .unwrap();
        let bound = self.components.bind_with(prefix.read(), PERMIT).unwrap();
        run(&mut append, bound.pattern(0).unwrap(), &relation, &other)
    }
}

fn values() -> [Value; 2] {
    [Value::Number(11), Value::Number(12)]
}

fn expected(value: Value) -> Atom {
    Atom::new(
        Predicate::with_sign("head", 3, Sign::Negative).unwrap(),
        vec![value.clone(), Value::Number(7), value],
    )
    .unwrap()
}

fn frame(append: &AtomAppender<'_>, row: Row<'_, '_>) -> TermAssignment {
    let read = append.read();
    let key = read.term_key(row.atom().values().at(0).unwrap()).unwrap();
    let mut values = read.assignment();
    values.resize_with(2, usize::MAX, PERMIT).unwrap();
    values.set_with(1, &key, PERMIT).unwrap();
    values
}

#[test]
fn projected_rows_preserve_scalar_identity() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        // First row already exists; the next is new and then repeated.
        let original = frame(append, source.row(1).unwrap());
        let old = append
            .insert_pattern_with(
                pattern,
                original.as_slice(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap();
        for index in [1, 0, 1, 0] {
            let row = source.row(index).unwrap();
            let projected = append
                .insert_prepared_rows_with(&prepared, &[row], limits(), PERMIT)
                .unwrap();
            let values = frame(append, row);
            let scalar = append
                .insert_pattern_with(pattern, values.as_slice(), unrestricted(), limits(), PERMIT)
                .unwrap();
            assert_eq!(projected, scalar);
            assert_eq!(
                append.get(projected).unwrap(),
                expected(if index == 0 {
                    Value::Number(11)
                } else {
                    Value::Number(12)
                })
            );
            if index == 1 {
                assert_eq!(projected, old);
            }
        }
        assert_eq!(append.len(), 4);
    });
}

#[test]
fn projected_rows_require_the_exact_prepared_relation() {
    Fixture::new(&values()).run_pair(|append, pattern, source, other| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            append.insert_prepared_rows_with(&prepared, &[other.row(0).unwrap()], limits(), PERMIT),
            Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog
            )))
        ));
        assert!(matches!(
            append.insert_prepared_rows_with(&prepared, &[], limits(), PERMIT),
            Err(AssignedFailure::Interner(Failure::Catalog(
                storage::Fault::Shape
            )))
        ));
        assert_eq!(append.len(), 2);
    });
}

#[test]
fn row_preparation_rejects_inconsistent_repeated_slots() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let mut mapping = columns();
        mapping[2] = Some(RowColumn {
            input: 0,
            column: 1,
        });
        assert!(matches!(
            append.prepare_row_pattern_with(
                pattern,
                vec![source],
                mapping,
                unrestricted(),
                limits(),
                PERMIT
            ),
            Err(AssignedFailure::Interner(Failure::Catalog(
                storage::Fault::Shape
            )))
        ));
        assert_eq!(append.len(), 2);
    });
}

#[test]
fn finite_row_limits_apply_only_when_the_argument_is_selected() {
    let compound = Value::from_nodes(
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(8)],
        ValueLimits::default(),
    )
    .unwrap();
    Fixture::new(&[Value::Number(11), compound]).run(|append, pattern, source| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                TermLimits {
                    max_nodes: 1,
                    ..TermLimits::default()
                },
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        assert!(prepared.check_terms);
        append
            .insert_prepared_rows_with(&prepared, &[source.row(0).unwrap()], limits(), PERMIT)
            .unwrap();
        let before = append.len();
        assert!(matches!(
            append.insert_prepared_rows_with(
                &prepared,
                &[source.row(1).unwrap()],
                limits(),
                PERMIT
            ),
            Err(AssignedFailure::Interner(Failure::Catalog(
                storage::Fault::Value(crate::ValueError::Limit {
                    resource: crate::ValueResource::Nodes,
                    ..
                })
            )))
        ));
        assert_eq!(append.len(), before);
    });
}

#[test]
fn row_preparation_moves_metadata_without_visiting_rows() {
    fn preparation(count: i32) -> usize {
        let values: Vec<_> = (0..count).map(Value::Number).collect();
        Fixture::new(&values).run(|append, pattern, source| {
            let sources = vec![source];
            let mapping = columns();
            let sources_pointer = sources.as_ptr();
            let mapping_pointer = mapping.as_ptr();
            let mut work = 0;
            let prepared = append
                .prepare_row_pattern_with(
                    pattern,
                    sources,
                    mapping,
                    unrestricted(),
                    limits(),
                    || {
                        work += 1;
                        Ok::<_, Infallible>(())
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(prepared.sources.as_ptr(), sources_pointer);
            assert_eq!(prepared.columns.as_ptr(), mapping_pointer);
            assert!(!prepared.check_terms);
            assert_eq!(
                prepared.storage_bytes(),
                size_of::<PreparedRows<'_, '_>>() as u128
                    + prepared.sources.capacity() as u128 * size_of::<&Relation<'_>>() as u128
                    + prepared.columns.capacity() as u128 * size_of::<Option<RowColumn>>() as u128
            );
            work
        })
    }
    assert_eq!(preparation(1), preparation(32));
}

#[test]
fn every_row_publication_stop_preserves_publication_coherence() {
    let total = Fixture::new(&values()).run(|append, pattern, source| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        let mut work = 0;
        append
            .insert_prepared_rows_with(&prepared, &[source.row(0).unwrap()], limits(), || {
                work += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap();
        work
    });
    let mut retained_canonical_row = false;
    for cutoff in 0..total {
        Fixture::new(&values()).run(|append, pattern, source| {
            let prepared = append.prepare_row_pattern_with(pattern, vec![source], columns(), unrestricted(), limits(), PERMIT).unwrap().unwrap();
            let initial = append.len();
            let mut work = 0;
            assert!(matches!(append.insert_prepared_rows_with(&prepared, &[source.row(0).unwrap()], limits(), || {
                if work == cutoff { Err(cutoff) } else { work += 1; Ok(()) }
            }), Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cutoff));
            assert_eq!(work, cutoff);
            assert_eq!(append.len(), initial);
            assert_eq!(append.index.nodes.len(), initial);
            assert_eq!(append.discovery.nodes.len(), initial);
            let binding = frame(append, source.row(0).unwrap());
            let projected = Projected {
                predicate: prepared.pattern.pattern.predicate,
                values: binding.as_slice().slots,
                arguments: ProjectionSource::Admitted(
                    prepared.pattern.source,
                    &prepared.pattern.pattern.terms,
                ),
            };
            let mut permit = PERMIT;
            let identity = crate::catalog::interner::query::Query::Projected(&projected)
                .identity_with(append.store, &mut permit)
                .unwrap();
            retained_canonical_row |= matches!(
                identity,
                crate::catalog::interner::query::Identity::Local(atom) if atom.present().is_some()
            );
            assert_eq!(append.find_pattern_with(pattern, binding.as_slice(), limits(), PERMIT).unwrap(), None);
            let id = append.insert_prepared_rows_with(&prepared, &[source.row(0).unwrap()], limits(), PERMIT).unwrap();
            assert_eq!(id, initial);
            let values = frame(append, source.row(0).unwrap());
            assert_eq!(append.find_pattern_with(pattern, values.as_slice(), limits(), PERMIT).unwrap(), Some(id));
            assert_eq!(append.len(), initial + 1);
        });
    }
    assert!(
        retained_canonical_row,
        "the stop matrix reaches canonical-only publication"
    );
}

#[test]
fn projected_query_header_counts_at_an_occupied_boundary() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let prepared = append.prepare_row_pattern_with(pattern, vec![source], columns(), unrestricted(), limits(), PERMIT).unwrap().unwrap();
        let rows = [source.row(0).unwrap()];
        let id = append.insert_prepared_rows_with(&prepared, &rows, limits(), PERMIT).unwrap();
        let required = append.storage_bytes() + Projected::HEADER_BYTES + size_of::<RowArguments<'_>>() as u128 + crate::catalog::interner::PREPARED_BYTES;
        assert_eq!(append.insert_prepared_rows_with(&prepared, &rows, Limits { max_bytes: required, ..limits() }, PERMIT).unwrap(), id);
        assert!(matches!(append.insert_prepared_rows_with(&prepared, &rows, Limits { max_bytes: required - 1, ..limits() }, PERMIT), Err(AssignedFailure::Interner(Failure::Bytes { required: actual, .. })) if actual == required));
    });
}

#[test]
fn finite_byte_limits_are_checked_on_an_occupied_row() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let row = source.row(0).unwrap();
        let values = frame(append, row);
        let old = append
            .insert_pattern_with(pattern, values.as_slice(), unrestricted(), limits(), PERMIT)
            .unwrap();
        // Constant 7 requires six encoded+rendered bytes, while 11 needs seven.
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                TermLimits {
                    max_bytes: 6,
                    ..TermLimits::default()
                },
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            append.insert_prepared_rows_with(&prepared, &[row], limits(), PERMIT),
            Err(AssignedFailure::Interner(Failure::Catalog(
                storage::Fault::Value(crate::ValueError::Limit {
                    resource: crate::ValueResource::Bytes,
                    observed: 7,
                    limit: 6
                })
            )))
        ));
        assert_eq!(append.get(old).unwrap(), expected(Value::Number(11)));
        assert_eq!(append.len(), 3);
    });
}

#[test]
fn prepared_rows_keep_an_older_source_valid_as_the_writer_grows() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        let key = append
            .import_term_with(
                (&Value::Number(31)).into(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap();
        assert!(source.canonical_read().unwrap().term(&key).is_err());
        let mut later = append.read().assignment();
        later.resize_with(2, usize::MAX, PERMIT).unwrap();
        later.set_with(1, &key, PERMIT).unwrap();
        let new = append
            .insert_pattern_with(pattern, later.as_slice(), unrestricted(), limits(), PERMIT)
            .unwrap();
        let old = append
            .insert_prepared_rows_with(&prepared, &[source.row(0).unwrap()], limits(), PERMIT)
            .unwrap();
        assert_ne!(new, old);
        assert_eq!(append.get(new).unwrap(), expected(Value::Number(31)));
        assert_eq!(append.get(old).unwrap(), expected(Value::Number(11)));
    });
}

#[test]
fn prepared_rows_refuse_a_different_writer() {
    Fixture::new(&values()).run(|append, pattern, source| {
        let prepared = append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                PERMIT,
            )
            .unwrap()
            .unwrap();
        let mut other = AtomInterner::new();
        assert!(matches!(
            other.appender().insert_prepared_rows_with(
                &prepared,
                &[source.row(0).unwrap()],
                limits(),
                PERMIT
            ),
            Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog
            )))
        ));
        assert!(other.is_empty());
    });
}

#[test]
fn every_row_preparation_stop_leaves_the_writer_unchanged() {
    let total = Fixture::new(&values()).run(|append, pattern, source| {
        let mut work = 0;
        append
            .prepare_row_pattern_with(
                pattern,
                vec![source],
                columns(),
                unrestricted(),
                limits(),
                || {
                    work += 1;
                    Ok::<_, Infallible>(())
                },
            )
            .unwrap()
            .unwrap();
        work
    });
    for cutoff in 0..total {
        Fixture::new(&values()).run(|append, pattern, source| {
            let bytes = append.storage_bytes();
            let peak = append.storage_peak_bytes();
            let mut work = 0;
            assert!(matches!(append.prepare_row_pattern_with(pattern, vec![source], columns(), unrestricted(), limits(), || {
                if work == cutoff { Err(cutoff) } else { work += 1; Ok(()) }
            }), Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cutoff));
            assert_eq!(work, cutoff);
            assert_eq!(append.storage_bytes(), bytes);
            assert_eq!(append.storage_peak_bytes(), peak);
            assert_eq!(append.len(), 2);
        });
    }
}

#[test]
fn canonical_admission_bounds_combined_lengths_before_projection() {
    let mut owner = AtomInterner::new();
    let mut append = owner.appender();
    let mut key = append
        .import_term_with((&Value::Number(0)).into(), unrestricted(), limits(), PERMIT)
        .unwrap();
    let mut values = append.read().assignment();
    values.resize_with(1, usize::MAX, PERMIT).unwrap();
    let mut refused_bytes = false;
    for _ in 0..usize::BITS {
        values.set_with(0, &key, PERMIT).unwrap();
        match append.construct_term_with(
            crate::ValueNodeRef::Tuple { arity: 2 },
            values.as_slice(),
            &[0, 0],
            unrestricted(),
            limits(),
            PERMIT,
        ) {
            Ok(next) => key = next,
            Err(AssignedFailure::Interner(Failure::Catalog(storage::Fault::Value(
                crate::ValueError::Limit {
                    resource: crate::ValueResource::Bytes,
                    observed,
                    limit,
                },
            )))) => {
                assert!(observed > usize::MAX as u128);
                assert_eq!(limit, usize::MAX);
                refused_bytes = true;
                break;
            }
            Err(error) => panic!("unexpected canonical refusal: {error}"),
        }
    }
    assert!(refused_bytes);
    assert!(append.is_empty());
}

#[test]
fn projected_rows_reuse_published_paths_without_repeating_preparation() {
    fn run(reuse: bool) -> usize {
        let values: Vec<_> = (0..64).map(Value::Number).collect();
        Fixture::new(&values).run(|append, pattern, source| {
            let prepared = append
                .prepare_row_pattern_with(
                    pattern,
                    vec![source],
                    columns(),
                    unrestricted(),
                    limits(),
                    PERMIT,
                )
                .unwrap()
                .unwrap();
            let mut work = 0;
            for index in 0..values.len() {
                if !reuse {
                    *append.spines = crate::catalog::interner::Spines::default();
                }
                let id = append
                    .insert_prepared_rows_with(
                        &prepared,
                        &[source.row(index).unwrap()],
                        limits(),
                        || {
                            work += 1;
                            Ok::<_, Infallible>(())
                        },
                    )
                    .unwrap();
                assert_eq!(id, values.len() + index);
                assert_eq!(append.get(id).unwrap(), expected(values[index].clone()));
                assert!(append.spines.semantic.is_some());
                assert!(append.spines.discovery.is_some());
            }
            assert_eq!(append.len(), 128);
            work
        })
    }
    let warm = run(true);
    let cold = run(false);
    assert!(warm < cold, "{warm} versus {cold}");
}
