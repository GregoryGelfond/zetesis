//! Candidate owner locations share lookup work without retaining tuple absence.

use super::*;
use crate::catalog::CatalogRead;

fn other(name: &str) -> Atom {
    Atom::new(
        Predicate::with_sign(name, 1, Sign::Negative).unwrap(),
        vec![Value::Number(0)],
    )
    .unwrap()
}

fn insert(append: &mut AtomAppender<'_>, atom: &Atom) -> usize {
    append
        .entry_atom_with(atom, limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), PERMIT)
        .unwrap()
}

fn crowded() -> Fixture {
    let mut fixture = Fixture::new();
    for index in 0..16 {
        insert(
            &mut fixture.owner.appender(),
            &other(&format!("a{index:02}")),
        );
    }
    fixture
}

fn assign(append: &mut AtomAppender<'_>, values: &mut TermAssignment, number: i32) {
    let key = append
        .import_term_with(
            (&Value::Number(number)).into(),
            TermLimits::default(),
            limits(),
            PERMIT,
        )
        .unwrap();
    values.set_with(1, &key, PERMIT).unwrap();
}

fn sequence(located: bool) -> (usize, Vec<usize>, Vec<Atom>) {
    let mut fixture = crowded();
    fixture.with_pattern(|append, pattern, values| {
        let mut work = 0;
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), || {
                // Give the cold arm free preparation: a saving then covers
                // even the located arm's complete once-only preparation cost.
                if located {
                    work += 1;
                }
                PERMIT()
            })
            .unwrap()
            .unwrap();
        let mut ids = Vec::new();
        for number in [11, 12, 9, 12, 13, 15, 11] {
            assign(append, values, number);
            // Both arms share identical admitted coordinates, exact row lookup,
            // and publication. Only reuse of predicate positions differs.
            let projected = Projected {
                predicate: prepared.pattern.predicate,
                values: values.as_slice().slots,
                arguments: ProjectionSource::Admitted(prepared.source, &prepared.pattern.terms),
            };
            ids.push(
                append
                    .insert_projection_at(
                        &projected,
                        located.then_some(prepared.locations),
                        limits(),
                        || {
                            work += 1;
                            PERMIT()
                        },
                    )
                    .unwrap(),
            );
        }
        let atoms = (0..append.len())
            .map(|id| {
                append
                    .get(id)
                    .unwrap()
                    .to_atom(ValueLimits::default())
                    .unwrap()
            })
            .collect();
        (work, ids, atoms)
    })
}

#[test]
fn predicate_locations_remove_repeated_lookup_work() {
    let (warm, warm_ids, warm_atoms) = sequence(true);
    let (cold, cold_ids, cold_atoms) = sequence(false);
    assert_eq!(warm_ids, cold_ids);
    assert_eq!(warm_atoms, cold_atoms);
    assert!(warm < cold, "located={warm}, ordinary={cold}");
}

#[test]
fn interleaved_signatures_preserve_prepared_identity() {
    let mut fixture = crowded();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        let first = append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
            .unwrap();
        insert(append, &other("aa_before"));
        let mut permit = PERMIT;
        let actual = super::super::super::super::subtree_of(
            append.store,
            append.subtrees,
            pattern.predicate(),
            &mut permit,
        )
        .unwrap()
        .unwrap();
        assert_ne!(
            prepared.locations.subtree, actual,
            "exercise a shifted subtree"
        );
        assign(append, values, 12);
        let second = append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
            .unwrap();
        assert_eq!(append.get(first).unwrap(), expected(11));
        assert_eq!(append.get(second).unwrap(), expected(12));
        assert_eq!(
            append
                .insert_pattern_with(
                    pattern,
                    values.as_slice(),
                    TermLimits::default(),
                    limits(),
                    PERMIT,
                )
                .unwrap(),
            second
        );
    });
}

#[test]
fn unoccupied_locations_do_not_certify_absence() {
    let mut fixture = crowded();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        // Both candidate insertion positions can now name another predicate.
        insert(append, &other("aa_intervening"));
        let mut permit = PERMIT;
        let actual_subtree = super::super::super::super::subtree_of(
            append.store,
            append.subtrees,
            pattern.predicate(),
            &mut permit,
        )
        .unwrap()
        .unwrap_err();
        let actual_column = append
            .store
            .atom_column_location_with(prepared.pattern.predicate, PERMIT)
            .unwrap();
        assert_ne!(
            prepared.locations.subtree, actual_subtree,
            "exercise stale subtree location"
        );
        assert_ne!(
            prepared.locations.column, actual_column,
            "exercise stale column location"
        );
        let ordinary = append
            .insert_pattern_with(
                pattern,
                values.as_slice(),
                TermLimits::default(),
                limits(),
                PERMIT,
            )
            .unwrap();
        let before = append.len();
        let reused = append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
            .unwrap();
        assert_eq!(reused, ordinary);
        assert_eq!(append.len(), before);
        assign(append, values, 12);
        let novel = append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
            .unwrap();
        assert_eq!(append.get(novel).unwrap(), expected(12));
    });
}

#[test]
fn committed_tails_preserve_prepared_identity() {
    let mut fixture = crowded();
    let source = fixture.owner.snapshot.clone();
    let pattern = fixture
        .components
        .bind_with(CatalogRead(storage::Read::from(&source)), PERMIT)
        .unwrap()
        .pattern(0)
        .unwrap();
    let prepared = fixture
        .owner
        .appender()
        .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
        .unwrap()
        .unwrap();
    for number in [11, 12, 13] {
        {
            let mut append = fixture.owner.appender();
            // A fresh tail puts this different signature before the head's
            // block. The retained prepared position cannot authorize its use.
            insert(&mut append, &other(&format!("z{number}")));
            let current = append
                .store
                .atom_column_location_with(prepared.pattern.predicate, PERMIT)
                .unwrap();
            assert_ne!(
                prepared.locations.column, current,
                "exercise a stale tail location"
            );
            assign(&mut append, &mut fixture.values, number);
            let id = append
                .insert_prepared_pattern_with(
                    &prepared,
                    fixture.values.as_slice(),
                    limits(),
                    PERMIT,
                )
                .unwrap();
            assert_eq!(append.get(id).unwrap(), expected(number));
            assert_eq!(
                append
                    .insert_pattern_with(
                        pattern,
                        fixture.values.as_slice(),
                        TermLimits::default(),
                        limits(),
                        PERMIT,
                    )
                    .unwrap(),
                id
            );
        }
        fixture.owner.commit_with(limits(), PERMIT).unwrap();
    }
}

fn warm_insertion(cut: Option<usize>) -> (usize, Result<usize, AssignedFailure<usize>>) {
    let mut fixture = crowded();
    fixture.with_pattern(|append, pattern, values| {
        let prepared = append
            .prepare_pattern_with(pattern, TermLimits::default(), limits(), PERMIT)
            .unwrap()
            .unwrap();
        let first = append
            .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
            .unwrap();
        let prefix = append.len();
        assign(append, values, 12);
        let mut work = 0;
        let result =
            append.insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), || {
                if cut == Some(work) {
                    return Err(work);
                }
                work += 1;
                Ok(())
            });
        if result.is_err() {
            assert_eq!(append.len(), prefix);
            assert_eq!(append.get(first).unwrap(), expected(11));
            let id = append
                .insert_prepared_pattern_with(&prepared, values.as_slice(), limits(), PERMIT)
                .unwrap();
            assert_eq!(id, prefix);
            assert_eq!(append.get(id).unwrap(), expected(12));
            assert_eq!(
                append
                    .insert_pattern_with(
                        pattern,
                        values.as_slice(),
                        TermLimits::default(),
                        limits(),
                        PERMIT,
                    )
                    .unwrap(),
                id
            );
        }
        (work, result)
    })
}

#[test]
fn every_located_insertion_stop_preserves_the_prefix() {
    let (total, completed) = warm_insertion(None);
    assert!(completed.is_ok());
    for cut in 0..total {
        let (work, stopped) = warm_insertion(Some(cut));
        assert_eq!(work, cut);
        assert!(
            matches!(stopped, Err(AssignedFailure::Interner(Failure::Stopped(at))) if at == cut)
        );
    }
    assert!(warm_insertion(Some(total)).1.is_ok());
}

#[test]
fn prepared_locations_remain_shareable() {
    fn shareable<T: Send + Sync>() {}
    shareable::<PreparedPattern<'static>>();
    shareable::<PreparedRows<'static, 'static>>();
}
