use super::*;
use crate::atom_interner::AtomInterner;
use crate::catalog::TermAssignment;
use crate::test_support::PERMIT;
use crate::{
    Atom, Predicate, Sign, TemplateComponents, TemplateTerm, Value, ValueLimits, ValueNode,
};
use std::convert::Infallible;

mod locations;

fn limits() -> Limits {
    Limits {
        max_atoms: 128,
        max_bytes: 1 << 22,
    }
}

struct Fixture {
    owner: AtomInterner,
    components: TemplateComponents,
    values: TermAssignment,
}

impl Fixture {
    fn new() -> Self {
        let mut owner = AtomInterner::new();
        let mut append = owner.appender();
        let predicate = append
            .declare_predicate_with(
                &Predicate::with_sign("prepared", 3, Sign::Negative).unwrap(),
                limits(),
                PERMIT,
            )
            .unwrap();
        let constant = append
            .import_term_with(
                (&Value::Number(7)).into(),
                TermLimits::default(),
                limits(),
                PERMIT,
            )
            .unwrap();
        let variable = append
            .import_term_with(
                (&Value::Number(11)).into(),
                TermLimits::default(),
                limits(),
                PERMIT,
            )
            .unwrap();
        let read = append.read();
        let mut values = read.assignment();
        values.resize_with(2, usize::MAX, PERMIT).unwrap();
        values.set_with(1, &variable, PERMIT).unwrap();
        // Slot zero remains unbound and is deliberately outside the projection.
        let mut components = TemplateComponents::new(read, usize::MAX).unwrap();
        let terms = [
            TemplateTerm::Variable(1),
            TemplateTerm::Constant(read.term(&constant).unwrap()),
            TemplateTerm::Variable(1),
        ];
        components
            .append_pattern_with(
                read,
                PatternRef::from_parts(read.predicate(&predicate).unwrap(), &terms).unwrap(),
                usize::MAX,
                PERMIT,
            )
            .unwrap();
        owner.commit_with(limits(), PERMIT).unwrap();
        owner.restart_storage_peak();
        Self {
            owner,
            components,
            values,
        }
    }

    fn with_pattern<R>(
        &mut self,
        run: impl FnOnce(&mut AtomAppender<'_>, PatternRef<'_>, &mut TermAssignment) -> R,
    ) -> R {
        let (prefix, mut append) = self.owner.split();
        let pattern = self
            .components
            .bind_with(prefix.read(), PERMIT)
            .unwrap()
            .pattern(0)
            .unwrap();
        run(&mut append, pattern, &mut self.values)
    }
}

fn expected(number: i32) -> Atom {
    Atom::new(
        Predicate::with_sign("prepared", 3, Sign::Negative).unwrap(),
        vec![
            Value::Number(number),
            Value::Number(7),
            Value::Number(number),
        ],
    )
    .unwrap()
}

#[test]
fn prepared_columns_borrow_the_admitted_metadata() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, _| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        assert!(std::ptr::eq(
            prepared.pattern,
            pattern.admitted_parts().unwrap().1
        ));
        let terms = [TemplateTerm::Variable(0); 3];
        let parts = PatternRef::from_parts(pattern.predicate(), &terms).unwrap();
        assert!(
            append
                .prepare_pattern_with(parts, TermLimits::default(), limits(), PERMIT)
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn prepared_instances_preserve_canonical_discovery() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        for number in [11, 12, 11, 42] {
            // This key can be newer than the pattern's snapshot: preparation is
            // attached to the live writer, not to that immutable read prefix.
            let key = append
                .import_term_with(
                    (&Value::Number(number)).into(),
                    TermLimits::default(),
                    limits(),
                    PERMIT,
                )
                .unwrap();
            values.set_with(1, &key, PERMIT).unwrap();
            let first = append
                .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
                .unwrap();
            let second = append
                .insert_pattern_with(
                    pattern,
                    values.as_slice(),
                    TermLimits::default(),
                    limits(),
                    PERMIT,
                )
                .unwrap();
            assert_eq!(first, second);
            assert_eq!(append.get(first).unwrap(), expected(number));
        }
        assert_eq!(append.len(), 3);
    });
}

#[test]
fn prepared_variables_keep_typed_frame_failures() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        values.clear_with(1, PERMIT).unwrap();
        assert!(matches!(
            append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT),
            Err(AssignedFailure::Assignment(AssignmentError::Unbound {
                slot: 1
            }))
        ));
        assert!(matches!(
            append.insert_prepared_pattern_with(
                &prepared,
                values.prefix(1).unwrap(),
                limits(),
                PERMIT
            ),
            Err(AssignedFailure::Assignment(AssignmentError::Slot {
                slot: 1,
                len: 1
            }))
        ));
        let foreign = AtomInterner::new();
        let foreign = foreign.read().assignment();
        assert!(matches!(
            append.insert_prepared_pattern_with(&prepared, foreign.as_slice(), limits(), PERMIT),
            Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::ForeignCatalog
            )))
        ));
        assert!(append.is_empty());
    });
}

#[test]
fn prepared_owner_is_stricter_than_shared_vocabulary() {
    let pattern = crate::AtomPattern::new(
        Predicate::new("p", 1).unwrap(),
        vec![crate::Term::Constant(Value::Number(7))],
    )
    .unwrap();
    let program = crate::Program::new(
        vec![crate::Template::new(
            Some(pattern),
            vec![],
            vec![],
            vec![],
            vec![],
        )],
        crate::AdmissionLimits::default(),
    )
    .unwrap();
    let mut first = AtomInterner::for_program(&program, 1 << 22).unwrap();
    let mut second = AtomInterner::for_program(&program, 1 << 22).unwrap();
    let pattern = program.templates().at(0).unwrap().head().unwrap();
    let prepared = first
        .appender()
        .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
        .unwrap()
        .unwrap();
    let values = second.read().assignment();
    // A same-vocabulary assignment is valid in the second writer, while the
    // capability deliberately belongs only to the first writer.
    assert!(matches!(
        second.appender().insert_prepared_pattern_with(
            &prepared,
            values.as_slice(),
            limits(),
            PERMIT
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
    assert!(second.is_empty());
}

#[test]
fn prepared_instances_recheck_variable_measures() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let narrow = TermLimits {
            max_nodes: 1,
            ..TermLimits::default()
        };
        let prepared = append
            .prepare_pattern_with(pattern, narrow, limits(), PERMIT)
            .unwrap()
            .unwrap();
        let compound = Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(8)],
            ValueLimits::default(),
        )
        .unwrap();
        let key = append
            .import_term_with((&compound).into(), TermLimits::default(), limits(), PERMIT)
            .unwrap();
        values.set_with(1, &key, PERMIT).unwrap();
        append
            .insert_pattern_with(
                pattern,
                values.as_slice(),
                TermLimits::default(),
                limits(),
                PERMIT,
            )
            .unwrap();
        assert!(matches!(
            append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT),
            Err(AssignedFailure::Interner(Failure::Catalog(
                crate::catalog::Error::Value(crate::ValueError::Limit {
                    resource: crate::ValueResource::Nodes,
                    observed: 2,
                    limit: 1,
                })
            )))
        ));
        assert_eq!(append.len(), 1);
    });
}

#[test]
fn prepared_constants_obey_the_fixed_logical_limits() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, _| {
        assert!(matches!(
            append.prepare_pattern_with(
                pattern,
                TermLimits {
                    max_nodes: 0,
                    ..TermLimits::default()
                },
                limits(),
                PERMIT
            ),
            Err(AssignedFailure::Interner(Failure::Catalog(
                crate::catalog::Error::Value(crate::ValueError::Limit {
                    resource: crate::ValueResource::Nodes,
                    observed: 1,
                    limit: 0,
                })
            )))
        ));
        assert!(append.is_empty());
    });
}

fn insertion_work() -> usize {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        let mut work = 0;
        append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), || {
                work += 1;
                PERMIT()
            })
            .unwrap();
        work
    })
}

#[test]
fn every_prepared_insertion_stop_withholds_discovery() {
    for cut in 0..insertion_work() {
        let mut fixture = Fixture::new();
        fixture.with_pattern(|append, pattern, values| {
            let prepared = append.prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT).unwrap().unwrap();
            let mut work = 0;
            assert!(matches!(append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), || {
                if work == cut { return Err(cut); }
                work += 1; Ok(())
            }), Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cut));
            assert!(append.is_empty(), "cut {cut}");
            assert_eq!(append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT).unwrap(), 0);
            assert_eq!(append.get(0).unwrap(), expected(11));
        });
    }
}

#[test]
fn prepared_occupied_storage_has_an_exact_boundary() {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append.prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT).unwrap().unwrap();
        append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT).unwrap();
        append.restart_storage_peak();
        let retained = append.storage_bytes();
        let required = retained + Projected::HEADER_BYTES + super::super::super::PREPARED_BYTES;
        assert_eq!(append.insert_prepared_pattern_with(&prepared, values.as_slice(), Limits { max_bytes: required, ..limits() }, PERMIT).unwrap(), 0);
        assert_eq!(append.storage_peak_bytes(), required);
        assert_eq!(append.storage_bytes(), retained);
        assert!(matches!(append.insert_prepared_pattern_with(&prepared, values.as_slice(), Limits { max_bytes: required - 1, ..limits() }, PERMIT),
            Err(AssignedFailure::Interner(Failure::Bytes { required: actual, limit })) if actual == required && limit == required - 1));
        assert_eq!(append.len(), 1);
    });
}

fn sequence(prepared: bool) -> (usize, Vec<Atom>) {
    let mut fixture = Fixture::new();
    fixture.with_pattern(|append, pattern, values| {
        let mut work = 0;
        let prepared = prepared.then(|| {
            append
                .prepare_pattern_with(pattern, TermLimits::default(), limits(), || {
                    work += 1;
                    PERMIT()
                })
                .unwrap()
                .unwrap()
        });
        for number in 0..64 {
            let key = append
                .import_term_with(
                    (&Value::Number(number)).into(),
                    TermLimits::default(),
                    limits(),
                    PERMIT,
                )
                .unwrap();
            values.set_with(1, &key, PERMIT).unwrap();
            let mut before = || {
                work += 1;
                Ok::<_, Infallible>(())
            };
            if let Some(prepared) = &prepared {
                append
                    .insert_prepared_pattern_with(
                        prepared,
                        values.as_slice(),
                        limits(),
                        &mut before,
                    )
                    .unwrap();
            } else {
                append
                    .insert_pattern_with(
                        pattern,
                        values.as_slice(),
                        TermLimits::default(),
                        limits(),
                        &mut before,
                    )
                    .unwrap();
            }
        }
        (
            work,
            (0..append.len())
                .map(|i| {
                    append
                        .get(i)
                        .unwrap()
                        .to_atom(ValueLimits::default())
                        .unwrap()
                })
                .collect(),
        )
    })
}

#[test]
fn repeated_templates_remove_repeated_validation_work() {
    let (prepared_work, prepared) = sequence(true);
    let (general_work, general) = sequence(false);
    assert_eq!(prepared, general);
    // Includes the once-only preparation cost, plus all canonical/index work.
    assert!(
        prepared_work < general_work,
        "prepared={prepared_work}, general={general_work}"
    );
}

#[test]
fn every_preparation_stop_preserves_the_owner() {
    let mut measured = Fixture::new();
    let total = measured.with_pattern(|append, pattern, _| {
        let mut work = 0;
        append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), || {
                work += 1;
                PERMIT()
            })
            .unwrap()
            .unwrap();
        work
    });
    for cut in 0..total {
        let mut fixture = Fixture::new();
        fixture.with_pattern(|append, pattern, _| {
            let bytes = append.storage_bytes();
            let peak = append.storage_peak_bytes();
            let mut work = 0;
            assert!(
                matches!(append.prepare_pattern_with(pattern, TermLimits::default(), limits(), || {
                if work == cut { return Err(cut); }
                work += 1;
                Ok(())
            }), Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cut)
            );
            assert_eq!(work, cut);
            assert_eq!(append.storage_bytes(), bytes);
            assert_eq!(append.storage_peak_bytes(), peak);
            assert!(append.is_empty());
        });
    }
}

#[test]
fn prepared_empty_patterns_authenticate_assignments() {
    let pattern = crate::AtomPattern::new(Predicate::new("empty", 0).unwrap(), vec![]).unwrap();
    let program = crate::Program::new(
        vec![crate::Template::new(
            Some(pattern),
            vec![],
            vec![],
            vec![],
            vec![],
        )],
        crate::AdmissionLimits::default(),
    )
    .unwrap();
    let mut owner = AtomInterner::for_program(&program, 1 << 22).unwrap();
    let foreign = AtomInterner::new();
    let values = foreign.read().assignment();
    let pattern = program.templates().at(0).unwrap().head().unwrap();
    let prepared = owner
        .appender()
        .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
        .unwrap()
        .unwrap();
    assert!(matches!(
        owner.appender().insert_prepared_pattern_with(
            &prepared,
            values.as_slice(),
            limits(),
            PERMIT
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::ForeignCatalog
        )))
    ));
    let local = owner.read().assignment();
    assert_eq!(
        owner
            .appender()
            .insert_prepared_pattern_with(&prepared, local.as_slice(), limits(), PERMIT)
            .unwrap(),
        0
    );
}
