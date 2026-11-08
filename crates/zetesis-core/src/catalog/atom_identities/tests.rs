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

#[test]
fn retain_held_drops_an_owner_nothing_else_holds() {
    let mut map = AtomIdentityMap::default();
    {
        let dropped = catalog(&["p"]);
        map.insert(dropped.atoms().at(0).unwrap(), 1).unwrap();
    }
    map.retain_held();
    assert!(map.is_empty());
}

#[test]
fn retain_held_keeps_an_owner_a_catalog_still_holds() {
    let held = catalog(&["p"]);
    let mut map = AtomIdentityMap::default();
    map.insert(held.atoms().at(0).unwrap(), 1).unwrap();
    map.retain_held();
    assert_eq!(map.get(held.atoms().at(0).unwrap()), Some(1));
}

#[test]
fn retain_held_keeps_the_owner_of_a_live_writer_without_catalogs() {
    let mut store = storage::Store::new(usize::MAX);
    let p = store.import_atom(&atom("p"), Limits::default()).unwrap();
    let mut map = AtomIdentityMap::default();
    {
        let published = store.snapshot(0).unwrap();
        map.insert(AtomRef::new(&published, p).unwrap(), 3).unwrap();
    }
    // Every published snapshot is gone; the writer can still present `p`.
    map.retain_held();
    assert_eq!(map.owners(), 1);
    let again = store.snapshot(0).unwrap();
    assert_eq!(map.get(AtomRef::new(&again, p).unwrap()), Some(3));
}

#[test]
fn many_owners_each_keep_their_own_entries() {
    let catalogs: Vec<_> = (0..1000).map(|_| catalog(&["p"])).collect();
    let mut map = AtomIdentityMap::default();
    for (index, catalog) in catalogs.iter().enumerate() {
        map.insert(catalog.atoms().at(0).unwrap(), index).unwrap();
    }
    assert_eq!(map.owners(), 1000);
    for (index, catalog) in catalogs.iter().enumerate() {
        assert_eq!(map.get(catalog.atoms().at(0).unwrap()), Some(index));
    }
}

#[test]
fn a_prune_returns_the_owner_tables_excess_capacity() {
    let mut map = AtomIdentityMap::default();
    let empty = map.retained_bytes();
    {
        let catalogs: Vec<_> = (0..256).map(|_| catalog(&["p"])).collect();
        for (index, catalog) in catalogs.iter().enumerate() {
            map.insert(catalog.atoms().at(0).unwrap(), index).unwrap();
        }
    }
    assert!(map.retained_bytes() > empty);
    map.retain_held();
    assert_eq!(map.owners(), 0);
    assert_eq!(map.retained_bytes(), empty);
}

/// Atoms read through descendants of one closed base, as reconstructed answers
/// are: each descendant discovers a base answer's atoms, which are base rows,
/// and may append atoms of its own, at IDs from the base's count on.
mod shared_base {
    use super::*;
    use crate::Model;
    use crate::catalog::interner::{AtomInterner, ClosedCatalog, Limits as InternerLimits};
    use crate::test_support::PERMIT;

    fn limits() -> InternerLimits {
        InternerLimits::for_atoms(64, 1 << 20)
    }

    fn number(name: &str, value: i32) -> Atom {
        Atom::new(Predicate::new(name, 1).unwrap(), vec![Value::Number(value)]).unwrap()
    }

    fn insert(owner: &mut AtomInterner, atom: &Atom) {
        owner
            .entry_atom_with(atom, limits(), PERMIT)
            .unwrap()
            .insert_with(limits(), PERMIT)
            .unwrap();
    }

    /// A closed base over `p(0)`, `p(1)`, `p(2)` and `q(0)`; the catalog its
    /// source writer published before closing; and a model of that catalog
    /// selecting `p(0)` and `p(1)`, as a base answer does. A descendant shares
    /// the base's frozen vocabulary, so the rows it can append, such as `q(1)`,
    /// combine the predicates and terms already there.
    fn closed_base() -> (ClosedCatalog, AtomCatalog, Model) {
        let mut source = AtomInterner::new();
        for value in 0..3 {
            insert(&mut source, &number("p", value));
        }
        insert(&mut source, &number("q", 0));
        let catalog = source
            .publish_selection_with(&[0, 1, 2, 3], limits(), PERMIT)
            .unwrap();
        let model = Model::from_positions(&catalog, [0, 1]).unwrap();
        let base = source.into_closed_with(limits(), PERMIT).unwrap();
        (base, catalog, model)
    }

    /// A descendant of `base` that discovers `model`'s atoms and appends
    /// `q(value)` for each of `appended`, published as an answer is.
    fn descendant(base: &ClosedCatalog, model: &Model, appended: &[i32]) -> AtomCatalog {
        let mut writer = AtomInterner::for_closed_catalog(base, usize::MAX).unwrap();
        writer
            .discover_model_with(base, model, limits(), PERMIT)
            .unwrap();
        for &value in appended {
            insert(&mut writer, &number("q", value));
        }
        writer.into_ordered_catalog_with(limits(), PERMIT).unwrap()
    }

    fn find<'a>(catalog: &'a AtomCatalog, name: &str, value: i32) -> AtomRef<'a> {
        catalog
            .atoms()
            .iter()
            .find(|atom| *atom == number(name, value))
            .unwrap()
    }

    #[test]
    fn a_base_row_recorded_through_one_descendant_is_found_through_another() {
        let (base, _, model) = closed_base();
        let left = descendant(&base, &model, &[]);
        let right = descendant(&base, &model, &[]);
        let mut map = AtomIdentityMap::default();
        assert!(map.insert(find(&left, "p", 1), 5).unwrap());
        assert_eq!(map.get(find(&right, "p", 1)), Some(5));
        assert_eq!(map.owners(), 1);
    }

    #[test]
    fn appended_rows_of_different_descendants_keep_distinct_identities() {
        // Both descendants append q(1) as their first row past the base, so its
        // ID is the same in each; only the scope tells the two rows apart.
        let (base, _, model) = closed_base();
        let left = descendant(&base, &model, &[1]);
        let right = descendant(&base, &model, &[1]);
        let id = |catalog| find(catalog, "q", 1).canonical().unwrap().1;
        assert_eq!(id(&left), id(&right));
        let mut map = AtomIdentityMap::default();
        map.insert(find(&left, "q", 1), 5).unwrap();
        assert_eq!(map.get(find(&right, "q", 1)), None);
        assert_eq!(map.get(find(&left, "q", 1)), Some(5));
    }

    #[test]
    fn a_base_row_has_the_identity_of_its_source_writer() {
        let (base, source, model) = closed_base();
        let answer = descendant(&base, &model, &[]);
        let mut map = AtomIdentityMap::default();
        map.insert(find(&source, "p", 0), 7).unwrap();
        assert_eq!(map.get(find(&answer, "p", 0)), Some(7));
    }

    #[test]
    fn base_rows_of_different_closed_bases_keep_distinct_identities() {
        // Equal sources close to equal rows under equal IDs.
        let (left_base, _, left_model) = closed_base();
        let (right_base, _, right_model) = closed_base();
        let left = descendant(&left_base, &left_model, &[]);
        let right = descendant(&right_base, &right_model, &[]);
        let mut map = AtomIdentityMap::default();
        map.insert(find(&left, "p", 1), 5).unwrap();
        assert_eq!(map.get(find(&right, "p", 1)), None);
    }

    #[test]
    fn retain_held_keeps_a_base_scope_its_closed_catalog_holds() {
        let mut map = AtomIdentityMap::default();
        let (base, id) = {
            let (base, _, model) = closed_base();
            let answer = descendant(&base, &model, &[]);
            let atom = find(&answer, "p", 0);
            map.insert(atom, 3).unwrap();
            (base, atom.canonical().unwrap().1)
        };
        // The base answer's model held the base scope through the source
        // writer's catalog. With the model and the answer gone, only the
        // closed catalog holds the scope.
        map.retain_held();
        assert_eq!(map.owners(), 1);
        // The closed catalog's own read presents the row under that scope.
        let atom = AtomRef::new(base.read().0, id).unwrap();
        assert_eq!(map.get(atom), Some(3));
    }

    #[test]
    fn retain_held_drops_a_base_scope_nothing_else_holds() {
        let mut map = AtomIdentityMap::default();
        {
            let (base, _, model) = closed_base();
            let answer = descendant(&base, &model, &[]);
            map.insert(find(&answer, "p", 0), 3).unwrap();
        }
        map.retain_held();
        assert_eq!(map.owners(), 0);
    }
}
