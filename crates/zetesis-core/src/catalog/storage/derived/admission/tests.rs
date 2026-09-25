use super::super::super::{budget::Budget, index::Index};
use super::*;
use crate::{Value, catalog::VocabularyBuilder};

#[test]
fn colliding_filters_still_distinguish_typed_root_descriptors() {
    let logical = Limits {
        max_nodes: 8,
        max_depth: 8,
        max_bytes: 1024,
    };
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let string = builder
        .import_term_with((&Value::String("7".into())).into(), logical, || {
            Ok::<_, ()>(())
        })
        .unwrap();
    let symbol = builder
        .import_term_with((&Value::Symbol("7".into())).into(), logical, || {
            Ok::<_, ()>(())
        })
        .unwrap();
    let owner = builder.finish_with(0, || Ok::<_, ()>(())).unwrap();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, || Ok::<_, ()>(())).unwrap();
    let number = arena
        .scalar_with(ValueNodeRef::Number(7), logical, || Ok::<_, ()>(()))
        .unwrap();
    let string = arena
        .borrow_with(owner.read().term(&string).unwrap(), || Ok::<_, ()>(()))
        .unwrap();
    let symbol = arena
        .borrow_with(owner.read().term(&symbol).unwrap(), || Ok::<_, ()>(()))
        .unwrap();
    // Deliberately equal hash filters exercise the production candidate equality
    // through the production chain, without changing the arena's own indexes.
    let mut collisions = Index::default();
    let mut budget = Budget::new(1 << 20, 0);
    for id in [number.id, string.id, symbol.id] {
        collisions.reserve(42, &mut budget).unwrap();
        collisions.insert(42, id.0);
    }
    let mut before = || Ok::<_, ()>(());
    let found = collisions
        .find_with(42, &mut Work::new(&mut before), |candidate, work| {
            arena.equal(TermId(candidate), ValueNodeRef::String("7"), work)
        })
        .unwrap();
    assert_eq!(found, Some(string.id.0));
}
