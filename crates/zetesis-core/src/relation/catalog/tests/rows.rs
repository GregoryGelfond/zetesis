//! Row identity by the hash of a row's equality IDs.

use super::*;

/// Every row is found by its own tuple, and the prepared runs, when the
/// extent is prepared, hold every row once in canonical order.
fn validate(fixture: &Fixture) {
    let atoms = fixture.source();
    for row in 0..fixture.rows.len() {
        let atom = atoms.at(row).unwrap();
        let found = fixture
            .rows
            .lookup(fixture.authority.read(), atom, Limits::default())
            .unwrap();
        assert_eq!(found.row, Some(row));
    }
    if let Some(runs) = fixture.rows.ordered() {
        let mut seen = vec![false; fixture.rows.len()];
        for run in runs.runs() {
            for pair in run.windows(2) {
                assert!(
                    atoms
                        .at(pair[0])
                        .unwrap()
                        .compare_ref_with(atoms.at(pair[1]).unwrap(), || Ok::<_, ()>(()))
                        .unwrap()
                        .is_lt()
                );
            }
            for &row in run {
                assert!(!std::mem::replace(&mut seen[row], true));
            }
        }
        assert!(seen.into_iter().all(|seen| seen));
    }
    dictionary::assert_translation(fixture);
}

fn prefix(count: i32) -> Fixture {
    let mut fixture = owner();
    for value in 0..count {
        fixture.insert(&atom(0, value), Limits::default()).unwrap();
    }
    fixture
}

fn columns(fixture: &Fixture) -> Vec<Vec<u32>> {
    fixture
        .view()
        .columns()
        .map(|column| column.iter().collect())
        .collect()
}

fn session_insert(
    fixture: &mut Fixture,
    input: &Atom,
    limits: Limits,
) -> Result<Insertion, CatalogFailure> {
    let canonical = fixture
        .authority
        .entry_atom_with(input, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_ref_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let mut append = fixture.rows.appender(limits)?;
    append
        .insert(canonical, limits)
        .map(|appended| appended.insertion)
}

#[test]
fn rows_in_any_order_are_found_once() {
    let mut scalar = owner();
    let mut session = owner();
    for value in [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 3, 19, 9, 40, 72, 72, 80, 0,
    ] {
        let input = atom(0, value);
        let expected = scalar.insert(&input, Limits::default()).unwrap();
        let actual = session_insert(&mut session, &input, Limits::default()).unwrap();
        assert_eq!(
            (actual.row, actual.inserted),
            (expected.row, expected.inserted)
        );
        assert_eq!(
            (actual.storage.construction_work, expected.inserted),
            (expected.storage.construction_work, actual.inserted)
        );
        validate(&scalar);
        validate(&session);
        assert_eq!(columns(&scalar), columns(&session));
    }
    assert_eq!(scalar.rows.len(), 14);
    scalar.prepare_ordered(Limits::default()).unwrap();
    validate(&scalar);
}

#[test]
fn insertion_work_does_not_grow_with_the_extent() {
    // Placing a row compares no other row: inserting the same new tuple, and
    // finding the same old one, costs the same beside 31 rows as beside 1023.
    let work = |count: i32, value: i32| {
        let mut fixture = prefix(count);
        fixture
            .insert(&atom(0, value), Limits::default())
            .unwrap()
            .storage
            .construction_work
    };
    assert_eq!(work(31, -1), work(1023, -1));
    assert_eq!(work(31, 7), work(1023, 7));
    assert!(work(31, 7) < work(31, -1));
}

#[test]
fn every_work_refusal_preserves_the_prefix() {
    let mut complete = prefix(30);
    let input = atom(0, 30);
    let required = session_insert(&mut complete, &input, Limits::default())
        .unwrap()
        .storage
        .construction_work;
    let expected = columns(&complete);
    for max_work in 0..u64::try_from(required).unwrap() {
        let mut fixture = prefix(30);
        let previous = columns(&fixture);
        let failure = session_insert(
            &mut fixture,
            &input,
            Limits {
                max_work,
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            failure.error,
            Failure::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert_eq!(fixture.rows.len(), 30);
        assert_eq!(columns(&fixture), previous);
        assert_eq!(failure.retained_bytes, fixture.retained_bytes());
        assert!(failure.peak_construction_bytes >= failure.retained_bytes);
        validate(&fixture);
        assert_eq!(
            session_insert(&mut fixture, &input, Limits::default())
                .unwrap()
                .row,
            30
        );
        assert_eq!(columns(&fixture), expected);
        validate(&fixture);
    }
}

#[test]
fn insertion_admits_its_actual_peak() {
    let input = atom(0, 30);
    let mut complete = prefix(30);
    let exact = complete
        .insert(&input, Limits::default())
        .unwrap()
        .storage
        .peak_construction_bytes;
    let mut accepted = prefix(30);
    accepted
        .insert(
            &input,
            Limits {
                max_bytes: exact,
                ..Limits::default()
            },
        )
        .unwrap();
    let mut refused = prefix(30);
    let failure = refused
        .insert(
            &input,
            Limits {
                max_bytes: exact - 1,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Limit {
            resource: Resource::Bytes,
            ..
        }
    ));
    assert_eq!(refused.rows.len(), 30);
    assert_eq!(failure.retained_bytes, refused.retained_bytes());
    validate(&refused);
    refused.insert(&input, Limits::default()).unwrap();
    assert_eq!(columns(&refused), columns(&accepted));
    validate(&refused);
}

#[test]
fn read_only_access_changes_no_row() {
    let mut fixture = prefix(31);
    let bytes = fixture.retained_bytes();
    fixture.lookup(&atom(0, 12), Limits::default()).unwrap();
    fixture.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&fixture), (0..31).collect::<Vec<_>>());
    assert!(
        !fixture
            .insert(&atom(0, 30), Limits::default())
            .unwrap()
            .inserted
    );
    assert!(fixture.retained_bytes() >= bytes);
    validate(&fixture);
}

#[test]
fn foreign_rows_are_refused_without_change() {
    let mut fixture = prefix(31);
    let foreign = prefix(32);
    let failure = fixture
        .rows
        .insert(foreign.authority.get(31).unwrap(), Limits::default())
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Read(ReadError::ForeignCatalog)
    ));
    validate(&fixture);
    assert_eq!(fixture.rows.len(), 31);
}

#[test]
fn clear_forgets_every_row() {
    let mut fixture = prefix(31);
    fixture.clear(Limits::default()).unwrap();
    assert_eq!(
        fixture.lookup(&atom(0, 5), Limits::default()).unwrap().row,
        None
    );
    assert_eq!(
        fixture.insert(&atom(0, 5), Limits::default()).unwrap().row,
        0
    );
    assert_eq!(fixture.rows.len(), 1);
    validate(&fixture);
}
