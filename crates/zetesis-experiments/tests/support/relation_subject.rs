//! Canonical typed views must not identify values with different ASP meanings.

use super::TypedValue;
use serde_json::json;
use zetesis_core::{Sign, Value, ValueLimits, ValueNode};

#[test]
fn scalar_subject_values_preserve_their_logical_kinds() {
    for (value, expected) in [
        (Value::Infimum, json!({"kind":"infimum"})),
        (Value::Supremum, json!({"kind":"supremum"})),
        (Value::Number(-1), json!({"kind":"number","value":-1})),
        (
            Value::String("#inf".into()),
            json!({"kind":"string","value":"#inf"}),
        ),
        (
            Value::Symbol("a".into()),
            json!({"kind":"symbol","value":"a"}),
        ),
        (
            Value::String("a".into()),
            json!({"kind":"string","value":"a"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(TypedValue(&value)).unwrap(), expected);
    }
}

#[test]
fn structured_subject_values_preserve_their_complete_shape() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 6,
            },
            ValueNode::Infimum,
            ValueNode::Number(1),
            ValueNode::String("1".into()),
            ValueNode::Symbol("x".into()),
            ValueNode::Tuple { arity: 1 },
            ValueNode::Number(2),
            ValueNode::Supremum,
        ],
        ValueLimits::default(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(TypedValue(&value)).unwrap(),
        json!({
            "kind": "structured",
            "value": [
                {"kind":"function","name":"f","sign":"negative","arity":6},
                {"kind":"infimum"},
                {"kind":"number","number":1},
                {"kind":"string","text":"1"},
                {"kind":"symbol","text":"x"},
                {"kind":"tuple","arity":1},
                {"kind":"number","number":2},
                {"kind":"supremum"}
            ]
        })
    );
}
