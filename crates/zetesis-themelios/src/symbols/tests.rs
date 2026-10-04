use super::{Error, Failure, atom_with, term_with};
use themelios_program::{Name, Sign as SymbolSign, Symbol};
use zetesis_core::catalog::TermRef;
use zetesis_core::{Atom, AtomCatalog, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::{Cancellation, Stop};

fn fixture() -> (AtomCatalog, Symbol) {
    let structured = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                arity: 2,
                sign: Sign::Negative,
            },
            ValueNode::Tuple { arity: 2 },
            ValueNode::Number(3),
            ValueNode::String("λ\n\"\\".into()),
            ValueNode::Symbol("a".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let values = vec![
        Value::Number(i32::MIN),
        Value::Number(i32::MAX),
        Value::Infimum,
        Value::Supremum,
        Value::String("text".into()),
        Value::Symbol("a".into()),
        structured,
    ];
    let expected = Symbol::Function {
        name: Name::new("p").unwrap(),
        sign: SymbolSign::Negative,
        arguments: vec![
            Symbol::Number(i32::MIN),
            Symbol::Number(i32::MAX),
            Symbol::Infimum,
            Symbol::Supremum,
            Symbol::String("text".into()),
            Symbol::Function {
                name: Name::new("a").unwrap(),
                sign: SymbolSign::Positive,
                arguments: vec![],
            },
            Symbol::Function {
                name: Name::new("f").unwrap(),
                sign: SymbolSign::Negative,
                arguments: vec![
                    Symbol::Tuple(vec![Symbol::Number(3), Symbol::String("λ\n\"\\".into())]),
                    Symbol::Function {
                        name: Name::new("a").unwrap(),
                        sign: SymbolSign::Positive,
                        arguments: vec![],
                    },
                ],
            },
        ],
    };
    let atom = Atom::new(
        Predicate::with_sign("p", values.len(), Sign::Negative).unwrap(),
        values,
    )
    .unwrap();
    (AtomCatalog::new(vec![atom]).unwrap(), expected)
}

#[test]
fn canonical_atom_export_preserves_signed_values() {
    let (catalog, expected) = fixture();
    let atom = catalog.atoms().at(0).unwrap();
    let actual = atom_with(atom, 8192, || Ok::<_, Stop>(())).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn canonical_term_export_preserves_nested_values() {
    let (catalog, expected) = fixture();
    let value = catalog.atoms().at(0).unwrap().arguments().at(6).unwrap();
    let actual = term_with(value, 8192, || Ok::<_, Stop>(())).unwrap();
    assert_eq!(&actual, &expected.arguments()[6]);
}

#[test]
fn nullary_atom_export_preserves_its_sign() {
    let atom = Atom::new(
        Predicate::with_sign("p", 0, Sign::Negative).unwrap(),
        vec![],
    )
    .unwrap();
    assert_eq!(
        atom_with((&atom).into(), 1024, || Ok::<_, Stop>(())).unwrap(),
        Symbol::Function {
            name: Name::new("p").unwrap(),
            sign: SymbolSign::Negative,
            arguments: vec![],
        },
    );
}

#[test]
fn atom_storage_is_shared_across_arguments() {
    let atom = Atom::new(
        Predicate::new("p", 32).unwrap(),
        vec![Value::String("x".repeat(64)); 32],
    )
    .unwrap();
    let catalog = AtomCatalog::new(vec![atom]).unwrap();
    let atom = catalog.atoms().at(0).unwrap();
    let limit = 3 * size_of::<Symbol>() as u128 + 256;
    for value in atom.arguments() {
        term_with(value, limit, || Ok::<_, Stop>(())).unwrap();
    }
    assert!(matches!(
        atom_with(atom, limit, || Ok::<_, Stop>(())),
        Err(Failure::Bridge(Error::Storage { required, limit: ceiling }))
            if required > ceiling && ceiling == limit,
    ));
}

#[test]
fn atom_export_preserves_every_control_refusal() {
    let (catalog, expected) = fixture();
    let atom = catalog.atoms().at(0).unwrap();
    let mut permits = 0;
    atom_with(atom, 8192, || {
        permits += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    for cutoff in 0..permits {
        let mut used = 0;
        let result = atom_with(atom, 8192, || {
            if used == cutoff {
                Err(cutoff)
            } else {
                used += 1;
                Ok(())
            }
        });
        assert!(matches!(result, Err(Failure::Stopped(stop)) if stop == cutoff));
        assert_eq!(used, cutoff);
    }
    assert_eq!(
        atom_with(atom, 8192, || Ok::<_, Stop>(())).unwrap(),
        expected,
    );
}

#[test]
fn cancelled_export_returns_the_typed_stop() {
    let (catalog, _) = fixture();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        atom_with(catalog.atoms().at(0).unwrap(), 8192, || cancellation.poll()),
        Err(Failure::Stopped(Stop::Cancelled)),
    ));
}

#[test]
fn expired_export_returns_the_typed_stop() {
    let (catalog, _) = fixture();
    let cancellation = Cancellation::with_deadline(std::time::Instant::now()).unwrap();
    assert!(matches!(
        atom_with(catalog.atoms().at(0).unwrap(), 8192, || cancellation.poll()),
        Err(Failure::Stopped(Stop::Deadline)),
    ));
}

#[test]
fn unspellable_predicate_is_a_typed_refusal() {
    let atom = Atom::new(Predicate::new("bad name", 0).unwrap(), vec![]).unwrap();
    assert!(matches!(
        atom_with((&atom).into(), 1024, || Ok::<_, Stop>(())),
        Err(Failure::Bridge(Error::InvalidName)),
    ));
}

#[test]
fn ingress_subtree_export_stops_during_depth_scan() {
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; 33];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    let subtree = TermRef::from(&value).child(0).unwrap();
    let mut accepted = 0;
    // The frame allowance cannot hold this subtree. The work limit is met
    // during depth measurement, before the constructor can request frames.
    // An unchecked depth scan would instead return Storage after one permit.
    let result = term_with(subtree, size_of::<Symbol>() as u128, || {
        if accepted == 8 {
            return Err(Stop::Cancelled);
        }
        accepted += 1;
        Ok(())
    });
    assert!(matches!(result, Err(Failure::Stopped(Stop::Cancelled))));
    assert_eq!(accepted, 8);
}

#[test]
fn wide_ingress_subtree_keeps_its_tight_storage_bound() {
    let mut nodes = vec![
        ValueNode::Tuple { arity: 1 },
        ValueNode::Tuple { arity: 64 },
    ];
    nodes.extend((0..64).map(ValueNode::Number));
    let value = Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    let subtree = TermRef::from(&value).child(0).unwrap();
    let limit = 66 * size_of::<Symbol>() as u128;
    let expected = Symbol::Tuple((0..64).map(Symbol::Number).collect());
    assert_eq!(
        term_with(subtree, limit, || Ok::<_, Stop>(())).unwrap(),
        expected,
    );
    assert!(matches!(
        term_with(subtree, size_of::<Symbol>() as u128, || Ok::<_, Stop>(())),
        Err(Failure::Bridge(Error::Storage { required, limit })) if required > limit,
    ));
}
