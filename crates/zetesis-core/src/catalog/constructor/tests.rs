use super::*;
use crate::atom_interner::{AtomInterner, Failure, Limits};
use crate::{Atom, Predicate, Value};
use std::convert::Infallible;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());
fn limits() -> Limits {
    Limits {
        max_atoms: 8,
        max_bytes: 4 * 1024 * 1024,
    }
}
fn shape() -> ValueNodeRef<'static> {
    ValueNodeRef::Function {
        name: "shared",
        sign: Sign::Negative,
        arity: 3,
    }
}

#[test]
fn constructor_names_share_existing_canonical_text() {
    let mut owner = AtomInterner::new();
    let atom = Atom::new(
        Predicate::new("shared", 2).unwrap(),
        vec![
            Value::Symbol("shared".into()),
            Value::String("shared".into()),
        ],
    )
    .unwrap();
    owner
        .entry_atom_with(&atom, limits(), PERMIT)
        .unwrap()
        .insert_with(limits(), PERMIT)
        .unwrap();
    let bytes = owner.storage_bytes();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap();
    assert_eq!(owner.storage_bytes(), bytes);
    let ValueNodeRef::Function { name, .. } = owner.read().constructor(&declaration).unwrap()
    else {
        panic!("function")
    };
    let atom = owner.get(0).unwrap();
    let ValueNodeRef::Symbol(symbol) = atom.values().at(0).unwrap().descriptor() else {
        panic!("symbol")
    };
    let ValueNodeRef::String(string) = atom.values().at(1).unwrap().descriptor() else {
        panic!("string")
    };
    assert_eq!(name.as_ptr(), atom.predicate().name().as_ptr());
    assert_eq!(name.as_ptr(), symbol.as_ptr());
    assert_eq!(name.as_ptr(), string.as_ptr());
}

#[test]
fn declarations_preserve_constructor_shape_before_term_normalization() {
    let mut owner = AtomInterner::new();
    for expected in [
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Positive,
            arity: 0,
        },
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Negative,
            arity: 0,
        },
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Negative,
            arity: 2,
        },
        ValueNodeRef::Tuple { arity: 0 },
        ValueNodeRef::Tuple { arity: 2 },
    ] {
        let declaration = owner
            .split()
            .1
            .declare_constructor_with(expected, limits(), PERMIT)
            .unwrap();
        assert_eq!(owner.read().constructor(&declaration).unwrap(), expected);
    }
    assert!(owner.is_empty());
}

#[test]
fn declaration_resolution_rejects_foreign_empty_scope() {
    let mut owner = AtomInterner::new();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(ValueNodeRef::Tuple { arity: 0 }, limits(), PERMIT)
        .unwrap();
    let foreign = AtomInterner::new();
    assert_eq!(
        foreign.read().constructor(&declaration),
        Err(ReadError::ForeignCatalog)
    );
}

#[test]
fn declaration_resolution_rejects_older_text_prefix() {
    let mut owner = AtomInterner::new();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap();
    assert_eq!(
        old.read().constructor(&declaration),
        Err(ReadError::OutsidePrefix)
    );
}

#[test]
fn every_declaration_work_cutoff_preserves_retry() {
    let mut owner = AtomInterner::new();
    let mut work = 0;
    owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), || {
            work += 1;
            Ok::<(), usize>(())
        })
        .unwrap();
    for cut in 0..work {
        let mut owner = AtomInterner::new();
        let mut visited = 0;
        let result = owner
            .split()
            .1
            .declare_constructor_with(shape(), limits(), || {
                if visited == cut {
                    return Err(cut);
                }
                visited += 1;
                Ok(())
            });
        assert!(matches!(result, Err(Failure::Stopped(at)) if at == cut));
        assert_eq!(visited, cut);
        let declaration = owner
            .split()
            .1
            .declare_constructor_with(shape(), limits(), PERMIT)
            .unwrap();
        assert_eq!(owner.read().constructor(&declaration).unwrap(), shape());
        assert!(owner.is_empty());
    }
}

#[test]
fn unfunded_constructor_text_refuses_without_discovery() {
    let mut owner = AtomInterner::new();
    let bytes = owner.storage_bytes();
    assert!(matches!(
        owner.split().1.declare_constructor_with(
            shape(),
            Limits {
                max_bytes: bytes,
                ..limits()
            },
            PERMIT
        ),
        Err(Failure::Bytes { .. })
    ));
    assert_eq!(owner.storage_bytes(), bytes);
    assert!(owner.is_empty());
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap();
    assert_eq!(owner.read().constructor(&declaration).unwrap(), shape());
}

#[test]
fn replacing_a_sign_preserves_name_identity_and_arity() {
    let mut owner = AtomInterner::new();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap();
    let changed = declaration.clone().with_sign(Sign::Positive).unwrap();
    let ValueNodeRef::Function { name: original, .. } =
        owner.read().constructor(&declaration).unwrap()
    else {
        panic!("function")
    };
    let ValueNodeRef::Function { name, sign, arity } = owner.read().constructor(&changed).unwrap()
    else {
        panic!("function")
    };
    assert_eq!(name.as_ptr(), original.as_ptr());
    assert_eq!((sign, arity), (Sign::Positive, 3));
}

#[test]
fn replacing_a_sign_preserves_text_prefix_validation() {
    let mut owner = AtomInterner::new();
    let old = owner.publish_selection_with(&[], limits(), PERMIT).unwrap();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap()
        .with_sign(Sign::Positive)
        .unwrap();
    assert_eq!(
        old.read().constructor(&declaration),
        Err(ReadError::OutsidePrefix)
    );
}

#[test]
fn replacing_a_sign_preserves_foreign_scope_refusal() {
    let mut owner = AtomInterner::new();
    let declaration = owner
        .split()
        .1
        .declare_constructor_with(shape(), limits(), PERMIT)
        .unwrap()
        .with_sign(Sign::Positive)
        .unwrap();
    let foreign = AtomInterner::new();
    assert_eq!(
        foreign.read().constructor(&declaration),
        Err(ReadError::ForeignCatalog)
    );
}

#[test]
fn tuple_declarations_have_no_sign_override() {
    let mut owner = AtomInterner::new();
    let tuple = owner
        .split()
        .1
        .declare_constructor_with(ValueNodeRef::Tuple { arity: 2 }, limits(), PERMIT)
        .unwrap();
    assert!(tuple.with_sign(Sign::Negative).is_none());
}
