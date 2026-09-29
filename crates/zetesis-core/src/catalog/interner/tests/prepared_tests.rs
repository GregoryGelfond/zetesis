//! Exclusive prepared results retain identity and named scratch admission.

use super::*;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

#[test]
fn canonical_only_entry_retains_the_exact_row() {
    let mut owner = AtomInterner::new();
    let identity = owner
        .store
        .import_atom(&atom(7), TermLimits::default())
        .unwrap();
    let snapshot = owner.store.snapshot(0).unwrap();
    let row = AtomRef::new(&snapshot, identity).unwrap();
    let entry = owner.entry_atom_with(row, limits(), PERMIT).unwrap();
    assert_eq!(entry.position(), None);
    assert_eq!(
        entry
            .prepared
            .as_ref()
            .and_then(storage::PreparedAtom::present),
        Some(identity)
    );
    assert_eq!(entry.insert_with(limits(), PERMIT).unwrap(), 0);
    assert_eq!(owner.pending, [identity]);
    validate(&owner);
}

#[test]
fn entry_rechecks_prepared_storage_before_occupied_return() {
    let mut owner = owner(&[7]);
    owner.commit_with(limits(), PERMIT).unwrap();
    let before = owner.storage_bytes();
    let (prefix, mut append) = owner.split();
    let row = prefix.get(0).unwrap();
    let entry = append.entry_atom_with(row, limits(), PERMIT).unwrap();
    let exact = entry.storage_bytes();
    assert_eq!(exact, before + PREPARED_BYTES);
    let short = Limits {
        max_bytes: exact - 1,
        ..limits()
    };
    assert!(matches!(entry.insert_with(short, || Err("no work")),
        Err(Failure::Bytes { required, limit }) if required == exact && limit == exact - 1));
    assert_eq!(append.len(), 1);
}

#[test]
fn entry_accepts_its_exact_prepared_storage() {
    let mut owner = owner(&[7]);
    owner.commit_with(limits(), PERMIT).unwrap();
    let exact = Limits {
        max_bytes: owner.storage_bytes() + PREPARED_BYTES,
        ..limits()
    };
    let (prefix, mut append) = owner.split();
    let entry = append
        .entry_atom_with(prefix.get(0).unwrap(), exact, PERMIT)
        .unwrap();
    assert_eq!(
        entry
            .insert_with(exact, || Err("occupied needs no further work"))
            .unwrap(),
        0
    );
}

#[test]
fn prepared_entry_rechecks_population() {
    let mut owner = owner(&[7]);
    owner.commit_with(limits(), PERMIT).unwrap();
    let (prefix, mut append) = owner.split();
    let entry = append
        .entry_atom_with(prefix.get(0).unwrap(), limits(), PERMIT)
        .unwrap();
    assert!(matches!(
        entry.insert_with(
            Limits {
                max_atoms: 0,
                ..limits()
            },
            PERMIT
        ),
        Err(Failure::Atoms {
            required: 1,
            limit: 0
        })
    ));
}

#[test]
fn canonical_only_retry_creates_one_discovery() {
    let mut measured = AtomInterner::new();
    let identity = measured
        .store
        .import_atom(&atom(7), TermLimits::default())
        .unwrap();
    let snapshot = measured.store.snapshot(0).unwrap();
    let row = AtomRef::new(&snapshot, identity).unwrap();
    let mut work = 0;
    measured
        .entry_atom_with(row, limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), || {
            work += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for cutoff in 0..work {
        let mut owner = AtomInterner::new();
        let identity = owner
            .store
            .import_atom(&atom(7), TermLimits::default())
            .unwrap();
        let snapshot = owner.store.snapshot(0).unwrap();
        let row = AtomRef::new(&snapshot, identity).unwrap();
        let entry = owner.entry_atom_with(row, limits(), PERMIT).unwrap();
        let mut admitted = 0;
        assert!(matches!(entry.insert_with(limits(), || {
            if admitted == cutoff { Err(cutoff) } else { admitted += 1; Ok(()) }
        }), Err(Failure::Stopped(actual)) if actual == cutoff));
        assert!(owner.is_empty());
        validate(&owner);
        assert_eq!(
            owner
                .entry_atom_with(row, limits(), PERMIT)
                .unwrap()
                .insert_with(limits(), PERMIT)
                .unwrap(),
            0
        );
        assert_eq!(
            owner
                .entry_atom_with(row, limits(), PERMIT)
                .unwrap()
                .insert_with(limits(), PERMIT)
                .unwrap(),
            0
        );
        assert_eq!(owner.pending, [identity]);
        validate(&owner);
    }
}

#[test]
fn prepared_key_publishes_the_authenticated_tuple() {
    let mut owner = owner(&[1]);
    let value = owner
        .store
        .import_value(&Value::Number(2), TermLimits::default())
        .unwrap();
    owner.commit_with(limits(), PERMIT).unwrap();
    let (prefix, mut append) = owner.split();
    let predicate = prefix.get(0).unwrap().predicate();
    let term = crate::catalog::TermRef::new(prefix.read().storage(), value).unwrap();
    let parts = [crate::TemplateTerm::Constant(term)];
    let pattern = crate::PatternRef::from_parts(predicate, &parts).unwrap();
    let key = pattern.key(&[] as &[Value]).unwrap();
    let entry = append.entry_key_with(key, limits(), PERMIT).unwrap();
    assert!(
        entry
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.present().is_none())
    );
    assert_eq!(entry.insert_with(limits(), PERMIT).unwrap(), 1);
    assert_eq!(append.get(1).unwrap(), AtomRef::from(&atom(2)));
}

#[test]
fn canonical_only_entry_rechecks_prepared_storage() {
    let mut owner = AtomInterner::new();
    let identity = owner
        .store
        .import_atom(&atom(7), TermLimits::default())
        .unwrap();
    let snapshot = owner.store.snapshot(0).unwrap();
    let row = AtomRef::new(&snapshot, identity).unwrap();
    let entry = owner.entry_atom_with(row, limits(), PERMIT).unwrap();
    let exact = entry.storage_bytes();
    assert_eq!(exact, entry.appender.storage_bytes() + PREPARED_BYTES);
    let short = Limits {
        max_bytes: exact - 1,
        ..limits()
    };
    assert!(matches!(entry.insert_with(short, || Err("no work")),
        Err(Failure::Bytes { required, limit }) if required == exact && limit == exact - 1));
    assert!(owner.is_empty());
    assert_eq!(owner.store.snapshot(0).unwrap().atom_count(), 1);
}
