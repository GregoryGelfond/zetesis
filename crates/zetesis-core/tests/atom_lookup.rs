//! Typed lookup agrees with canonical identity and preserves original row owners.
//! Refused construction/comparison exposes no partial index, range or membership.

use std::{cmp::Ordering, convert::Infallible};
use zetesis_core::catalog::AtomRef;

use zetesis_core::{
    Atom, AtomCatalog, AtomIndex, AtomIndexError, AtomPattern, Model, Predicate, Sign, Term, Value,
    ValueLimits, ValueNode,
};

fn atom(name: &str, sign: Sign, values: Vec<Value>) -> Atom {
    Atom::new(
        Predicate::with_sign(name, values.len(), sign).unwrap(),
        values,
    )
    .unwrap()
}

// The owned-input index still borrows its ingress payload. Canonical catalogs
// intentionally have a different physical representation; this helper is used
// only for AtomIndex::new_with over the original owned slice.
fn assert_ingress_borrow(actual: AtomRef<'_>, expected: &Atom) {
    assert_eq!(actual, AtomRef::from(expected));
    assert_eq!(
        actual.predicate().name().as_ptr(),
        expected.predicate().name().as_ptr()
    );
    for (actual, expected) in actual.values().iter().zip(expected.values()) {
        if let Value::String(text) | Value::Symbol(text) = expected {
            let (zetesis_core::ValueNodeRef::String(borrowed)
            | zetesis_core::ValueNodeRef::Symbol(borrowed)) = actual.descriptor()
            else {
                panic!("typed text fixture")
            };
            assert_eq!(borrowed.as_ptr(), text.as_ptr());
        }
    }
}

#[test]
fn stopped_index_exposes_the_callers_error_cause() {
    let error = AtomIndex::new_with(&[], || {
        Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stop"))
    })
    .unwrap_err();
    let source = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(source.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn intrinsic_index_refusals_have_no_underlying_cause() {
    for error in [
        AtomIndexError::<std::io::Error>::Allocation,
        AtomIndexError::Duplicate {
            first: 2,
            second: 7,
        },
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
}

fn values() -> Vec<Value> {
    let mut values = vec![
        Value::Infimum,
        Value::Number(i32::MIN),
        Value::Number(0),
        Value::Number(i32::MAX),
        Value::String(String::new()),
        Value::String("#inf".into()),
        Value::String("a\n\"é".into()),
        Value::Symbol("#inf".into()),
        Value::Symbol("a\n\"é".into()),
        Value::Supremum,
    ];
    for nodes in [
        vec![ValueNode::Tuple { arity: 0 }],
        vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(0)],
        vec![ValueNode::Function {
            name: "f".into(),
            sign: Sign::Negative,
            arity: 0,
        }],
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::String("é\n".into()),
        ],
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 1,
            },
            ValueNode::String("é\n".into()),
        ],
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 2,
            },
            ValueNode::Number(0),
            ValueNode::Number(1),
        ],
        vec![
            ValueNode::Function {
                name: "g".into(),
                sign: Sign::Positive,
                arity: 1,
            },
            ValueNode::Tuple { arity: 1 },
            ValueNode::Supremum,
        ],
    ] {
        values.push(Value::from_nodes(nodes, ValueLimits::default()).unwrap());
    }
    values
}

#[test]
fn checked_identity_agrees_with_storage_order() {
    for left in values() {
        for right in values() {
            let mut steps = 0;
            let order = left
                .compare_identity_with(&right, || {
                    steps += 1;
                    Ok::<_, Infallible>(())
                })
                .unwrap();
            assert_eq!(order, left.cmp(&right), "{left:?}, {right:?}");
            assert!(steps > 0);
        }
    }
    // This is identity/storage order, deliberately different from ASP term order.
    let string = Value::String("a".into());
    let symbol = Value::Symbol("a".into());
    assert_eq!(
        string
            .compare_identity_with(&symbol, || Ok::<_, Infallible>(()))
            .unwrap(),
        Ordering::Less
    );
    assert_eq!(string.compare_terms(&symbol), Ordering::Greater);
}

#[test]
fn identity_callbacks_cover_structural_self_comparison() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Positive,
                arity: 2,
            },
            ValueNode::Number(1),
            ValueNode::String("é".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    // Root; function descriptor, name byte and name terminator; number;
    // string descriptor, two UTF-8 bytes and text terminator; node terminator.
    let mut steps = 0;
    assert_eq!(
        value
            .compare_identity_with(&value, || {
                steps += 1;
                Ok::<_, Infallible>(())
            })
            .unwrap(),
        Ordering::Equal
    );
    assert_eq!(steps, 10);
    for limit in 0..10 {
        let mut spent = 0;
        let mut calls = 0;
        let result = value.compare_identity_with(&value, || {
            calls += 1;
            if spent == limit {
                return Err(spent);
            }
            spent += 1;
            Ok(())
        });
        assert_eq!(result, Err(limit));
        assert_eq!(spent, limit);
        assert_eq!(calls, limit + 1);
    }
}

#[test]
fn refused_identity_comparison_retains_its_exact_prefix() {
    let left = Value::String("shared prefix α".into());
    let right = Value::String("shared prefix β".into());
    let mut total = 0;
    left.compare_identity_with(&right, || {
        total += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for limit in 0..total {
        let mut spent = 0;
        let result = left.compare_identity_with(&right, || {
            if spent == limit {
                return Err(spent);
            }
            spent += 1;
            Ok(())
        });
        assert_eq!(result, Err(limit));
        assert_eq!(spent, limit);
    }
    // Mismatch at the first byte must not inspect/charge the large unused suffix.
    let short = Value::String("b".into());
    let long = Value::String(format!("a{}", "z".repeat(4096)));
    let mut steps = 0;
    short
        .compare_identity_with(&long, || {
            steps += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert!(steps < 8);
}

fn catalog() -> Vec<Atom> {
    let mut atoms: Vec<_> = values()
        .into_iter()
        .rev()
        .map(|value| atom("p", Sign::Positive, vec![value]))
        .collect();
    atoms.insert(3, atom("p", Sign::Negative, vec![Value::Number(0)]));
    atoms.insert(2, atom("p", Sign::Positive, vec![]));
    atoms.insert(
        1,
        atom(
            "p",
            Sign::Positive,
            vec![Value::Number(0), Value::Number(1)],
        ),
    );
    atoms.insert(0, atom("q", Sign::Positive, vec![Value::Number(0)]));
    atoms
}

#[test]
fn predicate_ranges_preserve_original_rows() {
    let atoms = catalog();
    let index = AtomIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let lookup = index.lookup();
    for query in &atoms {
        let actual: Vec<_> = lookup
            .predicate_with(query.predicate(), || Ok::<_, Infallible>(()))
            .unwrap()
            .map(|row| {
                assert_ingress_borrow(row.atom(), &atoms[row.position()]);
                row.position()
            })
            .collect();
        let expected: Vec<_> = atoms
            .iter()
            .enumerate()
            .filter(|(_, atom)| atom.predicate() == query.predicate())
            .map(|(i, _)| i)
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn key_lookup_preserves_full_typed_identity() {
    let atoms = catalog();
    let index = AtomIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let lookup = index.lookup();
    for query in &atoms {
        let row = lookup
            .get_with(query, || Ok::<_, Infallible>(()))
            .unwrap()
            .unwrap();
        assert_ingress_borrow(row.atom(), query);
        assert_eq!(&atoms[row.position()], query);
    }
    for missing in [
        atom("missing", Sign::Positive, vec![]),
        atom("p", Sign::Negative, vec![Value::Supremum]),
        atom("p", Sign::Positive, vec![Value::Number(77)]),
    ] {
        assert!(
            lookup
                .get_with(&missing, || Ok::<_, Infallible>(()))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn preparation_peak_includes_retained_index_storage() {
    let atoms = catalog();
    let index = AtomIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    assert!(index.preparation_peak_bytes() >= index.retained_bytes());
}

#[test]
fn model_lookup_never_selects_hidden_catalog_atoms() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let model = Model::from_positions(&atoms, [4, 7, 10, 7]).unwrap();
    let lookup = model.lookup();
    for (position, query) in atoms.atoms().iter().enumerate() {
        let found = lookup.get_with(query, || Ok::<_, Infallible>(())).unwrap();
        assert_eq!(found.is_some(), model.contains(query));
        if let Some(row) = found {
            assert_eq!(row.position(), position);
        }
    }
    let predicate = Predicate::new("p", 1).unwrap();
    let selected: Vec<_> = lookup
        .predicate_with(&predicate, || Ok::<_, Infallible>(()))
        .unwrap()
        .map(zetesis_core::AtomRow::atom)
        .collect();
    assert_eq!(
        selected,
        model
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().compare(&predicate).is_eq())
            .collect::<Vec<_>>()
    );
}

#[test]
fn bound_key_lookup_preserves_selected_identity() {
    let source = catalog();
    let atoms = AtomCatalog::new(source.clone()).unwrap();
    let model = Model::from_positions(&atoms, [4, 7, 10]).unwrap();
    for (position, query) in source.iter().enumerate() {
        let pattern = AtomPattern::new(
            query.predicate().clone(),
            (0..query.values().len()).map(Term::Variable).collect(),
        )
        .unwrap();
        let key = pattern.key(query.values()).unwrap();
        let found = model
            .lookup()
            .get_key_with(&key, || Ok::<_, Infallible>(()))
            .unwrap();
        assert_eq!(found.is_some(), model.contains(query));
        if let Some(row) = found {
            assert_eq!(row.position(), position);
            assert_eq!(row.atom(), AtomRef::from(query));
            assert_eq!(row.atom(), atoms.atoms().at(position).unwrap());
        }
    }
}

#[test]
fn refused_bound_key_never_reports_absence() {
    let atoms = AtomCatalog::new(catalog()).unwrap();
    let model = Model::from_positions(&atoms, 0..atoms.atoms().len()).unwrap();
    let pattern =
        AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
    let binding = [Value::String("a\\n\\\"é".into())];
    let key = pattern.key(binding.as_slice()).unwrap();
    let mut work = 0;
    model
        .lookup()
        .get_key_with(&key, || {
            work += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert!(work > 0);
    for limit in 0..work {
        let mut spent = 0;
        let result = model.lookup().get_key_with(&key, || {
            if spent == limit {
                return Err(limit);
            }
            spent += 1;
            Ok(())
        });
        assert!(matches!(result, Err(cause) if cause == limit));
        assert_eq!(spent, limit);
    }
}

#[test]
fn duplicate_catalog_rows_are_a_refusal_not_coalesced_ids() {
    let a = atom("a", Sign::Positive, vec![]);
    let atoms = [a.clone(), atom("b", Sign::Positive, vec![]), a];
    assert!(matches!(
        AtomIndex::new_with(&atoms, || Ok::<_, Infallible>(())),
        Err(AtomIndexError::Duplicate {
            first: 0,
            second: 2
        })
    ));
}

#[test]
fn index_stops_never_publish_a_partial_index() {
    let atoms = catalog();
    let mut total = 0;
    AtomIndex::new_with(&atoms, || {
        total += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    for limit in 0..total {
        let mut spent = 0;
        let result = AtomIndex::new_with(&atoms, || {
            if spent == limit {
                return Err(spent);
            }
            spent += 1;
            Ok(())
        });
        assert!(matches!(result, Err(AtomIndexError::Stopped(actual)) if actual == limit));
        assert_eq!(spent, limit);
    }
}

#[test]
fn lookup_stops_never_claim_absence() {
    let atoms = catalog();
    let index = AtomIndex::new_with(&atoms, || Ok::<_, Infallible>(())).unwrap();
    let query = &atoms[4];
    let mut total = 0;
    index
        .lookup()
        .get_with(query, || {
            total += 1;
            Ok::<_, Infallible>(())
        })
        .unwrap();
    for limit in 0..total {
        let mut spent = 0;
        let result = index.lookup().get_with(query, || {
            if spent == limit {
                return Err(spent);
            }
            spent += 1;
            Ok(())
        });
        assert_eq!(result.unwrap_err(), limit);
        assert_eq!(spent, limit);
    }
}

#[test]
fn empty_catalog_has_no_members() {
    let index = AtomIndex::new_with(&[], || Ok::<_, Infallible>(())).unwrap();
    let query = atom("p", Sign::Positive, vec![]);
    let mut calls = 0;
    let mut before = || {
        calls += 1;
        Ok::<_, Infallible>(())
    };
    assert!(
        index
            .lookup()
            .get_with(&query, &mut before)
            .unwrap()
            .is_none()
    );
    assert_eq!(calls, 0);
}

#[test]
fn empty_catalog_has_empty_predicate_ranges() {
    let index = AtomIndex::new_with(&[], || Ok::<_, Infallible>(())).unwrap();
    let query = atom("p", Sign::Positive, vec![]);
    let mut calls = 0;
    let mut before = || {
        calls += 1;
        Ok::<_, Infallible>(())
    };
    assert_eq!(
        index
            .lookup()
            .predicate_with(query.predicate(), &mut before)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(calls, 0);
}
