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
            matches!(error, crate::AdmissionFailure::Profile { feature: crate::ProfileFeature::NulString, location: actual } if actual.location() == Some(location))
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

fn deep_tuple_catalog(parents: usize) -> (zetesis_core::AtomCatalog, Symbol) {
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; parents];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    let mut expected = Symbol::Number(7);
    for _ in 0..parents {
        expected = Symbol::Tuple(vec![expected]);
    }
    let catalog = zetesis_core::AtomCatalog::new(vec![
        zetesis_core::Atom::new(zetesis_core::Predicate::new("p", 1).unwrap(), vec![value])
            .unwrap(),
    ])
    .unwrap();
    (catalog, expected)
}

#[test]
fn deep_export_retains_exact_frames_within_the_advertised_preflight() {
    // The previous geometric stack had capacity 64 at this depth, on top of
    // 33 already reserved Symbol child cells. Count actual returned capacities
    // and the conversion receipt, rather than only repeating a limit formula.
    let (catalog, expected) = deep_tuple_catalog(33);
    let value = catalog.atoms().at(0).unwrap().values().get(0).unwrap();
    let limit = 2 * value.expanded_nodes() as u128 * size_of::<Symbol>() as u128;
    let mut storage = ExportStorage::new(limit).unwrap();
    let mut nodes = value.nodes();
    let actual = from_nodes(
        value.expanded_nodes(),
        value.depth(),
        &mut storage,
        |before| nodes.next_with(before),
        || Ok::<_, ()>(()),
    )
    .unwrap();
    assert_eq!(actual, expected);
    let output_cells = 1 + actual
        .subsymbols()
        .map(|symbol| match symbol {
            Symbol::Function { arguments, .. } | Symbol::Tuple(arguments) => arguments.capacity(),
            _ => 0,
        })
        .sum::<usize>();
    let scratch_bytes = storage.retained - output_cells as u128 * size_of::<Symbol>() as u128;
    assert_eq!(scratch_bytes, 33 * size_of::<Frame<'_>>() as u128);
    assert!(storage.retained <= limit);
    assert_eq!(
        term_symbol_with(value, storage.retained, || Ok::<_, ()>(())).unwrap(),
        expected
    );
    assert!(matches!(
        term_symbol_with(value, storage.retained - 1, || Ok::<_, ()>(())),
        Err(BridgeFailure::Bridge(BridgeError::Storage { required, limit }))
            if required > limit && limit == storage.retained - 1,
    ));
}

#[test]
fn checked_export_refuses_every_stopped_prefix_without_mutating_the_input() {
    let (catalog, expected) = deep_tuple_catalog(33);
    let value = catalog.atoms().at(0).unwrap().values().get(0).unwrap();
    let limit = 2 * value.expanded_nodes() as u128 * size_of::<Symbol>() as u128;
    let mut permits = 0;
    assert_eq!(
        term_symbol_with(value, limit, || {
            permits += 1;
            Ok::<_, usize>(())
        })
        .unwrap(),
        expected
    );
    for cutoff in 0..permits {
        let mut used = 0;
        assert!(matches!(
            term_symbol_with(value, limit, || {
                if used == cutoff { Err(cutoff) } else { used += 1; Ok(()) }
            }),
            Err(BridgeFailure::Stopped(stopped)) if stopped == cutoff,
        ));
        assert_eq!(used, cutoff);
    }
    assert_eq!(
        term_symbol_with(value, limit, || Ok::<_, usize>(())).unwrap(),
        expected
    );
}

#[test]
fn export_frames_follow_depth_instead_of_wide_node_population() {
    let symbols = (0..64).map(Symbol::Number).collect::<Vec<_>>();
    let expected = Symbol::Tuple(symbols);
    let Value::Structured(value) = from_symbol(&expected).unwrap() else {
        panic!("tuple fixture")
    };
    let limit = 2 * value.nodes().len() as u128 * size_of::<Symbol>() as u128;
    let mut storage = ExportStorage::new(limit).unwrap();
    let mut nodes = value.nodes().iter();
    let actual = from_nodes(
        value.nodes().len(),
        value.depth(),
        &mut storage,
        |before| {
            before()?;
            Ok(nodes.next().map(ValueNode::view))
        },
        || Ok::<_, ()>(()),
    )
    .unwrap();
    assert_eq!(actual, expected);
    let Symbol::Tuple(arguments) = &actual else {
        panic!("tuple export")
    };
    assert_eq!(
        storage.retained,
        size_of::<Frame<'_>>() as u128
            + (arguments.capacity() + 1) as u128 * size_of::<Symbol>() as u128
    );
}
