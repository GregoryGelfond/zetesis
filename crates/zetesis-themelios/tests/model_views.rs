//! Typed view consumers need no CLI, solver, output writer or ASP-text parser.

use serde_json::{Value as Json, json};
use zetesis_core::{Atom, Model, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Control;
use zetesis_themelios::observation::{
    Limits, ObservationProgram, Symbol, SymbolSign, ViewError, ViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, OutputSelection, admit_formula,
};

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

#[test]
fn typed_json_preserves_every_value_class_and_all_json_control_characters() {
    let text: String = (0..32)
        .map(|value| char::from_u32(value).unwrap())
        .collect();
    let text = format!("{text}\"\\λ🦀");
    let structured = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: Sign::Negative,
                arity: 2,
            },
            ValueNode::Tuple { arity: 1 },
            ValueNode::Number(7),
            ValueNode::Tuple { arity: 0 },
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let model = Model::new([Atom::new(
        Predicate::with_sign("p", 6, Sign::Negative).unwrap(),
        vec![
            Value::Infimum,
            Value::Supremum,
            Value::Number(i32::MIN),
            Value::Symbol("#inf".into()),
            Value::String(text.clone()),
            structured,
        ],
    )
    .unwrap()]);
    let program = ObservationProgram::default();
    let selection = OutputSelection::default();
    let control = Control::default();
    let view = program
        .view(&model, &selection, None, Limits::default(), &control)
        .unwrap();
    assert!(std::ptr::eq(view.model(), std::ptr::from_ref(&model)));
    assert_eq!(view.shown_atoms().count(), 1);
    assert!(view.shown_terms().is_empty());
    let value: Json =
        serde_json::from_str(&view.json(ViewLimits::default(), &control).unwrap()).unwrap();
    let atom = &value["full_model"][0];
    assert_eq!(atom["sign"], "negative");
    assert_eq!(atom["arguments"][0][0], json!({"kind":"infimum"}));
    assert_eq!(atom["arguments"][1][0], json!({"kind":"supremum"}));
    assert_eq!(atom["arguments"][2][0]["value"], i32::MIN);
    assert_eq!(
        atom["arguments"][3][0],
        json!({"kind":"symbol","value":"#inf"})
    );
    assert_eq!(
        atom["arguments"][4][0],
        json!({"kind":"string","value":text})
    );
    assert_eq!(
        atom["arguments"][5],
        json!([
            {"kind":"function","name":"f","sign":"negative","arity":2},
            {"kind":"tuple","arity":1},{"kind":"number","value":7},{"kind":"tuple","arity":0}
        ])
    );
    assert!(value["costs"].is_null());
    assert_eq!(value["shown"]["atom_indices"], json!([0]));
}

#[test]
fn model_and_two_shown_channels_remain_separate_typed_values() {
    let admitted = admit_formula(
        "a. #show a. #show f((1,),()).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let model = Model::new([atom("a", vec![]), atom("hidden", vec![])]);
    let control = Control::default();
    let view = admitted
        .metadata()
        .observations()
        .view(
            &model,
            admitted.metadata().output(),
            None,
            Limits::default(),
            &control,
        )
        .unwrap();
    let value: Json =
        serde_json::from_str(&view.json(ViewLimits::default(), &control).unwrap()).unwrap();
    assert_eq!(view.model().atoms().len(), 2);
    assert_eq!(view.shown_atoms().count(), 2);
    assert_eq!(view.shown_terms().len(), 2);
    assert!(
        matches!(&view.shown_terms()[0], Symbol::Function { name, sign: SymbolSign::Positive, .. } if name.as_str() == "a")
    );
    assert_eq!(
        value["shown"]["terms"][0],
        json!([{"kind":"symbol","value":"a"}])
    );
    let hidden = admit_formula(
        "#show.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let hidden_view = hidden
        .metadata()
        .observations()
        .view(
            &model,
            hidden.metadata().output(),
            None,
            Limits::default(),
            &control,
        )
        .unwrap();
    assert_eq!(hidden_view.model().atoms(), model.atoms());
    assert_eq!(hidden_view.shown_atoms().count(), 0);
}

#[test]
fn record_ceiling_is_inclusive_and_control_work_depth_refusals_return_no_value() {
    let model = Model::new([atom("p", vec![Value::String("\n\"λ".into())])]);
    let program = ObservationProgram::default();
    let selection = OutputSelection::default();
    let control = Control::default();
    let view = program
        .view(&model, &selection, None, Limits::default(), &control)
        .unwrap();
    let expected = view.json(ViewLimits::default(), &control).unwrap();
    for maximum in [0, expected.len() - 1] {
        assert_eq!(
            view.json(
                ViewLimits {
                    max_bytes: maximum,
                    ..Default::default()
                },
                &control
            ),
            Err(ViewError::Bytes)
        );
    }
    assert_eq!(
        view.json(
            ViewLimits {
                max_bytes: expected.len(),
                ..Default::default()
            },
            &control
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        view.json(
            ViewLimits {
                max_work: 0,
                ..Default::default()
            },
            &control
        ),
        Err(ViewError::Work)
    );
    assert_eq!(
        view.json(
            ViewLimits {
                max_depth: 0,
                ..Default::default()
            },
            &control
        ),
        Err(ViewError::Depth)
    );
    control.cancel();
    assert_eq!(
        view.json(ViewLimits::default(), &control),
        Err(ViewError::Stopped(zetesis_cpu::Stop::Cancelled))
    );
}

#[test]
fn observation_constructor_cursor_is_preorder_and_bounded_before_descent() {
    let admitted = admit_formula(
        "#show f(g(1),(2,)).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let model = Model::new([]);
    let control = Control::default();
    let view = admitted
        .metadata()
        .observations()
        .view(
            &model,
            admitted.metadata().output(),
            None,
            Limits::default(),
            &control,
        )
        .unwrap();
    assert_eq!(
        view.json(
            ViewLimits {
                max_depth: 2,
                ..Default::default()
            },
            &control
        ),
        Err(ViewError::Depth)
    );
    let value: Json = serde_json::from_str(
        &view
            .json(
                ViewLimits {
                    max_depth: 3,
                    ..Default::default()
                },
                &control,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value["shown"]["terms"][0],
        json!([
            {"kind":"function","name":"f","sign":"positive","arity":2},
            {"kind":"function","name":"g","sign":"positive","arity":1},
            {"kind":"number","value":1},{"kind":"tuple","arity":1},{"kind":"number","value":2}
        ])
    );
}
