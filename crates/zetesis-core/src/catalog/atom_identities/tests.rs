use super::*;
use crate::catalog::{AtomCatalog, Limits};
use crate::{Atom, Predicate, Value};

fn atom(name: &str) -> Atom {
    Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(0)]).unwrap()
}

fn catalog(names: &[&str]) -> AtomCatalog {
    AtomCatalog::new(names.iter().map(|name| atom(name)).collect()).unwrap()
}

#[test]
fn a_recorded_atom_is_answered_by_its_identity() {
    let catalog = catalog(&["p", "q"]);
    let mut map = AtomIdentityMap::default();
    for (position, atom) in catalog.atoms().iter().enumerate() {
        assert!(map.insert(atom, position).unwrap());
    }
    for (position, atom) in catalog.atoms().iter().enumerate() {
        assert_eq!(map.get(atom), Some(position));
    }
}

#[test]
fn a_newer_read_of_the_owner_finds_a_recorded_atom() {
    // Identities are stable across an owner's publications.
    let mut store = storage::Store::new(usize::MAX);
    let p = store.import_atom(&atom("p"), Limits::default()).unwrap();
    let old = store.snapshot(0).unwrap();
    let q = store.import_atom(&atom("q"), Limits::default()).unwrap();
    let new = store.snapshot(0).unwrap();
    let mut map = AtomIdentityMap::default();
    assert!(map.insert(AtomRef::new(&old, p).unwrap(), 7).unwrap());
    assert_eq!(map.get(AtomRef::new(&new, p).unwrap()), Some(7));
    assert_eq!(map.get(AtomRef::new(&new, q).unwrap()), None);
}

#[test]
fn another_owner_has_no_entry_for_an_equal_atom() {
    let recorded = catalog(&["p"]);
    let other = catalog(&["p"]);
    let mut map = AtomIdentityMap::default();
    map.insert(recorded.atoms().at(0).unwrap(), 1).unwrap();
    assert_eq!(map.get(other.atoms().at(0).unwrap()), None);
}

#[test]
fn owned_ingress_has_no_identity() {
    let ingress = atom("p");
    let mut map = AtomIdentityMap::default();
    assert!(!map.insert(AtomRef::from(&ingress), 1).unwrap());
    assert_eq!(map.get(AtomRef::from(&ingress)), None);
    assert!(map.is_empty());
}

#[test]
fn several_owners_keep_separate_entries() {
    // Equal identities of independent owners name different atoms.
    let left = catalog(&["p", "q"]);
    let right = catalog(&["q", "p"]);
    let mut map = AtomIdentityMap::default();
    for (offset, catalog) in [(0, &left), (10, &right)] {
        for (position, atom) in catalog.atoms().iter().enumerate() {
            map.insert(atom, offset + position).unwrap();
        }
    }
    assert_eq!(map.len(), 4);
    assert_eq!(map.get(left.atoms().at(1).unwrap()), Some(1));
    assert_eq!(map.get(right.atoms().at(1).unwrap()), Some(11));
}

#[test]
fn retain_drops_entries_by_value() {
    let catalog = catalog(&["p", "q", "r"]);
    let mut map = AtomIdentityMap::default();
    for (position, atom) in catalog.atoms().iter().enumerate() {
        map.insert(atom, position).unwrap();
    }
    map.retain(|position| position < 2);
    let answers: Vec<_> = catalog.atoms().iter().map(|atom| map.get(atom)).collect();
    assert_eq!(answers, [Some(0), Some(1), None]);
}

#[test]
fn retained_bytes_grow_with_entries() {
    let catalog = catalog(&["p", "q"]);
    let mut map = AtomIdentityMap::default();
    let empty = map.retained_bytes();
    assert_eq!(empty, size_of::<AtomIdentityMap<usize>>());
    for (position, atom) in catalog.atoms().iter().enumerate() {
        map.insert(atom, position).unwrap();
    }
    assert!(
        map.retained_bytes()
            >= empty + size_of::<Scoped<usize>>() + 2 * size_of::<(storage::AtomId, usize)>()
    );
}
