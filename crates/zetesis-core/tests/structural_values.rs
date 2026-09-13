//! Closed-value identity, ordering, validation and hostile-depth contracts.
use std::{
    collections::{BTreeSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};
use zetesis_core::{Sign, Value, ValueError, ValueLimits, ValueNode as N, ValueResource};

fn value(nodes: Vec<N>) -> Value {
    Value::from_nodes(nodes, ValueLimits::default()).unwrap()
}
fn fun(name: &str, arity: usize) -> N {
    N::Function {
        name: name.into(),
        sign: Sign::Positive,
        arity,
    }
}
fn render(value: &Value) -> String {
    match value {
        Value::Structured(v) => v.to_string(),
        Value::Symbol(s) => s.clone(),
        _ => panic!("structural fixture"),
    }
}
#[test]
fn identity_normalization_order_and_spelling_are_distinct_contracts() {
    println!(
        "Value={} StructuralValue={} String={} usize={}",
        std::mem::size_of::<Value>(),
        std::mem::size_of::<zetesis_core::StructuralValue>(),
        std::mem::size_of::<String>(),
        std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<Value>(),
        std::mem::size_of::<String>() + std::mem::size_of::<usize>(),
        "structural data must not enlarge the existing scalar enum footprint"
    );
    assert_eq!(value(vec![fun("f", 0)]), Value::Symbol("f".into()));
    let tuple = value(vec![N::Tuple { arity: 0 }]);
    assert_ne!(tuple, Value::Symbol(String::new()));
    assert!(tuple.compare_terms(&Value::Symbol(String::new())).is_lt());
    let nested = value(vec![
        fun("f", 2),
        N::Tuple { arity: 1 },
        N::Number(1),
        fun("g", 1),
        N::String("\"\\\n\t".into()),
    ]);
    assert_eq!(render(&nested), "f((1,),g(\"\\\"\\\\\\n\t\"))");
    if let Value::Structured(v) = &nested {
        assert_eq!(v.rendered_bytes(), v.to_string().len());
    }
    let ordered = [
        Value::Infimum,
        Value::Number(-1),
        tuple,
        Value::Symbol("z".into()),
        value(vec![N::Function {
            name: "a".into(),
            sign: Sign::Negative,
            arity: 0,
        }]),
        Value::String("s".into()),
        value(vec![N::Tuple { arity: 1 }, N::Number(2)]),
        value(vec![fun("f", 1), N::Number(1)]),
        value(vec![fun("f", 1), N::Number(2)]),
        value(vec![fun("f", 2), N::Number(0), N::Number(0)]),
        value(vec![
            N::Function {
                name: "a".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            N::Number(0),
        ]),
        Value::Supremum,
    ];
    for (i, left) in ordered.iter().enumerate() {
        for (j, right) in ordered.iter().enumerate() {
            assert_eq!(left.compare_terms(right), i.cmp(&j));
        }
    }
    assert!(Value::String("s".into()) < Value::Symbol("a".into()));
    assert!(
        Value::String("s".into())
            .compare_terms(&Value::Symbol("a".into()))
            .is_gt()
    );
    assert_eq!(
        ordered.iter().cloned().collect::<BTreeSet<_>>().len(),
        ordered.len()
    );
}

#[test]
fn nested_terms_use_asp_order_at_each_child() {
    // This is the declared ASP order, not derived Value/ValueNode storage order:
    // extrema enclose numbers, positive constants, negative constants, strings
    // and compounds. Equal compound heads compare their complete children.
    let children = [
        vec![N::Infimum],
        vec![N::Number(-7)],
        vec![N::Number(3)],
        vec![N::Tuple { arity: 0 }],
        vec![N::Symbol("a".into())],
        vec![N::Symbol("z".into())],
        vec![N::Function {
            name: "a".into(),
            sign: Sign::Negative,
            arity: 0,
        }],
        vec![N::String("a".into())],
        vec![N::String("é".into())],
        vec![N::Tuple { arity: 1 }, N::Number(0)],
        vec![fun("g", 1), N::Number(0)],
        vec![fun("g", 1), N::Number(1)],
        vec![fun("g", 2), N::Number(-100), N::Number(0)],
        vec![
            N::Function {
                name: "g".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            N::Number(0),
        ],
        vec![N::Supremum],
    ];
    let mut ordered = Vec::new();
    for child in children {
        // The suffix must decide only after an equal whole first child, even
        // when that child spans several preorder nodes.
        for suffix in [-1, 0, 1] {
            let mut nodes = vec![fun("outer", 2)];
            nodes.extend(child.clone());
            nodes.push(N::Number(suffix));
            ordered.push(value(nodes));
        }
    }
    for (left_position, left) in ordered.iter().enumerate() {
        for (right_position, right) in ordered.iter().enumerate() {
            assert_eq!(
                left.compare_terms(right),
                left_position.cmp(&right_position),
                "{left:?} versus {right:?}",
            );
        }
    }
    assert!(
        ordered.windows(2).any(|pair| pair[0] > pair[1]),
        "the specification must distinguish ASP order from storage order"
    );
}

#[test]
fn malformed_trees_and_each_exact_construction_limit_refuse_without_partial_values() {
    for nodes in [
        vec![],
        vec![N::Number(1), N::Number(2)],
        vec![fun("f", 1)],
        vec![fun("", 0)],
        vec![fun("f", usize::MAX)],
    ] {
        assert_eq!(
            Value::from_nodes(nodes, ValueLimits::default()),
            Err(ValueError::Shape)
        );
    }
    let nodes = vec![fun("f", 1), N::Number(2)];
    let bytes = nodes.capacity() * std::mem::size_of::<N>()
        + 1
        + 4
        + 2 * (std::mem::size_of::<usize>() + std::mem::size_of::<(usize, bool, bool)>());
    let exact = ValueLimits {
        max_nodes: 2,
        max_depth: 2,
        max_bytes: bytes,
    };
    let expected = Value::from_nodes(nodes.clone(), exact).unwrap();
    for (limits, resource) in [
        (
            ValueLimits {
                max_nodes: 1,
                ..exact
            },
            ValueResource::Nodes,
        ),
        (
            ValueLimits {
                max_depth: 1,
                ..exact
            },
            ValueResource::Depth,
        ),
        (
            ValueLimits {
                max_bytes: bytes - 1,
                ..exact
            },
            ValueResource::Bytes,
        ),
    ] {
        assert!(
            matches!(Value::from_nodes(nodes.clone(),limits),Err(ValueError::Limit {resource:r,..}) if r==resource)
        );
        assert_eq!(Value::from_nodes(nodes.clone(), exact).unwrap(), expected);
    }
}
#[test]
fn deep_values_clone_hash_compare_render_and_drop_without_recursive_walks() {
    let depth = 10_000;
    let mut nodes = vec![fun("f", 1); depth];
    nodes.push(N::Number(0));
    let limits = ValueLimits {
        max_nodes: depth + 1,
        max_depth: depth + 1,
        max_bytes: 2_000_000,
    };
    let original = Value::from_nodes(nodes, limits).unwrap();
    let cloned = original.clone();
    assert_eq!(original, cloned);
    assert_eq!(original.cmp(&cloned), std::cmp::Ordering::Equal);
    assert_eq!(original.compare_terms(&cloned), std::cmp::Ordering::Equal);
    let mut first = DefaultHasher::new();
    let mut second = DefaultHasher::new();
    original.hash(&mut first);
    cloned.hash(&mut second);
    assert_eq!(first.finish(), second.finish());
    let Value::Structured(v) = &original else {
        panic!("compound")
    };
    assert_eq!(v.depth(), depth + 1);
    assert_eq!(v.to_string().len(), depth * 3 + 1);
    assert_eq!(v.rendered_bytes(), v.to_string().len());
    drop(original);
    drop(cloned);
}

#[test]
fn spelling_cache_is_canonical_at_numeric_and_nullary_boundaries() {
    for number in [i32::MIN, -1_000_000, -10, -1, 0, 1, 10, 1_000_000, i32::MAX] {
        let v = value(vec![fun("f", 1), N::Number(number)]);
        let Value::Structured(v) = v else {
            panic!("structure")
        };
        assert_eq!(v.to_string(), format!("f({number})"));
        assert_eq!(v.rendered_bytes(), v.to_string().len());
    }
    for node in [
        N::Tuple { arity: 0 },
        N::Function {
            name: "f".into(),
            sign: Sign::Negative,
            arity: 0,
        },
    ] {
        let first = value(vec![node.clone()]);
        let mut reserved = Vec::with_capacity(128);
        reserved.push(node);
        assert_eq!(first, value(reserved));
        assert!(matches!(
            Value::from_nodes(
                vec![N::Tuple { arity: 0 }],
                ValueLimits {
                    max_depth: 0,
                    ..ValueLimits::default()
                }
            ),
            Err(ValueError::Limit {
                resource: ValueResource::Depth,
                ..
            })
        ));
    }
}
