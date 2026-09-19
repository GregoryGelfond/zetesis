//! Typed view consumers need no CLI, solver, output writer or ASP-text parser.

use serde_json::{Value as Json, json};
use zetesis_core::{Atom, Model, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Control;
use zetesis_themelios::observation::{
    Limits, ModelView, ObservationProgram, Symbol, SymbolSign, ViewError, ViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, OutputSelection, admit_formula,
};

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

struct ObservationFixture {
    model: Model,
    program: ObservationProgram,
    selection: OutputSelection,
    control: Control,
}
impl ObservationFixture {
    fn plain(model: Model) -> Self {
        Self {
            model,
            program: ObservationProgram::default(),
            selection: OutputSelection::default(),
            control: Control::default(),
        }
    }

    fn with_directives(model: Model, source: &str) -> Self {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        Self {
            model,
            program: admitted.metadata().observations().clone(),
            selection: admitted.metadata().output().clone(),
            control: Control::default(),
        }
    }

    fn view(&self) -> ModelView<'_> {
        self.program
            .view(
                &self.model,
                &self.selection,
                None,
                Limits::default(),
                &self.control,
            )
            .unwrap()
    }

    fn json(&self) -> Json {
        serde_json::from_str(
            &self
                .view()
                .json(ViewLimits::default(), &self.control)
                .unwrap(),
        )
        .unwrap()
    }
}

#[test]
fn a_refused_record_after_the_first_leaves_the_first_records_atoms_indexed() {
    // The first record's atoms are indexed on the first lookup after it. A
    // second record refused before its first lookup withdraws nothing of the
    // first, so a third record refers to the first's atoms by their indices
    // and spells only its own new atom.
    use zetesis_themelios::observation::json::AtomTable;
    let first = ObservationFixture::plain(Model::new([atom("a", vec![]), atom("b", vec![])]));
    let later = ObservationFixture::plain(Model::new([atom("a", vec![]), atom("c", vec![])]));
    let mut table = AtomTable::new(16);
    first
        .view()
        .record(&mut table, ViewLimits::default(), &first.control)
        .unwrap();
    let refused = later.view().record(
        &mut table,
        ViewLimits {
            max_bytes: 1,
            ..ViewLimits::default()
        },
        &later.control,
    );
    assert!(matches!(refused, Err(ViewError::Bytes)));
    let third: Json = serde_json::from_str(
        &later
            .view()
            .record(&mut table, ViewLimits::default(), &later.control)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(third["atoms"].as_array().unwrap().len(), 1);
    assert_eq!(third["full_model"], json!([0, 2]));
    assert_eq!(table.len(), 3);
}

fn json_string_sample() -> String {
    let text: String = (0..32)
        .map(|value| char::from_u32(value).unwrap())
        .collect();
    format!("{text}\"\\λ🦀")
}

fn value_model() -> Model {
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
    Model::new([Atom::new(
        Predicate::with_sign("p", 6, Sign::Negative).unwrap(),
        vec![
            Value::Infimum,
            Value::Supremum,
            Value::Number(i32::MIN),
            Value::Symbol("#inf".into()),
            Value::String(json_string_sample()),
            structured,
        ],
    )
    .unwrap()])
}

fn string_observation() -> ObservationFixture {
    ObservationFixture::plain(Model::new([atom("p", vec![Value::String("\n\"λ".into())])]))
}

fn nested_observation() -> ObservationFixture {
    ObservationFixture::with_directives(Model::new([]), "#show f(g(1),(2,)).")
}

#[test]
fn model_view_borrows_full_identity() {
    let fixture = ObservationFixture::plain(value_model());
    let view = fixture.view();
    assert!(std::ptr::eq(
        view.model(),
        std::ptr::from_ref(&fixture.model)
    ));
}

#[test]
fn full_atoms_preserve_term_identity() {
    let fixture = ObservationFixture::plain(value_model());
    let value = fixture.json();
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
        atom["arguments"][5],
        json!([
            {"kind":"function","name":"f","sign":"negative","arity":2},
            {"kind":"tuple","arity":1},{"kind":"number","value":7},{"kind":"tuple","arity":0}
        ])
    );
}

#[test]
fn json_strings_preserve_control_characters() {
    let fixture = ObservationFixture::plain(value_model());
    assert_eq!(
        fixture.json()["full_model"][0]["arguments"][4][0],
        json!({"kind":"string","value":json_string_sample()})
    );
}

#[test]
fn default_selection_shows_all_atoms() {
    let fixture = ObservationFixture::plain(value_model());
    assert_eq!(fixture.view().shown_atoms().count(), 1);
    assert_eq!(fixture.json()["shown"]["atom_indices"], json!([0]));
}

#[test]
fn empty_observation_yields_no_terms() {
    let fixture = ObservationFixture::plain(value_model());
    assert!(fixture.view().shown_terms().is_empty());
}

#[test]
fn absent_score_has_no_cost_vector() {
    let fixture = ObservationFixture::plain(value_model());
    assert!(fixture.json()["costs"].is_null());
}

#[test]
fn shown_channels_retain_independent_identity() {
    let fixture = ObservationFixture::with_directives(
        Model::new([atom("a", vec![]), atom("hidden", vec![])]),
        "a. #show a. #show f((1,),()).",
    );
    let view = fixture.view();
    assert_eq!(view.model().atoms().len(), 2);
    assert_eq!(view.shown_atoms().count(), 2);
    assert_eq!(view.shown_terms().len(), 2);
    assert!(
        matches!(&view.shown_terms()[0], Symbol::Function { name, sign: SymbolSign::Positive, .. } if name.as_str() == "a")
    );
    assert_eq!(
        fixture.json()["shown"]["terms"][0],
        json!([{"kind":"symbol","value":"a"}])
    );
}

#[test]
fn hidden_selection_preserves_full_identity() {
    let fixture = ObservationFixture::with_directives(
        Model::new([atom("a", vec![]), atom("hidden", vec![])]),
        "#show.",
    );
    let view = fixture.view();
    assert_eq!(view.model().atoms(), fixture.model.atoms());
    assert_eq!(view.shown_atoms().count(), 0);
}

#[test]
fn record_byte_limit_is_inclusive() {
    let fixture = string_observation();
    let view = fixture.view();
    let expected = view.json(ViewLimits::default(), &fixture.control).unwrap();
    for maximum in [0, expected.len() - 1] {
        assert_eq!(
            view.json(
                ViewLimits {
                    max_bytes: maximum,
                    ..Default::default()
                },
                &fixture.control
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
            &fixture.control
        )
        .unwrap(),
        expected
    );
}

#[test]
fn encoding_work_limit_refuses() {
    let fixture = string_observation();
    assert_eq!(
        fixture.view().json(
            ViewLimits {
                max_work: 0,
                ..Default::default()
            },
            &fixture.control
        ),
        Err(ViewError::Work)
    );
}

#[test]
fn encoding_depth_limit_refuses() {
    let fixture = string_observation();
    assert_eq!(
        fixture.view().json(
            ViewLimits {
                max_depth: 0,
                ..Default::default()
            },
            &fixture.control
        ),
        Err(ViewError::Depth)
    );
}

#[test]
fn cancelled_encoding_refuses() {
    let fixture = string_observation();
    let view = fixture.view();
    fixture.control.cancel();
    assert_eq!(
        view.json(ViewLimits::default(), &fixture.control),
        Err(ViewError::Stopped(zetesis_cpu::Stop::Cancelled))
    );
}

#[test]
fn shown_terms_use_preorder_nodes() {
    let fixture = nested_observation();
    assert_eq!(
        fixture.json()["shown"]["terms"][0],
        json!([
            {"kind":"function","name":"f","sign":"positive","arity":2},
            {"kind":"function","name":"g","sign":"positive","arity":1},
            {"kind":"number","value":1},{"kind":"tuple","arity":1},{"kind":"number","value":2}
        ])
    );
}

#[test]
fn shown_term_depth_limit_is_inclusive() {
    let fixture = nested_observation();
    let view = fixture.view();
    assert_eq!(
        view.json(
            ViewLimits {
                max_depth: 2,
                ..Default::default()
            },
            &fixture.control
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
                &fixture.control,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value, fixture.json());
}

#[test]
fn detailed_encoding_preserves_json_bytes() {
    let fixture = nested_observation();
    let view = fixture.view();
    let observations = view.observation_statistics();
    let encoded = view
        .encode_json(ViewLimits::default(), &fixture.control)
        .unwrap();
    assert_eq!(
        encoded.text(),
        view.json(ViewLimits::default(), &fixture.control).unwrap()
    );
    assert_eq!(encoded.statistics().buffered_bytes, encoded.text().len());
    assert!(encoded.statistics().work > 0);
    assert_eq!(view.observation_statistics(), observations);
    assert_eq!(view.statistics(), observations);
    assert_eq!(zetesis_themelios::observation::json::SCHEMA_VERSION, 1);
}

#[test]
fn encoding_refusal_retains_discarded_accounting() {
    use zetesis_themelios::observation::json;
    let fixture = string_observation();
    let view = fixture.view();
    let complete = view
        .encode_json(json::Limits::default(), &fixture.control)
        .unwrap();
    let mut preceding_work = 0;
    for ceiling in 0..complete.text().len() {
        let failure = view
            .encode_json(
                json::Limits {
                    max_bytes: ceiling,
                    ..Default::default()
                },
                &fixture.control,
            )
            .unwrap_err();
        assert_eq!(failure.cause(), json::Error::Bytes);
        assert!(failure.statistics().buffered_bytes <= ceiling);
        assert!(failure.statistics().work >= preceding_work);
        assert!(failure.statistics().work <= complete.statistics().work);
        preceding_work = failure.statistics().work;
    }
}

#[test]
fn encoding_work_accounting_is_inclusive() {
    use zetesis_themelios::observation::json;
    let fixture = nested_observation();
    let view = fixture.view();
    let complete = view
        .encode_json(json::Limits::default(), &fixture.control)
        .unwrap();
    let work = complete.statistics().work;
    let exact = view
        .encode_json(
            json::Limits {
                max_work: work,
                ..Default::default()
            },
            &fixture.control,
        )
        .unwrap();
    assert_eq!(exact.text(), complete.text());
    assert_eq!(exact.statistics(), complete.statistics());
    let failure = view
        .encode_json(
            json::Limits {
                max_work: work - 1,
                ..Default::default()
            },
            &fixture.control,
        )
        .unwrap_err();
    assert_eq!(failure.cause(), json::Error::Work);
    assert!(failure.statistics().work < work);
    assert!(failure.statistics().buffered_bytes < complete.text().len());
}

#[test]
fn cancelled_encoding_has_zero_accounting() {
    use zetesis_themelios::observation::json;
    let fixture = string_observation();
    let view = fixture.view();
    fixture.control.cancel();
    let failure = view
        .encode_json(json::Limits::default(), &fixture.control)
        .unwrap_err();
    assert_eq!(
        failure.cause(),
        json::Error::Stopped(zetesis_cpu::Stop::Cancelled)
    );
    assert_eq!(failure.statistics(), json::Statistics::default());
}

#[test]
fn depth_refusal_retains_encoding_work() {
    use zetesis_themelios::observation::json;
    let fixture = nested_observation();
    let failure = fixture
        .view()
        .encode_json(
            json::Limits {
                max_depth: 1,
                ..Default::default()
            },
            &fixture.control,
        )
        .unwrap_err();
    assert_eq!(failure.cause(), json::Error::Depth);
    assert!(failure.statistics().work > 0);
    assert!(failure.statistics().buffered_bytes > 0);
}
