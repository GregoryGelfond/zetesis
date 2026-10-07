use crate::atom_interner::{AtomInterner, Limits as AtomLimits};
use crate::relation::{Catalog, Failure, Limits, Resource};
use crate::{Atom, Predicate, Value};

fn atom(values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new("tuple", values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn owner(atoms: &[Atom]) -> AtomInterner {
    let mut owner = AtomInterner::default();
    let limits = AtomLimits::for_atoms(65_536, 64 * 1024 * 1024);
    for atom in atoms {
        owner
            .entry_atom_with(atom, limits, || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(limits, || Ok::<_, ()>(()))
            .unwrap();
    }
    owner
}

fn catalog(owner: &AtomInterner) -> Catalog {
    let read = owner.read();
    Catalog::new(
        read,
        read.declare_existing(owner.get(0).unwrap().predicate())
            .unwrap(),
        Limits::default(),
    )
    .unwrap()
}

fn columns(catalog: &Catalog, owner: &AtomInterner) -> Vec<Vec<u32>> {
    catalog
        .view(owner.read())
        .unwrap()
        .columns()
        .map(|column| column.iter().collect())
        .collect()
}

#[test]
fn session_rows_match_scalar_insertion() {
    let owner = owner(&[
        atom(&[9, 3]),
        atom(&[1, 8]),
        atom(&[8, 1]),
        atom(&[3, 3]),
        atom(&[0, 9]),
    ]);
    let mut scalar = catalog(&owner);
    let mut session = catalog(&owner);
    {
        let mut append = session.appender(Limits::default()).unwrap();
        for id in [0, 1, 0, 2, 3, 1, 4] {
            let canonical = owner.get(id).unwrap();
            let expected = scalar.insert(canonical, Limits::default()).unwrap();
            let actual = append.insert(canonical, Limits::default()).unwrap();
            assert_eq!(actual.insertion.row, expected.row);
            assert_eq!(actual.insertion.inserted, expected.inserted);
            if expected.inserted {
                let ids: Vec<_> = scalar
                    .view(owner.read())
                    .unwrap()
                    .columns()
                    .map(|column| column.get(expected.row).unwrap())
                    .collect();
                assert_eq!(actual.equality_ids.unwrap(), ids);
            } else {
                assert!(actual.equality_ids.is_none());
            }
        }
    }
    assert_eq!(columns(&session, &owner), columns(&scalar, &owner));
    assert_eq!(
        session
            .atoms(owner.read())
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        scalar
            .atoms(owner.read())
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
    );
}

#[test]
fn nullary_publication_lends_an_empty_row() {
    let owner = owner(&[atom(&[])]);
    let mut catalog = catalog(&owner);
    let mut append = catalog.appender(Limits::default()).unwrap();
    let first = append
        .insert(owner.get(0).unwrap(), Limits::default())
        .unwrap();
    assert_eq!(first.equality_ids, Some(&[][..]));
    let again = append
        .insert(owner.get(0).unwrap(), Limits::default())
        .unwrap();
    assert_eq!(again.insertion.row, 0);
    assert_eq!(again.equality_ids, None);
}

#[test]
fn refused_row_preserves_the_successful_prefix() {
    let owner = owner(&[atom(&[4, 4]), atom(&[1, 9])]);
    let mut reference = catalog(&owner);
    let required = {
        let mut append = reference.appender(Limits::default()).unwrap();
        append
            .insert(owner.get(0).unwrap(), Limits::default())
            .unwrap();
        append
            .insert(owner.get(1).unwrap(), Limits::default())
            .unwrap()
            .insertion
            .storage
            .construction_work
    };
    for max_work in 0..u64::try_from(required).unwrap() {
        let mut catalog = catalog(&owner);
        let mut append = catalog.appender(Limits::default()).unwrap();
        append
            .insert(owner.get(0).unwrap(), Limits::default())
            .unwrap();
        let before = columns(append.catalog, &owner);
        let error = append
            .insert(
                owner.get(1).unwrap(),
                Limits {
                    max_work,
                    ..Limits::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error.error,
            Failure::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert_eq!(append.catalog.len(), 1);
        assert_eq!(columns(append.catalog, &owner), before);
        assert_eq!(append.catalog.layout.dictionary.len(), 1);
        assert!(append.plan.ids.is_empty());
        assert!(append.plan.added.is_empty());
        assert!(append.plan.patches.is_empty());
        assert_eq!(error.retained_bytes, append.catalog.retained_bytes());
        let retry = append
            .insert(owner.get(1).unwrap(), Limits::default())
            .unwrap();
        assert!(retry.insertion.inserted);
        assert_eq!(retry.insertion.row, 1);
        assert_eq!(columns(append.catalog, &owner), columns(&reference, &owner));
    }
}

#[test]
fn reused_plan_keeps_its_admitted_allocations() {
    let owner = owner(&[atom(&[1, 2]), atom(&[2, 1])]);
    let mut catalog = catalog(&owner);
    let mut append = catalog.appender(Limits::default()).unwrap();
    append
        .insert(owner.get(0).unwrap(), Limits::default())
        .unwrap();
    let ids = append.plan.ids.as_ptr();
    let added = append.plan.added.as_ptr();
    let patches = append.plan.patches.as_ptr();
    let retained = append.scratch_bytes();
    append
        .insert(owner.get(1).unwrap(), Limits::default())
        .unwrap();
    assert_eq!(append.plan.ids.as_ptr(), ids);
    assert_eq!(append.plan.added.as_ptr(), added);
    assert_eq!(append.plan.patches.as_ptr(), patches);
    assert_eq!(append.scratch_bytes(), retained);
}

#[test]
fn retained_scratch_is_admitted_on_duplicate_rows() {
    let owner = owner(&[atom(&[1, 2])]);
    let mut catalog = catalog(&owner);
    let mut append = catalog.appender(Limits::default()).unwrap();
    append
        .insert(owner.get(0).unwrap(), Limits::default())
        .unwrap();
    let scratch = append.scratch_bytes();
    let retained = append.catalog.retained_bytes();
    let exact = retained + scratch;
    let accepted = append
        .insert(
            owner.get(0).unwrap(),
            Limits {
                max_bytes: exact,
                ..Limits::default()
            },
        )
        .unwrap();
    assert!(!accepted.insertion.inserted);
    assert_eq!(accepted.insertion.storage.retained_bytes, retained);
    assert_eq!(accepted.scratch_bytes, scratch);
    assert_eq!(accepted.insertion.storage.peak_construction_bytes, exact);
    let error = append
        .insert(
            owner.get(0).unwrap(),
            Limits {
                max_bytes: exact - 1,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(
        matches!(error.error, Failure::Limit { resource: Resource::Bytes, observed, .. } if observed == exact as u128)
    );
    assert_eq!(error.retained_bytes, retained);
    assert_eq!(append.scratch_bytes(), scratch);
    assert_eq!(append.catalog.len(), 1);
}

#[test]
fn foreign_rows_do_not_change_session_membership() {
    let owner = owner(&[atom(&[1, 2])]);
    let foreign = self::owner(&[atom(&[1, 2])]);
    let mut catalog = catalog(&owner);
    let mut append = catalog.appender(Limits::default()).unwrap();
    append
        .insert(owner.get(0).unwrap(), Limits::default())
        .unwrap();
    let before = columns(append.catalog, &owner);
    let error = append
        .insert(foreign.get(0).unwrap(), Limits::default())
        .unwrap_err();
    assert!(matches!(error.error, Failure::Read(_)));
    assert_eq!(columns(append.catalog, &owner), before);
    assert!(
        !append
            .insert(owner.get(0).unwrap(), Limits::default())
            .unwrap()
            .insertion
            .inserted
    );
}

#[test]
fn session_creation_admits_its_header() {
    let owner = owner(&[atom(&[1, 2])]);
    let mut catalog = catalog(&owner);
    let retained = catalog.retained_bytes();
    let exact = retained + std::mem::size_of::<super::Appender<'_>>();
    let error = catalog
        .appender(Limits {
            max_bytes: exact - 1,
            ..Limits::default()
        })
        .err()
        .unwrap();
    assert!(
        matches!(error.error, Failure::Limit { resource: Resource::Bytes, observed, .. } if observed == exact as u128)
    );
    assert_eq!(error.peak_construction_bytes, retained);
    assert_eq!(catalog.retained_bytes(), retained);
    let append = catalog
        .appender(Limits {
            max_bytes: exact,
            ..Limits::default()
        })
        .unwrap();
    assert_eq!(append.scratch_bytes(), exact - retained);
}
