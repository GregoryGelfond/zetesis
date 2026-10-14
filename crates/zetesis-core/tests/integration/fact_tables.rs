//! Explicit facts preserve occurrence semantics while sharing canonical payload.

use zetesis_core::{
    AdmissionError, AdmissionLimits, AdmissionOrigin, AdmissionResource, FactSegment, FactTables,
    Predicate, Program, RuleRef, Sign, TemplateTerm, Value, ValueNode, ValueNodeRef,
};
use zetesis_test_support::programs::{atom, fact, number, numbered, value};
use zetesis_test_support::thresholds::first_success;

fn product() -> FactTables {
    let mut facts = FactTables::default();
    facts
        .push_product(
            0,
            Predicate::new("p", 2).unwrap(),
            vec![
                vec![
                    FactSegment::Value(Value::Number(9)),
                    FactSegment::Interval { first: -1, last: 1 },
                ],
                vec![FactSegment::Interval { first: 2, last: 3 }],
            ],
        )
        .unwrap();
    facts
}

#[test]
fn product_order_survives_canonical_interning() {
    let program = Program::new(vec![], product(), AdmissionLimits::default()).unwrap();
    let actual: Vec<_> = program
        .facts()
        .iter()
        .map(|fact| {
            fact.atom()
                .to_atom(zetesis_core::ValueLimits::default())
                .unwrap()
        })
        .collect();
    assert_eq!(
        actual,
        [
            (9, 2),
            (9, 3),
            (-1, 2),
            (-1, 3),
            (0, 2),
            (0, 3),
            (1, 2),
            (1, 3)
        ]
        .map(|(left, right)| numbered("p", &[left, right]))
    );
    assert!(program.templates().is_empty());
    assert!(!program.is_empty());
    assert_eq!(program.rules().len(), 8);
    let ordered: Vec<_> = program
        .fact_relations()
        .flat_map(zetesis_core::FactRelation::canonical_atoms)
        .map(|atom| atom.to_atom(zetesis_core::ValueLimits::default()).unwrap())
        .collect();
    assert_eq!(
        ordered,
        [
            (-1, 2),
            (-1, 3),
            (0, 2),
            (0, 3),
            (1, 2),
            (1, 3),
            (9, 2),
            (9, 3)
        ]
        .map(|(left, right)| numbered("p", &[left, right]))
    );
}

#[test]
fn occurrences_preserve_interleaving_and_duplicates() {
    let predicate = Predicate::new("p", 1).unwrap();
    let mut facts = FactTables::default();
    facts
        .push_row(0, predicate.clone(), vec![Value::Number(2)])
        .unwrap();
    facts
        .push_row(1, predicate.clone(), vec![Value::Number(2)])
        .unwrap();
    facts
        .push_row(1, predicate, vec![Value::Number(1)])
        .unwrap();
    let program = Program::new(
        vec![fact("middle", vec![])],
        facts,
        AdmissionLimits::default(),
    )
    .unwrap();
    let sequence: Vec<_> = program
        .rules()
        .map(|rule| match rule {
            RuleRef::Template { index, template } => (
                "template",
                index,
                template
                    .head()
                    .unwrap()
                    .key(&[] as &[Value])
                    .unwrap()
                    .to_atom(zetesis_core::ValueLimits::default())
                    .unwrap(),
            ),
            RuleRef::Fact(fact) => (
                "fact",
                fact.index(),
                fact.atom()
                    .to_atom(zetesis_core::ValueLimits::default())
                    .unwrap(),
            ),
        })
        .collect();
    assert_eq!(
        sequence,
        [
            ("fact", 0, numbered("p", &[2])),
            ("template", 0, atom("middle", vec![])),
            ("fact", 1, numbered("p", &[2])),
            ("fact", 2, numbered("p", &[1]))
        ]
    );
    assert_eq!(program.fact_relations().next().unwrap().atoms().len(), 2);
}

#[test]
fn table_values_share_vocabulary_without_subterm_domain() {
    let structured = value(vec![
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity: 1,
        },
        ValueNode::Symbol("child".into()),
    ]);
    let mut facts = FactTables::default();
    facts
        .push_row(0, Predicate::new("p", 1).unwrap(), vec![structured.clone()])
        .unwrap();
    let program = Program::new(
        vec![fact(
            "q",
            vec![zetesis_core::Term::Constant(structured.clone())],
        )],
        facts,
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(program.domain().len(), 1);
    assert_eq!(program.domain().at(0).unwrap(), structured);
    let atom = program.facts().at(0).unwrap().atom();
    let ValueNodeRef::Function {
        name: fact_name, ..
    } = atom.values().at(0).unwrap().descriptor()
    else {
        panic!("structured fact");
    };
    let ValueNodeRef::Function {
        name: domain_name, ..
    } = program.domain().at(0).unwrap().descriptor()
    else {
        panic!("structured domain");
    };
    assert!(std::ptr::eq(fact_name, domain_name));
    assert_eq!(program.gate_predicates().len(), 0);
    let terms = program.facts().at(0).unwrap().pattern().terms();
    assert!(matches!(terms.at(0), Some(TemplateTerm::Constant(_))));
    assert_eq!(terms.slice(0..1).unwrap().len(), 1);
    assert!(terms.slice(std::ops::Range { start: 1, end: 0 }).is_none());
    assert_eq!(terms.variables().count(), 0);
}

#[test]
fn empty_products_do_not_invent_domain_values() {
    let mut facts = FactTables::default();
    facts
        .push_product(
            0,
            Predicate::new("p", 2).unwrap(),
            vec![
                vec![FactSegment::Interval { first: 2, last: 1 }],
                vec![FactSegment::Value(Value::Symbol("absent".into()))],
            ],
        )
        .unwrap();
    facts
        .push_product(0, Predicate::new("unit", 0).unwrap(), vec![])
        .unwrap();
    facts
        .push_row(0, Predicate::new("unit", 0).unwrap(), vec![])
        .unwrap();
    let program = Program::new(vec![], facts, AdmissionLimits::default()).unwrap();
    assert_eq!(program.domain().len(), 0);
    assert_eq!(program.facts().len(), 2);
    assert_eq!(program.fact_relations().next().unwrap().atoms().len(), 1);
    assert_eq!(program.predicates().len(), 1);
}

#[test]
fn failed_input_cannot_publish_an_accepted_prefix() {
    let mut facts = FactTables::default();
    facts
        .push_row(0, Predicate::new("p", 1).unwrap(), vec![Value::Number(1)])
        .unwrap();
    assert_eq!(
        facts.push_row(0, Predicate::new("p", 1).unwrap(), vec![]),
        Err(zetesis_core::catalog::Error::Shape)
    );
    assert!(matches!(
        Program::new(vec![], facts, AdmissionLimits::default()),
        Err(AdmissionError::Canonical {
            error: zetesis_core::catalog::Error::Shape,
            ..
        })
    ));
}

#[test]
fn occurrence_limit_counts_duplicate_facts() {
    let mut facts = FactTables::default();
    for _ in 0..3 {
        facts
            .push_row(0, Predicate::new("p", 0).unwrap(), vec![])
            .unwrap();
    }
    assert!(matches!(
        Program::new(
            vec![],
            facts,
            AdmissionLimits {
                max_templates: 2,
                ..AdmissionLimits::default()
            }
        ),
        Err(AdmissionError::LimitExceeded {
            resource: AdmissionResource::Templates,
            actual: 3,
            limit: 2,
            ..
        })
    ));
}

#[test]
fn core_preserves_explicit_template_facts() {
    let program = Program::new(
        vec![fact("p", vec![number(1)])],
        FactTables::default(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(program.templates().len(), 1);
    assert_eq!(program.facts().len(), 0);
    assert!(program.fact_catalog().is_none());
}

#[test]
fn fact_storage_threshold_reports_the_complete_owner() {
    let attempt = |max_bytes| {
        Program::new(
            vec![],
            product(),
            AdmissionLimits {
                max_bytes,
                ..AdmissionLimits::default()
            },
        )
    };
    let least = first_success(1_000_000_usize, |limit| attempt(limit).is_ok());
    let program = attempt(least).unwrap();
    assert!(program.storage_bytes() <= least as u128);
    for limit in [0, least / 2, least - 1] {
        let error = attempt(limit).unwrap_err();
        let AdmissionError::Canonical {
            error:
                zetesis_core::catalog::Error::Storage {
                    required,
                    limit: reported,
                },
            ..
        } = error
        else {
            panic!("storage refusal: {error:?}");
        };
        assert_eq!(reported, limit);
        assert!(required > limit as u128);
    }
}

#[test]
fn fused_run_import_refusal_names_the_failing_row() {
    let mut facts = FactTables::default();
    facts
        .push_row(0, Predicate::new("prefix", 0).unwrap(), vec![])
        .unwrap();
    let predicate = Predicate::new("p", 1).unwrap();
    facts
        .push_row(0, predicate.clone(), vec![Value::Number(1)])
        .unwrap();
    facts
        .push_row(0, predicate, vec![Value::String("x".repeat(65_536))])
        .unwrap();
    let error = Program::new(
        vec![],
        facts,
        AdmissionLimits {
            max_bytes: 16_384,
            ..AdmissionLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.fact_index(), Some(2));
    assert!(matches!(
        error,
        AdmissionError::Canonical {
            error: zetesis_core::catalog::Error::Storage { .. },
            ..
        }
    ));
}

#[test]
fn product_import_refusal_names_first_containing_row() {
    for (column, expected) in [(0, 2), (1, 1)] {
        let mut columns = vec![
            vec![
                FactSegment::Value(Value::Number(1)),
                FactSegment::Value(Value::Number(2))
            ];
            2
        ];
        columns[column][1] = FactSegment::Value(Value::String("x".repeat(65_536)));
        let mut facts = FactTables::default();
        facts
            .push_product(0, Predicate::new("pair", 2).unwrap(), columns)
            .unwrap();
        let error = Program::new(
            vec![],
            facts,
            AdmissionLimits {
                max_bytes: 16_384,
                ..AdmissionLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.fact_index(), Some(expected));
        assert!(matches!(
            error,
            AdmissionError::Canonical {
                error: zetesis_core::catalog::Error::Storage { .. },
                ..
            }
        ));
    }
}

#[test]
fn product_size_overflow_refuses_before_expansion() {
    let mut facts = FactTables::default();
    let full = vec![FactSegment::Interval {
        first: i32::MIN,
        last: i32::MAX,
    }];
    assert_eq!(
        facts.push_product(
            0,
            Predicate::new("pair", 2).unwrap(),
            vec![full.clone(), full]
        ),
        Err(zetesis_core::catalog::Error::Overflow)
    );
    assert!(facts.is_empty());
    assert!(matches!(
        Program::new(vec![], facts, AdmissionLimits::default()),
        Err(AdmissionError::Canonical {
            error: zetesis_core::catalog::Error::Overflow,
            ..
        })
    ));
}

#[test]
fn anchors_cannot_address_a_missing_template() {
    let mut facts = FactTables::default();
    facts
        .push_row(1, Predicate::new("p", 0).unwrap(), vec![])
        .unwrap();
    let error = Program::new(vec![], facts, AdmissionLimits::default()).unwrap_err();
    assert_eq!(error.fact_index(), Some(0));
    assert_eq!(error.template_index(), None);
    assert!(matches!(
        error,
        AdmissionError::Canonical {
            error: zetesis_core::catalog::Error::Shape,
            origin: AdmissionOrigin::Fact(0)
        }
    ));
}

#[test]
fn canonical_causes_retain_their_input_identity_and_error_source() {
    use std::error::Error as _;
    use zetesis_core::catalog::Error;
    for (origin, template, fact, location) in [
        (AdmissionOrigin::Program, None, None, "template None"),
        (
            AdmissionOrigin::Template(3),
            Some(3),
            None,
            "template Some(3)",
        ),
        (AdmissionOrigin::Fact(5), None, Some(5), "fact 5"),
    ] {
        let refusal = AdmissionError::Canonical {
            error: Error::Allocation,
            origin,
        };
        assert_eq!(refusal.template_index(), template);
        assert_eq!(refusal.fact_index(), fact);
        assert_eq!(
            refusal.source().unwrap().downcast_ref::<Error>(),
            Some(&Error::Allocation)
        );
        assert_eq!(
            refusal.to_string(),
            format!(
                "program admission failed ({location}): {}",
                Error::Allocation
            )
        );
    }
}
