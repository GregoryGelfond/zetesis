//! Borrowed node descriptions retain typed identity and spelling measures.

use proptest::prelude::*;
use zetesis_core::{Sign, Value, ValueLimits, ValueNode, ValueNodeRef};

#[test]
fn node_views_preserve_complete_typed_descriptions() {
    let nodes = [
        ValueNode::Infimum,
        ValueNode::Supremum,
        ValueNode::Number(i32::MIN),
        ValueNode::String("x".into()),
        ValueNode::Symbol("x".into()),
        ValueNode::Function {
            name: "x".into(),
            sign: Sign::Positive,
            arity: 0,
        },
        ValueNode::Function {
            name: "x".into(),
            sign: Sign::Negative,
            arity: 0,
        },
        ValueNode::Function {
            name: "x".into(),
            sign: Sign::Positive,
            arity: 1,
        },
        ValueNode::Tuple { arity: 0 },
        ValueNode::Tuple { arity: 1 },
    ];
    for (index, node) in nodes.iter().enumerate() {
        assert_eq!(node.view().into_owned(), *node);
        for (other_index, other) in nodes.iter().enumerate() {
            assert_eq!(node.view() == other.view(), index == other_index);
        }
    }
}

#[test]
fn node_views_borrow_the_original_text() {
    let node = ValueNode::String("λ雪\n\"\\".into());
    let ValueNode::String(text) = &node else {
        unreachable!()
    };
    let ValueNodeRef::String(borrowed) = node.view() else {
        panic!("string view")
    };
    assert_eq!(text.as_ptr(), borrowed.as_ptr());
    assert_eq!(node.view().text_bytes(), text.len());
}

#[test]
fn node_storage_order_uses_typed_descriptors() {
    // Raw node descriptions keep positive nullary functions distinct from
    // symbols. Function storage order is name, sign, arity, unlike ASP order.
    let ordered = [
        ValueNode::Infimum,
        ValueNode::Number(i32::MIN),
        ValueNode::Number(i32::MAX),
        ValueNode::String("z".into()),
        ValueNode::Symbol("a".into()),
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity: 0,
        },
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity: 2,
        },
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Negative,
            arity: 0,
        },
        ValueNode::Function {
            name: "g".into(),
            sign: Sign::Positive,
            arity: 0,
        },
        ValueNode::Tuple { arity: 0 },
        ValueNode::Tuple { arity: 1 },
        ValueNode::Supremum,
    ];
    for (left_position, left) in ordered.iter().enumerate() {
        for (right_position, right) in ordered.iter().enumerate() {
            assert_eq!(left.cmp(right), left_position.cmp(&right_position));
        }
    }
}

#[test]
fn node_encoding_measures_include_typed_fields() {
    for (node, bytes) in [
        (ValueNodeRef::Infimum, 1),
        (ValueNodeRef::Supremum, 1),
        (ValueNodeRef::Number(i32::MIN), 5),
        (ValueNodeRef::String(""), 9),
        (ValueNodeRef::String("é"), 11),
        (ValueNodeRef::Symbol("é"), 11),
        (
            ValueNodeRef::Function {
                name: "é",
                sign: Sign::Negative,
                arity: 2,
            },
            20,
        ),
        (ValueNodeRef::Tuple { arity: 0 }, 9),
        (ValueNodeRef::Tuple { arity: 2 }, 9),
    ] {
        assert_eq!(node.canonical_bytes(), bytes);
    }
}

proptest! {
    #[test]
    fn borrowed_spelling_measures_match_construction(
        characters in prop::collection::vec(prop::sample::select(vec!['a', 'λ', '雪', '\n', '\r', '\t', '\\', '"']), 0..24),
        number in any::<i32>(),
        negative in any::<bool>(),
        depth in 0..8usize,
    ) {
        let mut nodes = vec![ValueNode::Function {
            name: "f".into(), sign: if negative { Sign::Negative } else { Sign::Positive }, arity: 5,
        }];
        nodes.extend(std::iter::repeat_n(ValueNode::Tuple { arity: 1 }, depth));
        nodes.push(ValueNode::String(characters.into_iter().collect()));
        nodes.push(ValueNode::Infimum);
        nodes.push(ValueNode::Supremum);
        nodes.push(ValueNode::Function { name: "g".into(), sign: Sign::Negative, arity: 0 });
        nodes.push(ValueNode::Number(number));
        let measured = nodes.iter().map(|node| node.view().rendered_bytes()).sum::<u128>();
        let Value::Structured(value) = Value::from_nodes(nodes, ValueLimits::default()).unwrap() else {
            panic!("compound fixture")
        };
        prop_assert_eq!(measured, value.rendered_bytes() as u128);
        prop_assert_eq!(measured, value.to_string().len() as u128);
    }
}
