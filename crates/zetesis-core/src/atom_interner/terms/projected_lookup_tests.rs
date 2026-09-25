use super::*;

fn lookup<E>(
    fixture: &mut Fixture,
    limits: Limits,
    before: impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, AssignedFailure<E>> {
    fixture.with_pattern(|append, pattern, values| {
        append.find_pattern_with(pattern, values, limits, before)
    })
}

fn occupied() -> Fixture {
    let mut fixture = Fixture::new();
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    fixture.owner.restart_storage_peak();
    fixture
}

#[test]
fn projected_lookup_agrees_with_complete_key() {
    let mut fixture = occupied();
    fixture.with_pattern(|append, pattern, values| {
        let view = values.bind_with(append.read(), SUCCESS).unwrap();
        let key = pattern.key(view).unwrap();
        assert_eq!(
            append
                .find_pattern_with(pattern, values, limits(), SUCCESS)
                .unwrap(),
            append.find_key_with(key, limits(), SUCCESS).unwrap()
        );
    });
    assert_eq!(lookup(&mut fixture, limits(), SUCCESS).unwrap(), Some(0));
    fixture
        .values
        .set_with(1, &fixture.constant, SUCCESS)
        .unwrap();
    fixture.with_pattern(|append, pattern, values| {
        let key = pattern
            .key(values.bind_with(append.read(), SUCCESS).unwrap())
            .unwrap();
        assert_eq!(append.find_key_with(key, limits(), SUCCESS).unwrap(), None);
        assert_eq!(
            append
                .find_pattern_with(pattern, values, limits(), SUCCESS)
                .unwrap(),
            None
        );
    });
}

#[test]
fn projected_lookup_work_ignores_unselected_width() {
    let mut totals = Vec::new();
    for width in [2, 65, 1025] {
        let mut fixture = occupied();
        fixture
            .values
            .resize_with(width, usize::MAX, SUCCESS)
            .unwrap();
        let mut calls = 0;
        assert_eq!(
            lookup(&mut fixture, limits(), || {
                calls += 1;
                SUCCESS()
            })
            .unwrap(),
            Some(0)
        );
        totals.push(calls);
    }
    assert!(totals[0] > 0);
    assert!(totals.iter().all(|&total| total == totals[0]));
}

#[test]
fn selected_authentication_ignores_a_newer_unused_cell() {
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
    fixture.values.set_with(0, &later, SUCCESS).unwrap();
    let values = fixture.values.as_slice();
    assert!(matches!(
        values.bind_with(prefix.read(), SUCCESS),
        Err(crate::catalog::AssignmentFailure::Assignment(
            AssignmentError::Read(ReadError::OutsidePrefix)
        ))
    ));
    let mut before = SUCCESS;
    projected_scope(prefix.read().0, values, &mut before).unwrap();
    projected_argument(
        prefix.read().0,
        values,
        TemplateTerm::Variable(1),
        &mut before,
    )
    .unwrap();
    assert!(matches!(
        projected_argument(
            prefix.read().0,
            values,
            TemplateTerm::Variable(0),
            &mut before
        ),
        Err(AssignedFailure::Assignment(AssignmentError::Read(
            ReadError::OutsidePrefix
        )))
    ));
}

#[test]
fn complete_lookup_authenticates_empty_frame_scope() {
    for arity in [0, 3] {
        let mut owner = AtomInterner::new();
        let (_, mut append) = owner.split();
        let predicate = append
            .declare_predicate_with(&Predicate::new("empty", arity).unwrap(), limits(), SUCCESS)
            .unwrap();
        let constant = append
            .import_term_with(
                (&Value::Number(1)).into(),
                TermLimits::default(),
                limits(),
                SUCCESS,
            )
            .unwrap();
        owner.commit_with(limits(), SUCCESS).unwrap();
        let other = AtomInterner::new();
        let foreign = other.read().assignment();
        let (prefix, append) = owner.split();
        let terms = vec![TemplateTerm::Constant(prefix.read().term(&constant).unwrap()); arity];
        let pattern =
            PatternRef::from_parts(prefix.read().predicate(&predicate).unwrap(), &terms).unwrap();
        assert!(super::foreign(
            &append
                .find_pattern_with(pattern, foreign.as_slice(), limits(), SUCCESS,)
                .unwrap_err()
        ));
    }
}

#[test]
fn projected_lookup_refuses_foreign_predicate() {
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
            .find_pattern_with(pattern, fixture.values.as_slice(), limits(), SUCCESS,)
            .unwrap_err()
    ));
}

#[test]
fn projected_lookup_refuses_foreign_constant() {
    let mut fixture = Fixture::new();
    let other = Fixture::new();
    fixture.with_pattern(|append, local, values| {
        let terms = [TemplateTerm::Constant(other.owner.read().term(&other.constant).unwrap()); 3];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(foreign(
            &append
                .find_pattern_with(pattern, values, limits(), SUCCESS)
                .unwrap_err()
        ));
    });
}

#[test]
fn projected_lookup_refuses_uninterned_constant() {
    let mut fixture = Fixture::new();
    let value = Value::Number(11);
    fixture.with_pattern(|append, local, values| {
        let terms = [TemplateTerm::Constant((&value).into()); 3];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(matches!(
            append.find_pattern_with(pattern, values, limits(), SUCCESS),
            Err(AssignedFailure::Assignment(AssignmentError::Read(
                ReadError::Uninterned
            )))
        ));
    });
}

#[test]
fn projected_lookup_refuses_invalid_selected_slots() {
    let mut fixture = Fixture::new();
    fixture.values.clear_with(1, SUCCESS).unwrap();
    assert!(matches!(
        lookup(&mut fixture, limits(), SUCCESS),
        Err(AssignedFailure::Assignment(AssignmentError::Unbound {
            slot: 1
        }))
    ));
    fixture.values.truncate_with(1, SUCCESS).unwrap();
    assert!(matches!(
        lookup(&mut fixture, limits(), SUCCESS),
        Err(AssignedFailure::Assignment(AssignmentError::Slot {
            slot: 1,
            len: 1
        }))
    ));
}

#[test]
fn insertion_preserves_argument_error_order() {
    let mut fixture = Fixture::new();
    let other = Fixture::new();
    fixture.with_pattern(|append, local, values| {
        let terms = [
            TemplateTerm::Variable(1),
            TemplateTerm::Constant(other.owner.read().term(&other.constant).unwrap()),
            TemplateTerm::Variable(1),
        ];
        let pattern = PatternRef::from_parts(local.predicate(), &terms).unwrap();
        assert!(matches!(
            append.insert_pattern_with(
                pattern,
                values,
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
                    limit: 0
                })
            )))
        ));
        // Lookup authenticates coordinates but does not add insertion's logical ceiling.
        assert!(foreign(
            &append
                .find_pattern_with(pattern, values, limits(), SUCCESS)
                .unwrap_err()
        ));
    });
}

#[test]
fn projected_lookup_requires_discovery_publication() {
    let mut fixture = Fixture::new();
    fixture
        .owner
        .store
        .import_atom_with((&expected()).into(), TermLimits::default(), SUCCESS)
        .unwrap();
    assert!(fixture.owner.is_empty());
    assert_eq!(lookup(&mut fixture, limits(), SUCCESS).unwrap(), None);
    fixture
        .insert(TermLimits::default(), limits(), SUCCESS)
        .unwrap();
    assert_eq!(lookup(&mut fixture, limits(), SUCCESS).unwrap(), Some(0));
}

#[test]
fn projected_lookup_reads_admitted_template_constants() {
    let (program, expected) = admitted_program();
    let mut owner = AtomInterner::for_program(&program, 1_048_576).unwrap();
    let values = owner.read().assignment();
    let pattern = program.templates().at(0).unwrap().head().unwrap();
    let (_, mut append) = owner.split();
    append
        .insert_pattern_with(
            pattern,
            values.as_slice(),
            TermLimits::default(),
            limits(),
            SUCCESS,
        )
        .unwrap();
    assert_eq!(
        append
            .find_pattern_with(pattern, values.as_slice(), limits(), SUCCESS)
            .unwrap(),
        Some(0)
    );
    assert_eq!(append.get(0).unwrap(), expected);
}

#[test]
fn projected_lookup_admits_its_exact_header() {
    let mut fixture = occupied();
    let bytes = fixture.owner.split().1.pattern_lookup_bytes();
    let retained = fixture.owner.storage_bytes();
    assert_eq!(bytes, retained + Projected::HEADER_BYTES);
    assert_eq!(
        lookup(
            &mut fixture,
            Limits {
                max_bytes: bytes,
                ..limits()
            },
            SUCCESS
        )
        .unwrap(),
        Some(0)
    );
    assert_eq!(fixture.owner.storage_bytes(), retained);
    assert_eq!(fixture.owner.storage_peak_bytes(), retained);
    let mut calls = 0;
    assert!(
        matches!(lookup(&mut fixture, Limits { max_bytes: bytes - 1, ..limits() }, || {
        calls += 1;
        SUCCESS()
    }), Err(AssignedFailure::Interner(Failure::Bytes { required, limit }))
        if required == bytes && limit == bytes - 1)
    );
    assert_eq!(calls, 0);
}

#[test]
fn lookup_population_precedes_header_admission() {
    let mut fixture = occupied();
    assert!(matches!(
        lookup(
            &mut fixture,
            Limits {
                max_atoms: 0,
                max_bytes: 0
            },
            SUCCESS
        ),
        Err(AssignedFailure::Interner(Failure::Atoms {
            required: 1,
            limit: 0
        }))
    ));
}

fn lookup_work(fixture: &mut Fixture) -> usize {
    let mut total = 0;
    lookup(fixture, limits(), || {
        total += 1;
        SUCCESS()
    })
    .unwrap();
    total
}

#[test]
fn lookup_refusal_preserves_the_owner() {
    for present in [false, true] {
        let mut fixture = if present { occupied() } else { Fixture::new() };
        let total = lookup_work(&mut fixture);
        let state = (
            fixture.owner.len(),
            fixture.owner.storage_bytes(),
            fixture.owner.storage_peak_bytes(),
        );
        for cut in 0..total {
            let mut visited = 0;
            let result = lookup(&mut fixture, limits(), || {
                if visited == cut {
                    return Err(cut);
                }
                visited += 1;
                Ok(())
            });
            assert!(
                matches!(result, Err(AssignedFailure::Interner(Failure::Stopped(actual))) if actual == cut)
            );
            assert_eq!(visited, cut);
            assert_eq!(
                (
                    fixture.owner.len(),
                    fixture.owner.storage_bytes(),
                    fixture.owner.storage_peak_bytes()
                ),
                state
            );
            assert_eq!(
                lookup(&mut fixture, limits(), SUCCESS).unwrap(),
                present.then_some(0)
            );
        }
    }
}

#[test]
fn lookup_unwind_preserves_the_owner() {
    let mut fixture = occupied();
    let total = lookup_work(&mut fixture);
    let state = (
        fixture.owner.len(),
        fixture.owner.storage_bytes(),
        fixture.owner.storage_peak_bytes(),
    );
    for cut in 0..total {
        let mut visited = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = lookup(&mut fixture, limits(), || {
                assert_ne!(visited, cut, "injected lookup unwind");
                visited += 1;
                SUCCESS()
            });
        }));
        assert!(panic.is_err());
        assert_eq!(
            (
                fixture.owner.len(),
                fixture.owner.storage_bytes(),
                fixture.owner.storage_peak_bytes()
            ),
            state
        );
        assert_eq!(lookup(&mut fixture, limits(), SUCCESS).unwrap(), Some(0));
    }
}
