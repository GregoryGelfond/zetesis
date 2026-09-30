use super::super::super::{budget::Budget, index::Index};
use super::*;
use crate::test_support::PERMIT_WITH_UNIT_ERROR as PERMIT;
use crate::{
    Sign, Value, ValueLimits, ValueNode,
    catalog::{Vocabulary, VocabularyBuilder},
};

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

fn collision_inputs() -> (Vocabulary, Vec<TermKey>, DeclaredConstructor) {
    let logical = Limits::default();
    let mut builder = VocabularyBuilder::new(1 << 20).unwrap();
    let mut inputs = Vec::new();
    for suffix in ["a", "b"] {
        let value = Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f".into(),
                    sign: Sign::Positive,
                    arity: 1,
                },
                ValueNode::String(format!("{}{suffix}", "shared".repeat(1024))),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        inputs.push(
            builder
                .import_term_with((&value).into(), logical, PERMIT)
                .unwrap(),
        );
    }
    let shape = builder
        .declare_constructor_with(ValueNodeRef::Tuple { arity: 1 }, PERMIT)
        .unwrap();
    (builder.finish_with(0, PERMIT).unwrap(), inputs, shape)
}

#[test]
fn compound_collision_children_use_scoped_equality() {
    let (owner, inputs, shape) = collision_inputs();
    let mut arena = DerivedTerms::new_with(&[owner.read()], 1 << 20, PERMIT).unwrap();
    let mut candidates = Vec::new();
    for input in &inputs {
        let child = arena
            .borrow_with(owner.read().term(input).unwrap(), PERMIT)
            .unwrap();
        let mut assignment = arena.read().assignment();
        assignment.resize_with(1, 1 << 20, PERMIT).unwrap();
        assignment.set_with(0, &child, PERMIT).unwrap();
        candidates.push(
            arena
                .construct_with(
                    &shape,
                    assignment.as_slice(),
                    &[0],
                    Limits::default(),
                    PERMIT,
                )
                .unwrap(),
        );
    }
    // The last real construction retains b as the candidate child buffer. The
    // collision predicate must reject tuple(a) without traversing either long
    // child and accept tuple(b) by the same checked identity operation.
    let before_roots = arena.roots.len();
    for (candidate, expected) in [(&candidates[0], false), (&candidates[1], true)] {
        let mut calls = 0;
        let mut before = || {
            calls += 1;
            if calls <= 4 {
                Ok(())
            } else {
                Err("child payload was visited")
            }
        };
        assert_eq!(
            arena
                .equal(
                    candidate.id,
                    ValueNodeRef::Tuple { arity: 1 },
                    &mut Work::new(&mut before)
                )
                .unwrap(),
            expected
        );
        // Root resolution, descriptor, child resolution, then scoped equality.
        assert_eq!(calls, 4);
        for cutoff in 0..calls {
            let mut accepted = 0;
            let mut before = || {
                if accepted == cutoff {
                    return Err("stop");
                }
                accepted += 1;
                Ok(())
            };
            assert!(matches!(
                arena.equal(
                    candidate.id,
                    ValueNodeRef::Tuple { arity: 1 },
                    &mut Work::new(&mut before)
                ),
                Err(Failure::Stopped("stop"))
            ));
            assert_eq!(accepted, cutoff);
            assert_eq!(arena.roots.len(), before_roots);
        }
    }
}
