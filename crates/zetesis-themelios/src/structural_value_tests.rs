//! Shared validation and construction preserve logical limits independently of storage.

use super::*;

fn function(argument: Symbol) -> Symbol {
    Symbol::Function {
        name: Name::new("f").unwrap(),
        arguments: vec![argument],
        sign: Sign::Positive,
    }
}

#[test]
fn validation_preserves_the_exact_node_limit() {
    let symbol = function(function(Symbol::Number(1)));
    let limits = ValueLimits {
        max_nodes: 3,
        ..ValueLimits::default()
    };
    assert!(traverse(&symbol, limits, Validation::default()).is_ok());
    let limits = ValueLimits {
        max_nodes: 2,
        ..limits
    };
    let validation = traverse(&symbol, limits, Validation::default()).unwrap_err();
    let construction = traverse(&symbol, limits, Construction::default()).unwrap_err();
    assert_eq!(validation, construction);
    assert!(matches!(
        validation,
        ValueError::Limit {
            resource: ValueResource::Nodes,
            observed: 3,
            ..
        }
    ));
}

#[test]
fn validation_preserves_the_exact_depth_limit() {
    let symbol = function(function(Symbol::Number(1)));
    let limits = ValueLimits {
        max_depth: 3,
        ..ValueLimits::default()
    };
    assert!(traverse(&symbol, limits, Validation::default()).is_ok());
    let limits = ValueLimits {
        max_depth: 2,
        ..limits
    };
    let validation = traverse(&symbol, limits, Validation::default()).unwrap_err();
    let construction = traverse(&symbol, limits, Construction::default()).unwrap_err();
    assert_eq!(validation, construction);
    assert!(matches!(
        validation,
        ValueError::Limit {
            resource: ValueResource::Depth,
            observed: 3,
            ..
        }
    ));
}

#[test]
fn validation_preserves_the_exact_logical_text_limit() {
    let symbol = function(Symbol::String("λ\n\"\\".into()));
    let logical_bytes = symbol
        .subsymbols()
        .map(|symbol| {
            let node = view(symbol);
            std::mem::size_of::<ValueNode>() as u128
                + node.text_bytes() as u128
                + node.rendered_bytes()
        })
        .sum::<u128>();
    let limits = ValueLimits {
        max_bytes: usize::try_from(logical_bytes).unwrap(),
        ..ValueLimits::default()
    };
    assert!(traverse(&symbol, limits, Validation::default()).is_ok());
    let error = traverse(
        &symbol,
        ValueLimits {
            max_bytes: limits.max_bytes - 1,
            ..limits
        },
        Validation::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        ValueError::Limit {
            resource: ValueResource::Bytes,
            observed: logical_bytes,
            limit: limits.max_bytes - 1
        }
    );
}

#[test]
fn validation_does_not_require_render_storage() {
    let symbol = function(Symbol::Number(1));
    // f(1) has two node cells, one referenced name byte and four spelling
    // bytes. A real construction also needs validation/render frame capacity.
    let limits = ValueLimits {
        max_bytes: 2 * std::mem::size_of::<ValueNode>() + 1 + 4,
        ..ValueLimits::default()
    };
    assert!(traverse(&symbol, limits, Validation::default()).is_ok());
    assert!(matches!(
        traverse(&symbol, limits, Construction::default()),
        Err(ValueError::Limit {
            resource: ValueResource::Bytes,
            ..
        })
    ));
}

#[test]
fn validation_retains_nested_nul_refusal() {
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Location, Span},
    };
    let location = Location {
        source: SourceId::new(11),
        span: Span::empty(ByteOffset::new(0)),
    };
    let symbol = function(Symbol::String("invalid\0text".into()));
    for error in [
        crate::compile::validate_scalar(&symbol, location).unwrap_err(),
        crate::compile::scalar(&symbol, location).unwrap_err(),
    ] {
        assert!(
            matches!(error, crate::AdmissionFailure::Profile { feature: crate::ProfileFeature::NulString, location: actual } if actual == location)
        );
    }
}

#[test]
fn validation_keeps_typed_constructor_boundaries() {
    let symbols = [
        Symbol::Tuple(vec![]),
        Symbol::Tuple(vec![Symbol::Number(1)]),
        Symbol::Function {
            name: Name::new("f").unwrap(),
            arguments: vec![],
            sign: Sign::Negative,
        },
        function(Symbol::Tuple(vec![Symbol::Infimum, Symbol::Supremum])),
    ];
    for symbol in &symbols {
        validate_symbol(symbol).unwrap();
        let Value::Structured(value) = from_symbol(symbol).unwrap() else {
            panic!("structural fixture")
        };
        assert_eq!(to_symbol(&value).unwrap(), *symbol);
    }
}
