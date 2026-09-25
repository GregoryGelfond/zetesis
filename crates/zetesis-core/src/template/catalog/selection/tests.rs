//! Scope, publication and failure controls for source-backed template metadata.

use super::*;
use crate::atom_interner::{AtomInterner, Limits};
use crate::{Atom, Predicate, TemplateCatalogBuilder, Value, ValueNodeRef};

const METADATA: usize = 1024 * 1024;
fn limits() -> Limits {
    Limits {
        max_atoms: 32,
        max_bytes: 4 * 1024 * 1024,
    }
}
const PERMIT: fn() -> Result<(), Infallible> = || Ok(());
fn atom(name: &str, value: Value) -> Atom {
    Atom::new(Predicate::new(name, 1).unwrap(), vec![value]).unwrap()
}
fn insert(owner: &mut AtomInterner, atom: &Atom) -> usize {
    owner
        .entry_atom_with(atom, limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), PERMIT)
        .unwrap()
}
fn owner() -> AtomInterner {
    let mut owner = AtomInterner::new();
    insert(
        &mut owner,
        &atom("p", Value::String("shared spelling".repeat(64))),
    );
    owner
}
fn publish(owner: &mut AtomInterner, positions: &[usize]) -> AtomCatalog {
    owner
        .publish_selection_with(positions, limits(), PERMIT)
        .unwrap()
}
fn append<E>(
    selection: &mut TemplateCatalogSelection,
    owner: &AtomInterner,
    max_bytes: usize,
    before: impl FnMut() -> Result<(), E>,
) -> Result<usize, TemplateCatalogFailure<E>> {
    let atom = owner.get(0).unwrap();
    let constant = TemplateTerm::Constant(atom.values().at(0).unwrap());
    let terms = [constant, TemplateTerm::Variable(3), constant];
    let pattern_terms = [constant];
    let pattern = PatternRef::from_parts(atom.predicate(), &pattern_terms).unwrap();
    selection.append_with(
        owner.read(),
        terms,
        [pattern, pattern],
        [FilterRef::from_parts(
            false,
            constant,
            TemplateTerm::Variable(3),
        )],
        max_bytes,
        before,
    )
}

#[test]
fn selected_rows_preserve_occurrences_and_borrow_payload() {
    let mut owner = owner();
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    assert_eq!(append(&mut selection, &owner, METADATA, PERMIT).unwrap(), 0);
    assert_eq!(append(&mut selection, &owner, METADATA, PERMIT).unwrap(), 1);
    let source = publish(&mut owner, &[0, 0]);
    let catalog = selection
        .finish_with(source.clone(), METADATA, PERMIT)
        .unwrap();
    assert!(catalog.source_catalog().unwrap().same_owner(&source));
    assert_eq!(
        catalog.storage_bytes(),
        source.storage().bytes + catalog.metadata_bytes()
    );
    assert_eq!(catalog.len(), 2);
    let atom = source.atoms().at(0).unwrap();
    let constant = TemplateTerm::Constant(atom.values().at(0).unwrap());
    for position in 0..2 {
        let row = catalog.at(position).unwrap();
        assert_eq!(
            row.terms().iter().collect::<Vec<_>>(),
            [constant, TemplateTerm::Variable(3), constant]
        );
        assert_eq!(row.patterns().len(), 2);
        for pattern in row.patterns() {
            assert_eq!(pattern.predicate(), atom.predicate());
            assert_eq!(pattern.terms().iter().collect::<Vec<_>>(), [constant]);
        }
        let filter = row.filters().at(0).unwrap();
        assert!(!filter.is_equality());
        assert_eq!(filter.terms(), (constant, TemplateTerm::Variable(3)));
        let TemplateTerm::Constant(actual) = row.terms().at(0).unwrap() else {
            panic!("constant")
        };
        let ValueNodeRef::String(actual) = actual.descriptor() else {
            panic!("string")
        };
        let ValueNodeRef::String(expected) = atom.values().at(0).unwrap().descriptor() else {
            panic!("string")
        };
        assert_eq!(actual.as_ptr(), expected.as_ptr());
    }
}

#[test]
fn publication_requires_the_original_vocabulary_even_when_empty() {
    let owner = owner();
    let selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let mut foreign = self::owner();
    assert!(matches!(
        selection.finish_with(publish(&mut foreign, &[0]), METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::ForeignCatalog))
    ));
}

#[test]
fn publication_requires_every_referenced_term() {
    let mut owner = owner();
    let old = publish(&mut owner, &[0]);
    insert(&mut owner, &atom("p", Value::Number(17)));
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let term = TemplateTerm::Constant(owner.get(1).unwrap().values().at(0).unwrap());
    selection
        .append_with(owner.read(), [term], [], [], METADATA, PERMIT)
        .unwrap();
    assert!(matches!(
        selection.finish_with(old, METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
}

#[test]
fn publication_requires_every_referenced_signature() {
    let mut owner = owner();
    let old = publish(&mut owner, &[0]);
    insert(
        &mut owner,
        &Atom::new(Predicate::new("later", 0).unwrap(), vec![]).unwrap(),
    );
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let pattern = PatternRef::from_parts(owner.get(1).unwrap().predicate(), &[]).unwrap();
    selection
        .append_with(owner.read(), [], [pattern], [], METADATA, PERMIT)
        .unwrap();
    assert!(matches!(
        selection.finish_with(old, METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    ));
}

#[test]
fn append_requires_canonical_constants() {
    let owner = owner();
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let value = Value::Number(3);
    assert_eq!(
        selection.append_with(
            owner.read(),
            [TemplateTerm::Constant((&value).into())],
            [],
            [],
            METADATA,
            PERMIT
        ),
        Err(TemplateCatalogFailure::Read(ReadError::Uninterned))
    );
    assert!(selection.is_empty());
}

#[test]
fn append_requires_canonical_signatures() {
    let owner = owner();
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let predicate = Predicate::new("p", 0).unwrap();
    let pattern = PatternRef::from_parts((&predicate).into(), &[]).unwrap();
    assert_eq!(
        selection.append_with(owner.read(), [], [pattern], [], METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::Uninterned))
    );
}

#[test]
fn append_validates_the_supplied_read_prefix() {
    let mut owner = owner();
    let old = publish(&mut owner, &[0]);
    insert(&mut owner, &atom("p", Value::Number(19)));
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let term = TemplateTerm::Constant(owner.get(1).unwrap().values().at(0).unwrap());
    assert_eq!(
        selection.append_with(old.read(), [term], [], [], METADATA, PERMIT),
        Err(TemplateCatalogFailure::Read(ReadError::OutsidePrefix))
    );
}

#[test]
fn interrupted_append_cannot_publish_a_partial_row() {
    let mut owner = owner();
    let mut baseline = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    let mut work = 0;
    append(&mut baseline, &owner, METADATA, || {
        work += 1;
        Ok::<(), usize>(())
    })
    .unwrap();
    let source = publish(&mut owner, &[0]);
    for cut in 0..work {
        let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
        let mut visited = 0;
        let result = append(&mut selection, &owner, METADATA, || {
            if visited == cut {
                return Err(cut);
            }
            visited += 1;
            Ok(())
        });
        assert_eq!(result, Err(TemplateCatalogFailure::Stopped(cut)));
        assert_eq!(visited, cut);
        assert_eq!(selection.len(), 0);
        assert!(selection.storage_bytes() <= baseline.storage_bytes());
        assert!(selection.storage_peak_bytes() <= baseline.storage_peak_bytes());
        assert!(matches!(
            selection.finish_with(source.clone(), METADATA, PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        ));
    }
}

#[test]
fn metadata_allowance_includes_publication_overlap() {
    let mut owner = owner();
    let source = publish(&mut owner, &[0]);
    let make = || {
        let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
        append(&mut selection, &owner, METADATA, PERMIT).unwrap();
        selection
    };
    let selection = make();
    let required = usize::try_from(selection.publication_peak_bytes()).unwrap();
    selection
        .finish_with(source.clone(), required, PERMIT)
        .unwrap();
    assert!(matches!(make().finish_with(source, required - 1, PERMIT),
        Err(TemplateCatalogFailure::Storage(Error::Storage { required: actual, limit }))
            if actual == required as u128 && limit == required - 1));
}

#[test]
fn selected_catalogs_refuse_indexed_writer_construction() {
    let mut owner = owner();
    let selected = TemplateCatalogSelection::new(owner.read(), METADATA)
        .unwrap()
        .finish_with(publish(&mut owner, &[0]), METADATA, PERMIT)
        .unwrap();
    assert!(matches!(
        AtomInterner::for_template_catalog(&selected, METADATA),
        Err(Error::UnindexedVocabulary)
    ));
    let closed = TemplateCatalogBuilder::new(METADATA)
        .unwrap()
        .finish()
        .unwrap();
    assert!(AtomInterner::for_template_catalog(&closed, METADATA).is_ok());
}

#[test]
fn exact_snapshot_sharing_is_distinct_from_occurrences() {
    let mut owner = owner();
    let first = publish(&mut owner, &[0]);
    let second = publish(&mut owner, &[0, 0]);
    assert!(!first.same_owner(&second));
    assert!(first.shares_snapshot(&second));
    assert_eq!(
        first.shared_snapshot_bytes(),
        second.shared_snapshot_bytes()
    );
    assert_eq!(
        first.shared_snapshot_bytes(),
        first.storage().bytes - first.publication_bytes()
    );
    insert(&mut owner, &atom("p", Value::Number(23)));
    let later = publish(&mut owner, &[0]);
    assert!(!first.shares_snapshot(&later));
    assert!(first.shares_terms(&later));
}

#[test]
fn borrowed_pattern_parts_enforce_arity() {
    let owner = owner();
    assert!(matches!(
        PatternRef::from_parts(owner.get(0).unwrap().predicate(), &[]),
        Err(crate::ConstructionError::ArityMismatch {
            expected: 1,
            actual: 0
        })
    ));
}

#[test]
fn peak_restart_excludes_previous_growth_overlap() {
    let owner = owner();
    let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
    for _ in 0..5 {
        selection
            .append_with(owner.read(), [], [], [], METADATA, PERMIT)
            .unwrap();
    }
    let current = selection.storage_bytes();
    assert!(selection.storage_peak_bytes() > current);
    selection.restart_storage_peak();
    selection
        .append_with(
            owner.read(),
            [],
            [],
            [],
            usize::try_from(current).unwrap(),
            PERMIT,
        )
        .unwrap();
    assert_eq!(selection.storage_peak_bytes(), current);
    assert_eq!(selection.len(), 6);
}

#[test]
fn a_panicking_callback_poisons_publication() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut owner = owner();
    let source = publish(&mut owner, &[0]);
    let make = || {
        let mut selection = TemplateCatalogSelection::new(owner.read(), METADATA).unwrap();
        for _ in 0..4 {
            selection
                .append_with(owner.read(), [], [], [], METADATA, PERMIT)
                .unwrap();
        }
        selection
    };
    let mut work = 0;
    make()
        .append_with(owner.read(), [], [], [], METADATA, || {
            work += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
    for cut in 0..work {
        let mut selection = make();
        let mut visited = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            selection.append_with(owner.read(), [], [], [], METADATA, || {
                assert_ne!(visited, cut, "injected callback unwind");
                visited += 1;
                Ok::<(), Infallible>(())
            })
        }));
        assert!(result.is_err());
        assert_eq!(visited, cut);
        assert_eq!(
            selection.append_with(owner.read(), [], [], [], METADATA, PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        );
        assert!(matches!(
            selection.finish_with(source.clone(), METADATA, PERMIT),
            Err(TemplateCatalogFailure::Incomplete)
        ));
    }
}
