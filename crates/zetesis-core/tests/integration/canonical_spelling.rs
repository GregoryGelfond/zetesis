//! Borrowed spelling shares the admitted value renderer without payload export.

use std::{convert::Infallible, fmt};

use zetesis_core::{
    Atom, AtomCatalog, Predicate, Sign, Value, ValueError, ValueLimits, ValueNode, ValueResource,
    ValueWriteError, catalog::TermRef,
};

fn catalog(value: Value) -> AtomCatalog {
    AtomCatalog::new(vec![
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap(),
    ])
    .unwrap()
}
fn complex() -> Value {
    Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 5,
            },
            ValueNode::String("a\"\\\nλ".into()),
            ValueNode::Tuple { arity: 1 },
            ValueNode::Number(i32::MIN),
            ValueNode::Tuple { arity: 0 },
            ValueNode::Function {
                name: "z".into(),
                sign: Sign::Negative,
                arity: 0,
            },
            ValueNode::Tuple { arity: 2 },
            ValueNode::Infimum,
            ValueNode::Supremum,
        ],
        ValueLimits::default(),
    )
    .unwrap()
}

#[test]
fn borrowed_spelling_matches_canonical_punctuation() {
    let value = complex();
    let owner = catalog(value.clone());
    let canonical = owner.atoms().at(0).unwrap().values().at(0).unwrap();
    let expected = "-f(\"a\\\"\\\\\\nλ\",(-2147483648,),(),-z,(#inf,#sup))";
    assert_eq!(canonical.to_string(), expected);
    assert_eq!(TermRef::from(&value).to_string(), expected);
    let Value::Structured(structured) = value else {
        panic!("compound fixture");
    };
    assert_eq!(structured.to_string(), expected);
}

#[test]
fn every_spelling_work_cutoff_preserves_the_cause() {
    let owner = catalog(complex());
    let value = owner.atoms().at(0).unwrap().values().at(0).unwrap();
    let mut expected = String::new();
    let mut total = 0;
    value
        .write_with(&mut expected, usize::MAX, |_| {
            total += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for cutoff in 0..total {
        let cause = ("write", cutoff);
        let mut accepted = 0;
        let mut output = String::new();
        let result = value.write_with(&mut output, usize::MAX, |_| {
            if accepted == cutoff {
                Err(&cause)
            } else {
                accepted += 1;
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(ValueWriteError::Stopped(actual)) if std::ptr::eq(actual, &raw const cause))
        );
        assert_eq!(accepted, cutoff);
        assert!(expected.starts_with(&output));
    }
}

#[test]
fn constructor_spelling_refuses_unadmitted_frames() {
    let owner = catalog(complex());
    let value = owner.atoms().at(0).unwrap().values().at(0).unwrap();
    let mut output = String::new();
    let result = value.write_with(&mut output, 0, |_| Ok::<_, Infallible>(()));
    assert!(matches!(
        result,
        Err(ValueWriteError::Storage(ValueError::Limit {
            resource: ValueResource::Bytes,
            limit: 0,
            ..
        }))
    ));
    assert!(output.is_empty());
}

#[test]
fn spelling_preserves_sink_refusal() {
    struct Refuse;
    impl fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    let value = Value::Number(42);
    assert!(matches!(
        TermRef::from(&value).write_with(&mut Refuse, 0, |_| Ok::<_, Infallible>(())),
        Err(ValueWriteError::Writer(_))
    ));
}

#[test]
fn deep_spelling_uses_bounded_iterative_frames() {
    let depth = 10_000;
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_nodes: depth + 1,
            max_depth: depth + 1,
            max_bytes: 16_777_216,
        },
    )
    .unwrap();
    let owner = catalog(value);
    let term = owner.atoms().at(0).unwrap().values().at(0).unwrap();
    let mut output = String::new();
    term.write_with(&mut output, 1_048_576, |_| Ok::<_, Infallible>(()))
        .unwrap();
    assert_eq!(output.len(), 3 * depth + 1);
    assert!(output.starts_with(&"(".repeat(depth)));
    assert!(output.ends_with(&",)".repeat(depth)));
    assert_eq!(output.as_bytes()[depth], b'7');
}
