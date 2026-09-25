use super::super::budget;
use super::*;
use crate::{
    Sign, Value, ValueLimits, ValueNode,
    catalog::{Limits, Vocabulary, VocabularyBuilder},
};

const PERMIT: fn() -> Result<(), ()> = || Ok(());
fn logical() -> Limits {
    Limits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}
fn fixture() -> (Vocabulary, TermKey, TermKey, DeclaredConstructor) {
    let mut builder = VocabularyBuilder::new(1 << 22).unwrap();
    let number = builder
        .import_term_with((&Value::Number(7)).into(), logical(), PERMIT)
        .unwrap();
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 2,
            },
            ValueNode::Number(7),
            ValueNode::Number(7),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let term = builder
        .import_term_with((&value).into(), logical(), PERMIT)
        .unwrap();
    let shape = builder
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 2,
            },
            PERMIT,
        )
        .unwrap();
    (builder.finish_with(0, PERMIT).unwrap(), number, term, shape)
}
fn assigned(arena: &DerivedTerms<'_>, key: &TermKey) -> crate::catalog::TermAssignment {
    let mut values = arena.read().assignment();
    values.resize_with(1, 1 << 20, PERMIT).unwrap();
    values.set_with(0, key, PERMIT).unwrap();
    values
}

#[test]
fn constructed_root_and_registered_input_share_one_identity() {
    let (owner, number, original, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let key = arena
        .borrow_with(owner.read().term(&number).unwrap(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &key);
    let constructed = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    let alias = arena
        .borrow_with(owner.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    assert_eq!(constructed.id, alias.id);
    assert_eq!(arena.roots.len(), 2);
    assert_eq!(arena.nodes.kinds.len(), 1);
}

#[test]
fn registered_root_reuses_payload_when_constructed_later() {
    let (owner, number, original, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let alias = arena
        .borrow_with(owner.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    let key = arena
        .borrow_with(owner.read().term(&number).unwrap(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &key);
    let constructed = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    assert_eq!(constructed.id, alias.id);
    assert!(arena.nodes.kinds.is_empty());
}

#[test]
fn foreign_equal_inputs_are_canonicalized_by_content() {
    let (left, _, original, _) = fixture();
    let (right, _, equal, _) = fixture();
    let mut arena = DerivedTerms::new_with(&[left.read(), right.read()], 1 << 20, PERMIT).unwrap();
    let one = arena
        .borrow_with(left.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    let two = arena
        .borrow_with(right.read().term(&equal).unwrap(), PERMIT)
        .unwrap();
    assert_eq!(one.id, two.id);
    assert_eq!(arena.roots.len(), 1);
}

#[test]
fn unregistered_owner_is_not_accepted_by_equal_numeric_id() {
    let (left, _, _, _) = fixture();
    let (right, number, _, _) = fixture();
    let mut arena = DerivedTerms::new_with(&[left.read()], 1 << 20, PERMIT).unwrap();
    assert!(matches!(
        arena.borrow_with(right.read().term(&number).unwrap(), PERMIT),
        Err(DerivedFailure::Read(ReadError::ForeignCatalog))
    ));
    assert!(arena.roots.is_empty());
}

#[test]
fn generated_nullary_function_uses_declared_text_without_a_source_term() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let shape = builder
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "fresh",
                sign: Sign::Positive,
                arity: 0,
            },
            PERMIT,
        )
        .unwrap();
    let owner = builder.finish_with(0, PERMIT).unwrap();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let empty = arena.read().assignment();
    let key = arena
        .construct_with(&shape, empty.as_slice(), &[], logical(), PERMIT)
        .unwrap();
    assert_eq!(
        arena.read().term(&key).unwrap().descriptor(),
        ValueNodeRef::Symbol("fresh")
    );
    let projected = arena.read().constructor_of(&key).unwrap().unwrap();
    assert_eq!(
        owner.read().constructor(&projected).unwrap(),
        ValueNodeRef::Function {
            name: "fresh",
            sign: Sign::Positive,
            arity: 0
        }
    );
}

#[test]
fn repeated_children_keep_expanded_measures_and_shared_spelling() {
    let (owner, _, _, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let key = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &key);
    let root = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    let term = arena.read().term(&root).unwrap();
    assert_eq!(term.expanded_nodes(), 3);
    assert_eq!(term.depth(), 2);
    assert_eq!(term.to_string(), "f(7,7)");
    assert_eq!(term.rendered_bytes(), term.to_string().len());
    assert_eq!(term.nodes().count(), term.expanded_nodes());
}

#[test]
fn every_construction_work_cutoff_keeps_only_complete_indexed_roots() {
    let (owner, _, _, shape) = fixture();
    let run = |stop: usize| {
        let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
        let key = arena
            .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
            .unwrap();
        let values = assigned(&arena, &key);
        let mut count = 0;
        let result = arena.construct_with(&shape, values.as_slice(), &[0, 0], logical(), || {
            count += 1;
            if count == stop { Err(count) } else { Ok(()) }
        });
        if stop <= count {
            assert!(matches!(result, Err(DerivedFailure::Stopped(value)) if value == stop));
            assert_eq!(arena.roots.len(), 1);
        }
        let first = arena
            .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
            .unwrap();
        let second = arena
            .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(arena.roots.len(), 2);
        count
    };
    let steps = run(usize::MAX);
    for stop in 1..=steps {
        run(stop);
    }
}

#[test]
fn storage_refusal_retains_accounted_capacity_and_allows_retry() {
    let (owner, _, _, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let key = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &key);
    let current = usize::try_from(arena.storage_bytes()).unwrap();
    arena.ceiling(current).unwrap();
    assert!(matches!(
        arena.construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT),
        Err(DerivedFailure::Storage(Fault::Storage { .. }))
    ));
    assert_eq!(arena.roots.len(), 1);
    arena.ceiling(1 << 20).unwrap();
    let result = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    assert_eq!(arena.read().term(&result).unwrap().to_string(), "f(7,7)");
    let measured = size_of::<DerivedTerms<'_>>() as u128
        + budget::capacity(&arena.inputs)
        + budget::capacity(&arena.roots)
        + arena.nodes.buffer_bytes()
        + budget::capacity(&arena.symbol_names)
        + arena.index.buffer_bytes()
        + budget::capacity(&arena.children);
    assert_eq!(arena.storage_bytes(), measured);
}

#[test]
fn ordinary_importer_consumes_derived_graph_without_owned_export() {
    let (owner, _, _, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let key = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &key);
    let result = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    let mut destination = VocabularyBuilder::new(1 << 20).unwrap();
    let imported = destination
        .import_term_with(arena.read().term(&result).unwrap(), logical(), PERMIT)
        .unwrap();
    assert_eq!(
        destination.read().term(&imported).unwrap(),
        arena.read().term(&result).unwrap()
    );
}

#[test]
fn a_registered_older_prefix_refuses_a_newer_term() {
    let mut store = super::super::Store::new(1 << 20);
    store.import_value(&Value::Number(1), logical()).unwrap();
    let prefix = store.snapshot(0).unwrap();
    let newer = store.import_value(&Value::Number(2), logical()).unwrap();
    let mut arena =
        DerivedTerms::new_with(&[CatalogRead(Read::from(&prefix))], 1 << 20, PERMIT).unwrap();
    assert!(matches!(
        arena.borrow_with(TermRef::new(&store, newer).unwrap(), PERMIT),
        Err(DerivedFailure::Read(ReadError::OutsidePrefix))
    ));
    assert!(arena.roots.is_empty());
}

#[test]
fn a_later_registered_prefix_can_cover_the_same_input_lineage() {
    let mut store = super::super::Store::new(1 << 20);
    store.import_value(&Value::Number(1), logical()).unwrap();
    let prefix = store.snapshot(0).unwrap();
    let newer = store.import_value(&Value::Number(2), logical()).unwrap();
    let mut arena = DerivedTerms::new_with(
        &[
            CatalogRead(Read::from(&prefix)),
            CatalogRead(Read::from(&store)),
        ],
        1 << 20,
        PERMIT,
    )
    .unwrap();
    let key = arena
        .borrow_with(TermRef::new(&store, newer).unwrap(), PERMIT)
        .unwrap();
    assert_eq!(
        arena.read().term(&key).unwrap().descriptor(),
        ValueNodeRef::Number(2)
    );
}

#[test]
fn declared_name_requires_its_exact_text_prefix() {
    let mut store = super::super::Store::new(1 << 20);
    let prefix = store.snapshot(0).unwrap();
    let data = store
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "later",
                sign: Sign::Positive,
                arity: 0,
            },
            PERMIT,
        )
        .unwrap();
    let shape = DeclaredConstructor::new(Read::from(&store), data);
    let mut arena =
        DerivedTerms::new_with(&[CatalogRead(Read::from(&prefix))], 1 << 20, PERMIT).unwrap();
    let empty = arena.read().assignment();
    assert!(matches!(
        arena.construct_with(&shape, empty.as_slice(), &[], logical(), PERMIT),
        Err(DerivedFailure::Read(ReadError::OutsidePrefix))
    ));
    assert!(arena.roots.is_empty());
}

#[test]
fn a_later_text_only_prefix_covers_a_declared_name() {
    let mut store = super::super::Store::new(1 << 20);
    let prefix = store.snapshot(0).unwrap();
    let data = store
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "later",
                sign: Sign::Positive,
                arity: 0,
            },
            PERMIT,
        )
        .unwrap();
    let shape = DeclaredConstructor::new(Read::from(&store), data);
    let mut arena = DerivedTerms::new_with(
        &[
            CatalogRead(Read::from(&prefix)),
            CatalogRead(Read::from(&store)),
        ],
        1 << 20,
        PERMIT,
    )
    .unwrap();
    let empty = arena.read().assignment();
    let key = arena
        .construct_with(&shape, empty.as_slice(), &[], logical(), PERMIT)
        .unwrap();
    assert_eq!(
        arena.read().term(&key).unwrap().descriptor(),
        ValueNodeRef::Symbol("later")
    );
}

#[test]
fn caught_last_permit_panic_keeps_an_indexed_complete_prefix() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let (owner, _, _, shape) = fixture();
    let prepare = || {
        let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
        let key = arena
            .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
            .unwrap();
        let values = assigned(&arena, &key);
        (arena, values)
    };
    let (mut complete, values) = prepare();
    let mut steps = 0;
    complete
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), || {
            steps += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    let (mut arena, values) = prepare();
    let mut visited = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        arena.construct_with(&shape, values.as_slice(), &[0, 0], logical(), || {
            visited += 1;
            assert!(visited < steps, "stop at the final publication permit");
            Ok::<_, ()>(())
        })
    }));
    assert!(result.is_err());
    assert_eq!(visited, steps);
    assert_eq!(arena.roots.len(), 1);
    assert_eq!(arena.nodes.kinds.len(), 1);
    assert_eq!(arena.storage_bytes(), complete.storage_bytes());
    let root = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    let repeated = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    assert_eq!(root.id, repeated.id);
    assert_eq!(arena.roots.len(), 2);
}

#[test]
fn derived_and_ordinary_views_share_the_public_hash_contract() {
    use std::hash::{Hash, Hasher};
    fn hash(term: TermRef<'_>) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        term.hash(&mut state);
        state.finish()
    }
    let (owner, _, original, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let number = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &number);
    let constructed = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    assert_eq!(
        hash(arena.read().term(&constructed).unwrap()),
        hash(owner.read().term(&original).unwrap())
    );
}

#[test]
fn derived_values_keep_the_shared_asp_and_storage_orders() {
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let string = builder
        .import_term_with((&Value::String("a".into())).into(), logical(), PERMIT)
        .unwrap();
    let symbol = builder
        .import_term_with((&Value::Symbol("a".into())).into(), logical(), PERMIT)
        .unwrap();
    let owner = builder.finish_with(0, PERMIT).unwrap();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let number = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let copied_string = arena
        .borrow_with(owner.read().term(&string).unwrap(), PERMIT)
        .unwrap();
    let copied_symbol = arena
        .borrow_with(owner.read().term(&symbol).unwrap(), PERMIT)
        .unwrap();
    let values = [
        arena.read().term(&number).unwrap(),
        arena.read().term(&copied_string).unwrap(),
        arena.read().term(&copied_symbol).unwrap(),
    ];
    let descriptions = [
        Value::Number(7),
        Value::String("a".into()),
        Value::Symbol("a".into()),
    ];
    for (left, expected_left) in values.iter().zip(&descriptions) {
        for (right, expected_right) in values.iter().zip(&descriptions) {
            assert_eq!(left.cmp(right), expected_left.cmp(expected_right));
            assert_eq!(
                left.compare_terms(*right),
                TermRef::from(expected_left).compare_terms(TermRef::from(expected_right))
            );
        }
    }
}

#[test]
fn input_alias_children_are_registered_only_when_selected() {
    let (owner, _, original, _) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let alias = arena
        .borrow_with(owner.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    let input_child = arena.read().term(&alias).unwrap().child(0).unwrap();
    assert_eq!(
        arena.read().term_key(input_child).unwrap_err(),
        ReadError::ForeignCatalog
    );
    assert_eq!(arena.roots.len(), 1);
    // Reborrow from the input owner so no arena read lives across mutation.
    let child = owner.read().term(&original).unwrap().child(0).unwrap();
    let selected = arena.borrow_with(child, PERMIT).unwrap();
    assert_eq!(
        arena.read().term(&selected).unwrap().descriptor(),
        ValueNodeRef::Number(7)
    );
    assert_eq!(arena.roots.len(), 2);
}

#[test]
fn each_arena_keeps_its_own_identity_scope() {
    let mut left = DerivedTerms::new_with(&[], 1 << 20, PERMIT).unwrap();
    let mut right = DerivedTerms::new_with(&[], 1 << 20, PERMIT).unwrap();
    let own = left
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let foreign = right
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    assert_eq!(own.id, foreign.id);
    assert_eq!(
        left.read().term(&foreign).unwrap_err(),
        ReadError::ForeignCatalog
    );
    assert_eq!(
        left.read().term(&own).unwrap(),
        right.read().term(&foreign).unwrap()
    );
}

#[test]
fn selecting_an_alias_child_registers_only_the_selected_subtree() {
    let (owner, _, original, _) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let root = arena
        .borrow_with(owner.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    let first = arena.child_with(&root, 0, PERMIT).unwrap().unwrap();
    assert_eq!(arena.roots.len(), 2);
    assert!(arena.nodes.kinds.is_empty());
    let repeated = arena.child_with(&root, 1, PERMIT).unwrap().unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(arena.roots.len(), 2);
    assert_eq!(
        arena.read().term(&first).unwrap().descriptor(),
        ValueNodeRef::Number(7)
    );
}

#[test]
fn selecting_a_generated_child_reuses_its_local_identity() {
    let (owner, _, _, shape) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let number = arena
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    let values = assigned(&arena, &number);
    let root = arena
        .construct_with(&shape, values.as_slice(), &[0, 0], logical(), PERMIT)
        .unwrap();
    let child = arena.child_with(&root, 1, PERMIT).unwrap().unwrap();
    assert_eq!(number.id, child.id);
    assert_eq!(arena.roots.len(), 2);
}

#[test]
fn an_absent_alias_child_does_not_register_other_subtrees() {
    let (owner, _, original, _) = fixture();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let root = arena
        .borrow_with(owner.read().term(&original).unwrap(), PERMIT)
        .unwrap();
    assert!(arena.child_with(&root, 2, PERMIT).unwrap().is_none());
    assert_eq!(arena.roots.len(), 1);
}

#[test]
fn child_selection_checks_scope_before_absence() {
    let mut left = DerivedTerms::new_with(&[], 1 << 20, PERMIT).unwrap();
    let mut right = DerivedTerms::new_with(&[], 1 << 20, PERMIT).unwrap();
    let foreign = right
        .scalar_with(ValueNodeRef::Number(7), logical(), PERMIT)
        .unwrap();
    assert!(matches!(
        left.child_with(&foreign, 0, PERMIT),
        Err(DerivedFailure::Read(ReadError::ForeignCatalog))
    ));
}
