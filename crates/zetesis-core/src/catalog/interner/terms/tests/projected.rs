use super::*;
use crate::catalog::TermAssignment;

struct Fixture {
    owner: AtomInterner,
    predicate: DeclaredPredicate,
    constant: TermKey,
    values: TermAssignment,
}

fn predicate() -> Predicate {
    Predicate::with_sign("projected", 3, Sign::Negative).unwrap()
}

fn expected() -> crate::Atom {
    crate::Atom::new(
        predicate(),
        vec![
            Value::Number(11),
            Value::String("shared".into()),
            Value::Number(11),
        ],
    )
    .unwrap()
}

impl Fixture {
    fn new() -> Self {
        let mut owner = AtomInterner::new();
        let (_, mut append) = owner.split();
        let predicate = append
            .declare_predicate_with(&predicate(), limits(), SUCCESS)
            .unwrap();
        let constant = append
            .import_term_with(
                (&Value::String("shared".into())).into(),
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap();
        let variable = append
            .import_term_with(
                (&Value::Number(11)).into(),
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap();
        let mut values = append.read().assignment();
        values.resize_with(2, usize::MAX, SUCCESS).unwrap();
        values.set_with(0, &constant, SUCCESS).unwrap();
        values.set_with(1, &variable, SUCCESS).unwrap();
        owner.commit_with(limits(), SUCCESS).unwrap();
        owner.restart_storage_peak();
        Self {
            owner,
            predicate,
            constant,
            values,
        }
    }

    fn with_pattern<R>(
        &mut self,
        operation: impl FnOnce(&mut AtomAppender<'_>, PatternRef<'_>, AssignmentSlice<'_>) -> R,
    ) -> R {
        let (prefix, mut append) = self.owner.split();
        let terms = [
            TemplateTerm::Variable(1),
            TemplateTerm::Constant(prefix.read().term(&self.constant).unwrap()),
            TemplateTerm::Variable(1),
        ];
        let pattern =
            PatternRef::from_parts(prefix.read().predicate(&self.predicate).unwrap(), &terms)
                .unwrap();
        operation(&mut append, pattern, self.values.as_slice())
    }

    fn insert<E>(
        &mut self,
        term_limits: TermLimits,
        limits: Limits,
        before: impl FnMut() -> Result<(), E>,
    ) -> Result<usize, AssignedFailure<E>> {
        self.with_pattern(|append, pattern, values| {
            append.insert_pattern_with(pattern, values, term_limits, limits, before)
        })
    }
}

#[test]
fn pattern_projection_preserves_signed_argument_order() {
    let mut fixture = Fixture::new();
    let position = fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    assert_eq!(fixture.owner.get(position).unwrap(), expected());
}

#[test]
fn pattern_and_assigned_projection_share_discovery() {
    let mut fixture = Fixture::new();
    let position = fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .split()
            .1
            .insert_assigned_with(
                &fixture.predicate,
                fixture.values.as_slice(),
                &[1, 0, 1],
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap(),
        position
    );
    assert_eq!(fixture.owner.len(), 1);
}

#[test]
fn pattern_projection_reuses_ingress_identity() {
    let mut fixture = Fixture::new();
    let position = fixture
        .owner
        .split()
        .1
        .entry_atom_with(&expected(), limits(), SUCCESS)
        .unwrap()
        .insert_with(limits(), SUCCESS)
        .unwrap();
    assert_eq!(
        fixture
            .insert(TermLimits::default(), limits(), SUCCESS)
            .unwrap(),
        position
    );
    assert_eq!(fixture.owner.len(), 1);
}

#[test]
fn projected_occupied_entry_checks_logical_limits() {
    let mut fixture = Fixture::new();
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    assert!(matches!(
        fixture.insert(
            TermLimits {
                max_nodes: 0,
                ..TermLimits::default()
            },
            limits(),
            SUCCESS,
        ),
        Err(AssignedFailure::Interner(Failure::Catalog(
            crate::catalog::Error::Value(crate::ValueError::Limit {
                resource: crate::ValueResource::Nodes,
                observed: 1,
                limit: 0,
            })
        )))
    ));
    assert_eq!(fixture.owner.len(), 1);
}

fn foreign<E>(error: &AssignedFailure<E>) -> bool {
    matches!(
        error,
        AssignedFailure::Assignment(AssignmentError::Read(ReadError::ForeignCatalog))
    )
}

#[test]
fn projected_pattern_rejects_foreign_predicate() {
    let mut fixture = Fixture::new();
    let other = Fixture::new();
    let terms = [TemplateTerm::Variable(1); 3];
    let pattern = PatternRef::from_parts(
        other.owner.read().predicate(&other.predicate).unwrap(),
        &terms,
    )
    .unwrap();
    assert!(foreign(
        &fixture
            .owner
            .split()
            .1
            .insert_pattern_with(
                pattern,
                fixture.values.as_slice(),
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap_err()
    ));
    assert!(fixture.owner.is_empty());
}

#[test]
fn projected_pattern_rejects_foreign_constant() {
    let mut fixture = Fixture::new();
    let other = Fixture::new();
    fixture.with_pattern(|append, local, values| {
        let terms = [
            TemplateTerm::Variable(1),
            TemplateTerm::Constant(other.owner.read().term(&other.constant).unwrap()),
            TemplateTerm::Variable(1),
        ];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(foreign(
            &append
                .insert_pattern_with(pattern, values, TermLimits::default(), limits(), SUCCESS,)
                .unwrap_err()
        ));
    });
    assert!(fixture.owner.is_empty());
}

#[test]
fn constant_projection_checks_empty_frame_scope() {
    let mut fixture = Fixture::new();
    let other = AtomInterner::new();
    let empty = other.read().assignment();
    fixture.with_pattern(|append, local, _| {
        let constant = local.terms().at(1).unwrap();
        let terms = [constant; 3];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(foreign(
            &append
                .insert_pattern_with(
                    pattern,
                    empty.as_slice(),
                    TermLimits::default(),
                    limits(),
                    SUCCESS,
                )
                .unwrap_err()
        ));
    });
    assert!(fixture.owner.is_empty());
}

#[test]
fn projected_pattern_refuses_uninterned_constant() {
    let mut fixture = Fixture::new();
    let value = Value::String("shared".into());
    fixture.with_pattern(|append, local, values| {
        let terms = [TemplateTerm::Constant((&value).into()); 3];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(matches!(
            append.insert_pattern_with(pattern, values, TermLimits::default(), limits(), SUCCESS,),
            Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::Uninterned
            )))
        ));
    });
}

#[test]
fn projected_pattern_refuses_unbound_variable() {
    let mut fixture = Fixture::new();
    fixture.values.clear_with(1, SUCCESS).unwrap();
    assert!(matches!(
        fixture.insert(TermLimits::default(), limits(), SUCCESS),
        Err(AssignedFailure::Assignment(AssignmentError::Unbound {
            slot: 1
        }))
    ));
    assert!(fixture.owner.is_empty());
}

#[test]
fn projected_argument_refuses_inaccessible_tail() {
    let mut fixture = Fixture::new();
    let (prefix, mut append) = fixture.owner.split();
    let later = append
        .import_term_with(
            (&Value::Number(42)).into(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    fixture.values.set_with(1, &later, SUCCESS).unwrap();
    let mut before = SUCCESS;
    assert!(matches!(
        projected_argument(
            prefix.read().0,
            fixture.values.as_slice(),
            TemplateTerm::Variable(1),
            &mut before,
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
}

fn work() -> usize {
    let mut fixture = Fixture::new();
    let mut calls = 0;
    fixture
        .insert(TermLimits::default(), limits(), || {
            calls += 1;
            SUCCESS()
        })
        .unwrap();
    calls
}

#[test]
fn every_projection_work_stop_withholds_discovery() {
    for cut in 0..work() {
        let mut fixture = Fixture::new();
        let mut visited = 0;
        let result = fixture.insert(TermLimits::default(), limits(), || {
            if visited == cut {
                return Err(cut);
            }
            visited += 1;
            Ok(())
        });
        assert!(matches!(result,
            Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cut));
        assert_eq!(visited, cut);
        assert!(fixture.owner.is_empty(), "cut {cut}");
        assert_eq!(
            fixture
                .insert(TermLimits::default(), limits(), SUCCESS)
                .unwrap(),
            0
        );
        assert_eq!(fixture.owner.get(0).unwrap(), expected());
    }
}

#[test]
fn projection_retry_reuses_previously_admitted_row() {
    let mut saw_canonical_row = false;
    for cut in 0..work() {
        let mut fixture = Fixture::new();
        let mut visited = 0;
        let stopped = fixture.insert(TermLimits::default(), limits(), || {
            if visited == cut {
                return Err(());
            }
            visited += 1;
            Ok(())
        });
        assert!(stopped.is_err());
        fixture.owner.commit_with(limits(), SUCCESS).unwrap();
        if fixture.owner.snapshot.atom_count() == 0 {
            continue;
        }
        saw_canonical_row = true;
        assert!(fixture.owner.is_empty());
        fixture
            .insert(TermLimits::default(), limits(), SUCCESS)
            .unwrap();
        fixture.owner.commit_with(limits(), SUCCESS).unwrap();
        assert_eq!(fixture.owner.snapshot.atom_count(), 1);
        assert_eq!(fixture.owner.len(), 1);
    }
    assert!(
        saw_canonical_row,
        "must exercise a stop after canonical admission"
    );
}

#[test]
fn projection_unwind_preserves_actual_peak_receipt() {
    for cut in 0..work() {
        let mut refused = Fixture::new();
        let mut visited = 0;
        let _ = refused.insert(TermLimits::default(), limits(), || {
            if visited == cut {
                return Err(());
            }
            visited += 1;
            Ok(())
        });
        let expected_peak = refused.owner.storage_peak_bytes();
        let mut unwound = Fixture::new();
        let mut visited = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = unwound.insert(TermLimits::default(), limits(), || {
                assert_ne!(visited, cut, "injected work panic");
                visited += 1;
                SUCCESS()
            });
        }));
        assert!(panic.is_err());
        assert!(unwound.owner.is_empty());
        assert_eq!(
            unwound.owner.storage_peak_bytes(),
            expected_peak,
            "cut {cut}"
        );
        assert_eq!(
            unwound
                .insert(TermLimits::default(), limits(), SUCCESS)
                .unwrap(),
            0
        );
    }
}

#[test]
fn occupied_projection_counts_live_scratch() {
    let mut fixture = Fixture::new();
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    fixture.owner.restart_storage_peak();
    let retained = fixture.owner.storage_bytes();
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let scratch = size_of::<Projected<'_>>() as u128 + super::super::super::PREPARED_BYTES;
    assert_eq!(fixture.owner.storage_bytes(), retained);
    assert_eq!(fixture.owner.storage_peak_bytes(), retained + scratch);
}

#[test]
fn projected_admission_accepts_exact_combined_peak() {
    let mut measured = Fixture::new();
    measured
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let peak = measured.owner.storage_peak_bytes();
    let mut exact = Fixture::new();
    assert_eq!(
        exact
            .insert(
                TermLimits::default(),
                Limits {
                    max_bytes: peak,
                    ..limits()
                },
                SUCCESS
            )
            .unwrap(),
        0
    );
    assert_eq!(exact.owner.storage_peak_bytes(), peak);
}

#[test]
fn projected_admission_refuses_below_combined_peak() {
    let mut measured = Fixture::new();
    measured
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    let peak = measured.owner.storage_peak_bytes();
    let mut bounded = Fixture::new();
    assert!(matches!(bounded.insert(
        TermLimits::default(), Limits { max_bytes: peak - 1, ..limits() }, SUCCESS,
    ), Err(AssignedFailure::Interner(Failure::Bytes { required, limit }))
        if required > limit && limit == peak - 1));
    assert!(bounded.owner.is_empty());
    assert_eq!(
        bounded
            .insert(TermLimits::default(), limits(), SUCCESS)
            .unwrap(),
        0
    );
}

fn admitted_program() -> (crate::Program, crate::Atom) {
    let nested = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 2,
            },
            ValueNode::String("shared".into()),
            ValueNode::Number(11),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let expected =
        crate::Atom::new(predicate(), vec![nested.clone(), Value::Number(11), nested]).unwrap();
    let pattern = crate::AtomPattern::new(
        predicate(),
        expected
            .values()
            .iter()
            .cloned()
            .map(crate::Term::Constant)
            .collect(),
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
    (program, expected)
}

#[test]
fn admitted_pattern_publishes_its_borrowed_constants() {
    let (program, expected) = admitted_program();
    let mut owner = AtomInterner::for_program(&program, 1_048_576).unwrap();
    let values = owner.read().assignment();
    let pattern = program.templates().at(0).unwrap().head().unwrap();
    // This producer is PatternTerms::Admitted, rather than from_parts metadata.
    let position = owner
        .split()
        .1
        .insert_pattern_with(
            pattern,
            values.as_slice(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    assert_eq!(position, 0);
    assert_eq!(owner.get(position).unwrap(), expected);
    let TemplateTerm::Constant(original) = pattern.terms().at(0).unwrap() else {
        panic!("admitted closed constant");
    };
    assert!(original.same_identity(owner.get(position).unwrap().values().at(0).unwrap()));
}

#[test]
fn occupied_projection_checks_nested_bounds() {
    let (program, _) = admitted_program();
    let mut owner = AtomInterner::for_program(&program, 1_048_576).unwrap();
    let values = owner.read().assignment();
    let pattern = program.templates().at(0).unwrap().head().unwrap();
    owner
        .split()
        .1
        .insert_pattern_with(
            pattern,
            values.as_slice(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    for (term_limits, expected) in [
        (
            TermLimits {
                max_nodes: 2,
                ..TermLimits::default()
            },
            crate::ValueResource::Nodes,
        ),
        (
            TermLimits {
                max_depth: 1,
                ..TermLimits::default()
            },
            crate::ValueResource::Depth,
        ),
        (
            TermLimits {
                max_bytes: 0,
                ..TermLimits::default()
            },
            crate::ValueResource::Bytes,
        ),
    ] {
        assert!(matches!(owner.split().1.insert_pattern_with(
            pattern, values.as_slice(), term_limits, limits(), SUCCESS,
        ), Err(AssignedFailure::Interner(Failure::Catalog(crate::catalog::Error::Value(
            crate::ValueError::Limit { resource, .. }
        )))) if resource == expected));
        assert_eq!(owner.len(), 1);
    }
}

fn before_frame<E>(
    fixture: &mut Fixture,
    before: impl FnMut() -> Result<(), E>,
) -> Result<usize, AssignedFailure<E>> {
    fixture.with_pattern(|append, pattern, values| {
        let (_, predicate) = pattern.predicate().canonical().unwrap();
        append.insert_projected_with(
            predicate,
            values,
            ProjectionSource::Pattern(pattern.terms()),
            TermLimits::default(),
            limits(),
            before,
        )
    })
}

fn projection_bytes() -> u128 {
    size_of::<Projected<'_>>() as u128 + super::super::super::PREPARED_BYTES
}

#[test]
fn validation_stop_keeps_the_live_projection_receipt() {
    let mut fixture = Fixture::new();
    let retained = fixture.owner.storage_bytes();
    assert!(matches!(
        before_frame(&mut fixture, || Err("before frame")),
        Err(AssignedFailure::Interner(Failure::Stopped("before frame")))
    ));
    assert_eq!(
        fixture.owner.storage_peak_bytes(),
        retained + projection_bytes()
    );
    assert_eq!(fixture.owner.storage_bytes(), retained);
    assert!(fixture.owner.is_empty());
}

#[test]
fn validation_unwind_keeps_the_live_projection_receipt() {
    let mut fixture = Fixture::new();
    let retained = fixture.owner.storage_bytes();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = before_frame(&mut fixture, || -> Result<(), Infallible> {
            panic!("before frame")
        });
    }));
    assert!(panic.is_err());
    assert_eq!(
        fixture.owner.storage_peak_bytes(),
        retained + projection_bytes()
    );
    assert_eq!(fixture.owner.storage_bytes(), retained);
    assert!(fixture.owner.is_empty());
}

#[test]
fn occupied_projection_scratch_is_independent_of_arity() {
    for arity in [0, 1, 65, 257] {
        let mut owner = AtomInterner::new();
        let (_, mut append) = owner.split();
        let predicate = append
            .declare_predicate_with(&Predicate::new("wide", arity).unwrap(), limits(), SUCCESS)
            .unwrap();
        let term = append
            .import_term_with(
                (&Value::Number(11)).into(),
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap();
        let mut values = append.read().assignment();
        values.resize_with(1, usize::MAX, SUCCESS).unwrap();
        values.set_with(0, &term, SUCCESS).unwrap();
        let arguments = vec![0; arity];
        let first = append
            .insert_assigned_with(
                &predicate,
                values.as_slice(),
                &arguments,
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap();
        append.restart_storage_peak();
        let retained = append.storage_bytes();
        let exact = Limits {
            max_bytes: retained + projection_bytes(),
            ..limits()
        };
        assert_eq!(
            append
                .insert_assigned_with(
                    &predicate,
                    values.as_slice(),
                    &arguments,
                    TermLimits::default(),
                    exact,
                    SUCCESS,
                )
                .unwrap(),
            first
        );
        assert_eq!(append.storage_bytes(), retained);
        assert_eq!(append.storage_peak_bytes(), exact.max_bytes);
        let row = append.get(first).unwrap();
        assert_eq!(row.values().len(), arity);
        assert!(row.values().iter().all(|value| value == Value::Number(11)));
    }
}

#[test]
fn occupied_projection_refuses_missing_scratch_byte() {
    let mut fixture = Fixture::new();
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    fixture.owner.restart_storage_peak();
    let required = fixture.owner.storage_bytes() + projection_bytes();
    let bounded = Limits {
        max_bytes: required - 1,
        ..limits()
    };
    assert!(
        matches!(fixture.insert(TermLimits::default(), bounded, SUCCESS),
        Err(AssignedFailure::Interner(Failure::Bytes { required: actual, limit }))
            if actual == required && limit == required - 1)
    );
    assert_eq!(fixture.owner.len(), 1);
}

mod lookup;
