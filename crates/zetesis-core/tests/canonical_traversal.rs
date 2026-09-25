//! Logical traversal and order do not depend on the payload's owner.

use std::convert::Infallible;
use zetesis_core::catalog::TermRef;
use zetesis_core::{Atom, AtomCatalog, Predicate, Sign, Value, ValueLimits, ValueNode as N};

fn function(name: &str, arity: usize) -> N {
    N::Function {
        name: name.into(),
        sign: Sign::Positive,
        arity,
    }
}
fn tree(nodes: Vec<N>) -> Value {
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}
fn catalog(value: Value) -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap(),
    ])
    .unwrap()
}
fn term(catalog: &AtomCatalog) -> TermRef<'_> {
    catalog.atoms().at(0).unwrap().values().at(0).unwrap()
}
fn before(remaining: &mut usize) -> Result<(), ()> {
    if *remaining == 0 {
        return Err(());
    }
    *remaining -= 1;
    Ok(())
}
fn nested() -> Value {
    tree(vec![
        function("f", 3),
        function("g", 2),
        N::String("same".into()),
        N::String("same".into()),
        N::Tuple { arity: 1 },
        N::Number(-2),
        function("g", 2),
        N::String("same".into()),
        N::String("same".into()),
    ])
}

#[test]
fn canonical_preorder_preserves_repeated_occurrences() {
    let source = nested();
    let admitted = catalog(source.clone());
    assert_eq!(
        term(&admitted).nodes().collect::<Vec<_>>(),
        TermRef::from(&source).nodes().collect::<Vec<_>>()
    );
}

#[test]
fn refused_preorder_step_preserves_the_cursor() {
    let source = nested();
    let admitted = catalog(source.clone());
    for value in [TermRef::from(&source), term(&admitted)] {
        let expected = value.nodes().collect::<Vec<_>>();
        let mut total = 0;
        let mut cursor = value.nodes();
        while cursor
            .next_with(|| {
                total += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap()
            .is_some()
        {}
        for limit in 0..total {
            let mut cursor = value.nodes();
            let mut remaining = limit;
            let mut prefix = Vec::new();
            loop {
                match cursor.next_with(|| before(&mut remaining)) {
                    Ok(Some(node)) => prefix.push(node),
                    Err(()) => break,
                    Ok(None) => panic!("a short allowance cannot complete"),
                }
            }
            assert_eq!(prefix, expected[..prefix.len()]);
            assert_eq!(cursor.collect::<Vec<_>>(), expected[prefix.len()..]);
        }
    }
}

#[test]
fn checked_comparison_obeys_asp_term_order() {
    let values = [
        Value::Infimum,
        Value::Number(-1),
        tree(vec![N::Tuple { arity: 0 }]),
        Value::Symbol("z".into()),
        tree(vec![N::Function {
            name: "a".into(),
            sign: Sign::Negative,
            arity: 0,
        }]),
        Value::String("a".into()),
        tree(vec![N::Tuple { arity: 1 }, N::Number(2)]),
        tree(vec![function("f", 1), N::Number(1)]),
        tree(vec![function("f", 1), N::Number(2)]),
        tree(vec![function("f", 2), N::Number(0), N::Number(0)]),
        Value::Supremum,
    ];
    let owners: Vec<_> = values.iter().cloned().map(catalog).collect();
    for (i, left) in values.iter().enumerate() {
        for (j, right) in values.iter().enumerate() {
            for (left, right) in [
                (TermRef::from(left), term(&owners[j])),
                (term(&owners[i]), TermRef::from(right)),
                (term(&owners[i]), term(&owners[j])),
            ] {
                assert_eq!(
                    left.compare_terms_with(right, || Ok::<_, Infallible>(()))
                        .unwrap(),
                    i.cmp(&j)
                );
            }
        }
    }
}

#[test]
fn checked_term_order_honors_each_refusal() {
    let left = catalog(nested());
    let right = catalog(nested());
    let mut total = 0;
    term(&left)
        .compare_terms_with(term(&right), || {
            total += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    for limit in 0..total {
        let mut remaining = limit;
        assert_eq!(
            term(&left).compare_terms_with(term(&right), || before(&mut remaining)),
            Err(())
        );
    }
}

#[test]
fn preorder_selection_borrows_each_complete_subterm() {
    let source = nested();
    let admitted = catalog(source.clone());
    let expected = [
        "f(g(\"same\",\"same\"),(-2,),g(\"same\",\"same\"))",
        "g(\"same\",\"same\")",
        "\"same\"",
        "\"same\"",
        "(-2,)",
        "-2",
        "g(\"same\",\"same\")",
        "\"same\"",
        "\"same\"",
    ];
    for value in [TermRef::from(&source), term(&admitted)] {
        for (rank, spelling) in expected.iter().enumerate() {
            let selected = value
                .subterm_with(rank, || Ok::<_, Infallible>(()))
                .unwrap()
                .unwrap();
            assert_eq!(selected.to_string(), *spelling);
        }
        assert!(
            value
                .subterm_with(expected.len(), || Ok::<_, Infallible>(()))
                .unwrap()
                .is_none()
        );
        assert!(
            value
                .subterm_with(usize::MAX, || Ok::<_, Infallible>(()))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn preorder_selection_preserves_every_work_refusal() {
    let source = nested();
    let admitted = catalog(source.clone());
    for value in [TermRef::from(&source), term(&admitted)] {
        for rank in 0..value.expanded_nodes() {
            let mut total = 0;
            let expected = value
                .subterm_with(rank, || {
                    total += 1;
                    Ok::<_, Infallible>(())
                })
                .unwrap();
            for limit in 0..total {
                let mut remaining = limit;
                assert_eq!(value.subterm_with(rank, || before(&mut remaining)), Err(()));
            }
            let mut remaining = total;
            assert_eq!(
                value.subterm_with(rank, || before(&mut remaining)).unwrap(),
                expected
            );
        }
    }
}
