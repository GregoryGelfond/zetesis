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
