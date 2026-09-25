use super::*;
use crate::{Sign, ValueNodeRef};
use std::convert::Infallible;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

#[test]
fn declaring_a_function_adds_only_its_text() {
    let mut store = Store::new(1024 * 1024);
    store
        .declare_constructor_with(
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 3,
            },
            PERMIT,
        )
        .unwrap();
    let counts = store.counts();
    assert_eq!(counts.texts, 1);
    assert_eq!(counts.terms, 0);
    assert_eq!(counts.predicates, 0);
    assert_eq!(counts.atoms, 0);
}

#[test]
fn declaring_a_tuple_adds_no_canonical_payload() {
    let mut store = Store::new(1024 * 1024);
    let bytes = store.current_bytes();
    store
        .declare_constructor_with(ValueNodeRef::Tuple { arity: usize::MAX }, PERMIT)
        .unwrap();
    let counts = store.counts();
    assert_eq!(counts.texts, 0);
    assert_eq!(counts.terms, 0);
    assert_eq!(counts.predicates, 0);
    assert_eq!(counts.atoms, 0);
    assert_eq!(store.current_bytes(), bytes);
}

#[test]
fn nonconstructor_descriptors_do_not_import_payload() {
    let mut store = Store::new(1024 * 1024);
    for descriptor in [
        ValueNodeRef::Function {
            name: "",
            sign: Sign::Positive,
            arity: 1,
        },
        ValueNodeRef::Number(0),
        ValueNodeRef::String("not a constructor"),
        ValueNodeRef::Symbol("not a constructor"),
    ] {
        assert!(matches!(
            store.declare_constructor_with(descriptor, PERMIT),
            Err(Failure::Storage(Fault::Shape))
        ));
    }
    assert_eq!(store.counts().texts, 0);
}
