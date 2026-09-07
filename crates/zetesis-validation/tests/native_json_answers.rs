//! Native typed records preserve full identity independently of shown output.
use serde_json::{Value as Json, json};
use zetesis_validation::answers::{Error, Issue, Resource, native_json};
use zetesis_validation::core::{Sign, Value, ValueError, ValueResource};

fn atom(name: &str, arguments: Vec<Json>) -> Json {
    json!({"predicate":name,"sign":"positive","arguments":Json::Array(arguments)})
}
fn record(atoms: Vec<Json>, number: usize) -> Json {
    let count = atoms.len();
    json!({"number":number,"model":{"full_model":Json::Array(atoms),"shown":{"atom_indices":(0..count).collect::<Vec<_>>(),"terms":[]},"costs":null}})
}
fn document(records: Vec<Json>) -> Json {
    let count = records.len();
    json!({"schema":1,"format":"zetesis","models":Json::Array(records),"outcome":{
        "status":if count==0 {"unsatisfiable"} else {"satisfiable"},"completion":"exhausted","coverage":"exhausted",
        "published_models":count,"verified_models":count,"checked":count,"interruption":null,"optimization":null,"error":null
    },"statistics":null})
}
fn parse(value: &Json) -> Result<native_json::NativeAnswers, Error> {
    native_json::parse(
        &serde_json::to_vec(value).unwrap(),
        native_json::Limits::default(),
    )
}
fn one() -> Json {
    document(vec![record(vec![atom("a", vec![])], 1)])
}

#[test]
fn full_identity_survives_empty_shown_channels() {
    let mut value = document(vec![
        record(vec![atom("a", vec![])], 1),
        record(vec![atom("b", vec![])], 2),
    ]);
    for record in value["models"].as_array_mut().unwrap() {
        record["model"]["shown"]["atom_indices"] = json!([]);
    }
    let answer = parse(&value).unwrap();
    assert!(
        answer
            .records()
            .iter()
            .all(|record| record.shown_atom_indices().is_empty())
    );
    assert_eq!(
        answer.full_model_symbols(2).unwrap(),
        [vec!["a"], vec!["b"]]
    );
}

#[test]
fn repeated_full_model_records_are_retained() {
    let answer = parse(&document(vec![
        record(vec![atom("a", vec![])], 1),
        record(vec![atom("a", vec![])], 2),
    ]))
    .unwrap();
    assert_eq!(
        answer.full_model_symbols(2).unwrap(),
        [vec!["a"], vec!["a"]]
    );
}

#[test]
fn atom_order_is_irrelevant_to_full_model_comparison() {
    let answer = parse(&document(vec![record(
        vec![atom("b", vec![]), atom("a", vec![])],
        1,
    )]))
    .unwrap();
    assert_eq!(answer.full_model_symbols(2).unwrap(), [vec!["a", "b"]]);
}

#[test]
fn duplicate_atoms_cannot_form_a_full_model_record() {
    let result = parse(&document(vec![record(
        vec![atom("a", vec![]), atom("a", vec![])],
        1,
    )]));
    assert!(matches!(
        result,
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn outer_atom_sign_is_independent_of_constructor_sign() {
    let mut a = atom(
        "p",
        vec![
            json!([{"kind":"function","name":"f","sign":"negative","arity":1},{"kind":"symbol","value":"a"}]),
        ],
    );
    a["sign"] = "negative".into();
    let answer = parse(&document(vec![record(vec![a], 1)])).unwrap();
    assert_eq!(
        answer.records()[0].full_model()[0].predicate().sign(),
        Sign::Negative
    );
    assert_eq!(answer.full_model_symbols(9).unwrap(), [vec!["-p(-f(a))"]]);
}

#[test]
fn scalar_value_kinds_remain_distinct() {
    let arguments = vec![
        json!([{"kind":"infimum"}]),
        json!([{"kind":"number","value":i32::MIN}]),
        json!([{"kind":"symbol","value":"a"}]),
        json!([{"kind":"string","value":"a"}]),
        json!([{"kind":"supremum"}]),
    ];
    let answer = parse(&document(vec![record(vec![atom("p", arguments)], 1)])).unwrap();
    assert_eq!(
        answer.records()[0].full_model()[0].values(),
        &[
            Value::Infimum,
            Value::Number(i32::MIN),
            Value::Symbol("a".into()),
            Value::String("a".into()),
            Value::Supremum
        ]
    );
    assert_eq!(
        answer.full_model_symbols(64).unwrap(),
        [vec!["p(#inf,-2147483648,a,\"a\",#sup)"]]
    );
}

#[test]
fn singleton_tuple_does_not_collapse_to_its_child() {
    let answer = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"tuple","arity":1},{"kind":"number","value":1}])],
        )],
        1,
    )]))
    .unwrap();
    assert!(matches!(
        answer.records()[0].full_model()[0].values()[0],
        Value::Structured(_)
    ));
    assert_eq!(answer.full_model_symbols(7).unwrap(), [vec!["p((1,))"]]);
}

#[test]
fn empty_tuple_retains_its_constructor() {
    let answer = parse(&document(vec![record(
        vec![atom("p", vec![json!([{"kind":"tuple","arity":0}])])],
        1,
    )]))
    .unwrap();
    assert_eq!(answer.full_model_symbols(5).unwrap(), [vec!["p(())"]]);
}

#[test]
fn scalar_string_spelling_agrees_with_nested_core_spelling() {
    let string = "a\n\"b\\c";
    let answer = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![
                json!([{"kind":"string","value":string}]),
                json!([{"kind":"tuple","arity":1},{"kind":"string","value":string}]),
            ],
        )],
        1,
    )]))
    .unwrap();
    assert_eq!(
        answer.full_model_symbols(128).unwrap(),
        [vec!["p(\"a\\n\\\"b\\\\c\",(\"a\\n\\\"b\\\\c\",))"]]
    );
}

#[test]
fn malformed_flat_value_is_refused_by_core_construction() {
    let result = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"tuple","arity":2},{"kind":"number","value":1}])],
        )],
        1,
    )]));
    assert!(matches!(result, Err(Error::Value(ValueError::Shape))));
}

#[test]
fn machine_integer_overflow_is_not_truncated() {
    let result = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"number","value":2_147_483_648_i64}])],
        )],
        1,
    )]));
    assert!(matches!(
        result,
        Err(Error::Invalid {
            issue: Issue::MalformedField,
            ..
        })
    ));
}

#[test]
fn shown_indices_must_refer_to_the_full_model() {
    let mut value = one();
    value["models"][0]["model"]["shown"]["atom_indices"] = json!([1]);
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn repeated_shown_terms_remain_separate_occurrences() {
    let mut value = one();
    value["models"][0]["model"]["shown"]["terms"] =
        json!([[{"kind":"symbol","value":"a"}],[{"kind":"symbol","value":"a"}]]);
    let answer = parse(&value).unwrap();
    assert_eq!(
        answer.records()[0].shown_terms(),
        [Value::Symbol("a".into()), Value::Symbol("a".into())]
    );
    assert_eq!(answer.records()[0].full_model().len(), 1);
}

#[test]
fn malformed_shown_value_cannot_hide_behind_full_model_comparison() {
    let mut value = one();
    value["models"][0]["model"]["shown"]["terms"] = json!([[{"kind":"tuple","arity":1}]]);
    assert!(matches!(
        parse(&value),
        Err(Error::Value(ValueError::Shape))
    ));
}

#[test]
fn partial_publication_is_not_complete_enumeration() {
    let mut value = one();
    value["outcome"]["published_models"] = 2.into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn unknown_schema_is_refused() {
    let mut value = one();
    value["schema"] = 2.into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::MalformedField,
            ..
        })
    ));
}

#[test]
fn selected_count_cannot_replace_exhausted_coverage() {
    let mut value = one();
    value["outcome"]["completion"] = "requested_models".into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Incomplete,
            ..
        })
    ));
}

fn optimal() -> Json {
    let costs = json!([{"priority":2,"value":-1},{"priority":-3,"value":i64::MAX}]);
    let mut value = document(vec![
        record(vec![atom("a", vec![])], 1),
        record(vec![atom("b", vec![])], 2),
    ]);
    value["outcome"]["verified_models"] = 3.into();
    value["outcome"]["checked"] = 5.into();
    value["outcome"]["optimization"] =
        json!({"optimal":true,"tied_models":2,"scored_models":3,"work":10,"costs":costs});
    for record in value["models"].as_array_mut().unwrap() {
        record["model"]["costs"] = costs.clone();
    }
    value
}

#[test]
fn completed_optimum_preserves_priority_slots_and_ties() {
    let answer = parse(&optimal()).unwrap();
    assert_eq!(answer.costs(), Some([(2, -1), (-3, i64::MAX)].as_slice()));
    assert_eq!(answer.records().len(), 2);
    assert_eq!(answer.verified_models(), 3);
    assert_eq!(answer.checked(), 5);
}

#[test]
fn failed_envelope_cannot_qualify_through_retained_optimum() {
    let mut value = optimal();
    value["outcome"]["status"] = "failed".into();
    value["outcome"]["error"] = json!({"kind":"output","secondary_output_failure":false});
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Incomplete,
            ..
        })
    ));
}

#[test]
fn final_tie_cost_must_equal_the_reported_optimum() {
    let mut value = optimal();
    value["models"][1]["model"]["costs"][0]["value"] = 0.into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn repeated_priority_slots_are_not_a_cost_vector() {
    let mut value = optimal();
    value["outcome"]["optimization"]["costs"][1]["priority"] = 2.into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn unscored_verified_models_cannot_establish_exact_optimization() {
    let mut value = optimal();
    value["outcome"]["optimization"]["scored_models"] = 2.into();
    assert!(matches!(
        parse(&value),
        Err(Error::Invalid {
            issue: Issue::Contradiction,
            ..
        })
    ));
}

#[test]
fn input_ceiling_precedes_json_decoding() {
    let mut limits = native_json::Limits::default();
    limits.report.max_input_bytes = 0;
    assert!(matches!(
        native_json::parse(b"{", limits),
        Err(Error::Limit {
            resource: Resource::InputBytes,
            ..
        })
    ));
}

#[test]
fn full_atom_occurrence_ceiling_is_inclusive() {
    let bytes = serde_json::to_vec(&one()).unwrap();
    let limits = native_json::Limits {
        max_atoms: 1,
        ..native_json::Limits::default()
    };
    assert!(native_json::parse(&bytes, limits).is_ok());
    assert!(matches!(
        native_json::parse(
            &bytes,
            native_json::Limits {
                max_atoms: 0,
                ..limits
            }
        ),
        Err(Error::Limit {
            resource: Resource::Atoms,
            attempted: 1,
            ..
        })
    ));
}

#[test]
fn node_ceiling_includes_shown_terms() {
    let mut value = one();
    value["models"][0]["model"]["shown"]["terms"] = json!([[{"kind":"number","value":1}]]);
    let limits = native_json::Limits {
        max_value_nodes: 0,
        ..native_json::Limits::default()
    };
    assert!(matches!(
        native_json::parse(&serde_json::to_vec(&value).unwrap(), limits),
        Err(Error::Limit {
            resource: Resource::ValueNodes,
            attempted: 1,
            ..
        })
    ));
}

#[test]
fn value_depth_limit_is_the_core_construction_limit() {
    let value = document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"tuple","arity":1},{"kind":"number","value":1}])],
        )],
        1,
    )]);
    let mut limits = native_json::Limits::default();
    limits.value.max_depth = 1;
    assert!(matches!(
        native_json::parse(&serde_json::to_vec(&value).unwrap(), limits),
        Err(Error::Value(ValueError::Limit {
            resource: ValueResource::Depth,
            ..
        }))
    ));
}

#[test]
fn spelling_ceiling_refuses_before_returning_a_partial_view() {
    let answer = parse(&one()).unwrap();
    assert!(matches!(
        answer.full_model_symbols(0),
        Err(Error::Limit {
            resource: Resource::SpellingBytes,
            limit: 0,
            attempted: 1
        })
    ));
    assert_eq!(answer.full_model_symbols(1).unwrap(), [vec!["a"]]);
}

#[test]
fn empty_full_interpretation_remains_satisfiable() {
    let answer = parse(&document(vec![record(Vec::new(), 1)])).unwrap();
    assert!(answer.satisfiable());
    assert_eq!(
        answer.full_model_symbols(0).unwrap(),
        [Vec::<String>::new()]
    );
}

#[test]
fn unsatisfiable_report_contains_no_full_interpretations() {
    let answer = parse(&document(Vec::new())).unwrap();
    assert!(!answer.satisfiable());
    assert!(answer.full_model_symbols(0).unwrap().is_empty());
}

#[test]
fn a_nullary_predicate_cannot_impersonate_an_applied_atom() {
    let invalid = parse(&document(vec![record(vec![atom("p(a)", vec![])], 1)])).unwrap();
    let valid = parse(&document(vec![record(
        vec![atom("p", vec![json!([{"kind":"symbol","value":"a"}])])],
        1,
    )]))
    .unwrap();
    assert_ne!(
        invalid.records()[0].full_model(),
        valid.records()[0].full_model()
    );
    assert!(matches!(
        invalid.full_model_symbols(64),
        Err(Error::Identifier(_))
    ));
    assert_eq!(valid.full_model_symbols(64).unwrap(), [vec!["p(a)"]]);
}

#[test]
fn a_symbol_cannot_impersonate_an_extremum_sentinel() {
    let invalid = parse(&document(vec![record(
        vec![atom("p", vec![json!([{"kind":"symbol","value":"#inf"}])])],
        1,
    )]))
    .unwrap();
    let valid = parse(&document(vec![record(
        vec![atom("p", vec![json!([{"kind":"infimum"}])])],
        1,
    )]))
    .unwrap();
    assert_ne!(
        invalid.records()[0].full_model(),
        valid.records()[0].full_model()
    );
    assert!(matches!(
        invalid.full_model_symbols(64),
        Err(Error::Identifier(_))
    ));
    assert_eq!(valid.full_model_symbols(64).unwrap(), [vec!["p(#inf)"]]);
}

#[test]
fn a_nested_function_name_cannot_impersonate_its_arguments() {
    let invalid = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"function","name":"f(a)","sign":"negative","arity":0}])],
        )],
        1,
    )]))
    .unwrap();
    let valid = parse(&document(vec![record(vec![atom("p", vec![json!([{"kind":"function","name":"f","sign":"negative","arity":1},{"kind":"symbol","value":"a"}])])], 1)])).unwrap();
    assert_ne!(
        invalid.records()[0].full_model(),
        valid.records()[0].full_model()
    );
    assert!(matches!(
        invalid.full_model_symbols(64),
        Err(Error::Identifier(_))
    ));
    assert_eq!(valid.full_model_symbols(64).unwrap(), [vec!["p(-f(a))"]]);
}

#[test]
fn a_nested_symbol_cannot_impersonate_two_arguments() {
    let invalid = parse(&document(vec![record(vec![atom("p", vec![json!([{"kind":"function","name":"f","sign":"positive","arity":1},{"kind":"symbol","value":"a,b"}])])], 1)])).unwrap();
    let valid = parse(&document(vec![record(vec![atom("p", vec![json!([{"kind":"function","name":"f","sign":"positive","arity":2},{"kind":"symbol","value":"a"},{"kind":"symbol","value":"b"}])])], 1)])).unwrap();
    assert_ne!(
        invalid.records()[0].full_model(),
        valid.records()[0].full_model()
    );
    assert!(matches!(
        invalid.full_model_symbols(64),
        Err(Error::Identifier(_))
    ));
    assert_eq!(valid.full_model_symbols(64).unwrap(), [vec!["p(f(a,b))"]]);
}

#[test]
fn quoted_string_content_need_not_be_an_identifier() {
    let answer = parse(&document(vec![record(
        vec![atom(
            "p",
            vec![json!([{"kind":"string","value":"#inf f(a), b"}])],
        )],
        1,
    )]))
    .unwrap();
    assert_eq!(
        answer.full_model_symbols(64).unwrap(),
        [vec!["p(\"#inf f(a), b\")"]]
    );
}

#[test]
fn spelling_byte_refusal_precedes_identifier_copying() {
    let answer = parse(&document(vec![record(vec![atom("p(a)", vec![])], 1)])).unwrap();
    assert!(matches!(
        answer.full_model_symbols(0),
        Err(Error::Limit {
            resource: Resource::SpellingBytes,
            attempted: 4,
            ..
        })
    ));
    match answer.full_model_symbols(4) {
        Err(Error::Identifier(error)) => assert_eq!(error.text, "p(a)"),
        other => panic!("invalid identifier was not retained: {other:?}"),
    }
}
