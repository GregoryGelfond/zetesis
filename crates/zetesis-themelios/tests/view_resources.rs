//! Logical observation payload depends on term identity, not input allocation.

use zetesis_core::{Atom, Model, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{ErrorKind, Limits, Resource};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn term_model(capacity: usize) -> Model {
    let mut nodes = Vec::with_capacity(capacity);
    nodes.extend([
        ValueNode::Function {
            name: "f".into(),
            sign: Sign::Positive,
            arity: 1,
        },
        ValueNode::Number(1),
    ]);
    let value = Value::from_nodes(nodes, ValueLimits::default()).unwrap();
    Model::new([Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()]).unwrap()
}

fn observation() -> zetesis_themelios::AdmittedFormula {
    admit_formula(
        "#show X:p(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

#[test]
fn logical_payload_ignores_input_capacity() {
    let admitted = observation();
    let small = term_model(2);
    let large = term_model(128);
    assert_eq!(small, large);
    let limits = Limits {
        max_symbol_bytes: 1,
        max_output_bytes: 33,
        ..Limits::default()
    };
    let cancellation = Cancellation::default();
    let first = admitted
        .metadata()
        .observations()
        .evaluate(&small, limits, &cancellation)
        .unwrap();
    let second = admitted
        .metadata()
        .observations()
        .evaluate(&large, limits, &cancellation)
        .unwrap();
    assert_eq!(first.symbols(), second.symbols());
    assert_eq!(first.statistics(), second.statistics());
}

#[test]
fn logical_text_limit_is_inclusive() {
    let admitted = observation();
    let model = term_model(128);
    let program = admitted.metadata().observations();
    let cancellation = Cancellation::default();
    assert!(
        program
            .evaluate(
                &model,
                Limits {
                    max_symbol_bytes: 1,
                    ..Limits::default()
                },
                &cancellation
            )
            .is_ok()
    );
    assert!(matches!(
        program
            .evaluate(
                &model,
                Limits {
                    max_symbol_bytes: 0,
                    ..Limits::default()
                },
                &cancellation
            )
            .unwrap_err()
            .kind(),
        ErrorKind::Limit {
            resource: Resource::Bytes,
            observed: 1,
            limit: 0
        }
    ));
}

#[test]
fn logical_output_limit_is_inclusive() {
    let admitted = observation();
    let model = term_model(128);
    let program = admitted.metadata().observations();
    let cancellation = Cancellation::default();
    assert!(
        program
            .evaluate(
                &model,
                Limits {
                    max_output_bytes: 33,
                    ..Limits::default()
                },
                &cancellation
            )
            .is_ok()
    );
    assert!(matches!(
        program
            .evaluate(
                &model,
                Limits {
                    max_output_bytes: 32,
                    ..Limits::default()
                },
                &cancellation
            )
            .unwrap_err()
            .kind(),
        ErrorKind::Limit {
            resource: Resource::OutputBytes,
            observed: 33,
            limit: 32
        }
    ));
}

#[test]
fn construction_limit_includes_conversion_stack() {
    use zetesis_themelios::observation::{ConstructionLimits, Symbol};
    let admitted = observation();
    let program = admitted.metadata().observations();
    let required = 4 * std::mem::size_of::<Symbol>() + 2;
    for capacity in [2, 128] {
        let model = term_model(capacity);
        assert!(
            program
                .evaluate_with_construction_limits(
                    &model,
                    Limits::default(),
                    ConstructionLimits {
                        max_bytes: required
                    },
                    &Cancellation::default()
                )
                .is_ok()
        );
        assert!(
            matches!(program.evaluate_with_construction_limits(&model, Limits::default(), ConstructionLimits { max_bytes: required - 1 }, &Cancellation::default()).unwrap_err().kind(), ErrorKind::Limit { resource: Resource::ConstructionBytes, observed, limit } if *observed == required as u128 && *limit == (required - 1) as u128)
        );
    }
}

#[test]
fn template_construction_uses_storage_preflight() {
    use zetesis_themelios::observation::{ConstructionLimits, Symbol};
    let admitted = admit_formula(
        "#show f(1).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let required = 4 * std::mem::size_of::<Symbol>() + 2;
    let program = admitted.metadata().observations();
    assert!(
        program
            .evaluate_with_construction_limits(
                &Model::new([]).unwrap(),
                Limits::default(),
                ConstructionLimits {
                    max_bytes: required
                },
                &Cancellation::default()
            )
            .is_ok()
    );
    assert!(matches!(
        program
            .evaluate_with_construction_limits(
                &Model::new([]).unwrap(),
                Limits::default(),
                ConstructionLimits {
                    max_bytes: required - 1
                },
                &Cancellation::default()
            )
            .unwrap_err()
            .kind(),
        ErrorKind::Limit {
            resource: Resource::ConstructionBytes,
            ..
        }
    ));
}

#[test]
fn rendered_atoms_ignore_input_capacity() {
    let program = zetesis_themelios::observation::ObservationProgram::default();
    let selection = zetesis_themelios::AtomSelection::all();
    let limits = Limits {
        max_symbol_bytes: 1,
        ..Limits::default()
    };
    let first = program
        .render(&term_model(2), &selection, limits, &Cancellation::default())
        .unwrap();
    let second = program
        .render(
            &term_model(128),
            &selection,
            limits,
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(first.text(), "p(f(1))");
    assert_eq!(second.text(), first.text());
    assert_eq!(second.statistics(), first.statistics());
}

#[test]
fn model_view_respects_construction_refusal() {
    use zetesis_themelios::observation::ConstructionLimits;
    let admitted = observation();
    let model = term_model(2);
    let selection = zetesis_themelios::AtomSelection::all();
    let result = admitted
        .metadata()
        .observations()
        .view_with_construction_limits(
            &model,
            &selection,
            None,
            Limits::default(),
            ConstructionLimits { max_bytes: 0 },
            &Cancellation::default(),
        );
    let Err(error) = result else {
        panic!("construction cannot fit");
    };
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::ConstructionBytes,
            limit: 0,
            ..
        }
    ));
    assert_eq!(error.statistics().bindings, 1);
}

#[test]
fn rendered_observations_respect_storage_refusal() {
    use zetesis_themelios::observation::ConstructionLimits;
    let admitted = observation();
    let error = admitted
        .metadata()
        .observations()
        .render_with_construction_limits(
            &term_model(2),
            &zetesis_themelios::AtomSelection::all(),
            Limits::default(),
            ConstructionLimits { max_bytes: 0 },
            &Cancellation::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::ConstructionBytes,
            limit: 0,
            ..
        }
    ));
    assert_eq!(error.statistics().bindings, 1);
}

#[test]
fn construction_covers_name_validation_copy() {
    use zetesis_themelios::observation::{ConstructionLimits, Symbol};
    let admitted = observation();
    let name = "a".repeat(1_024);
    let required = 2 * std::mem::size_of::<Symbol>() + 2 * name.len();
    let old_bound = required - name.len();
    let model = Model::new([
        Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Symbol(name)]).unwrap(),
    ])
    .unwrap();
    let program = admitted.metadata().observations();
    let result = program.evaluate_with_construction_limits(
        &model,
        Limits::default(),
        ConstructionLimits {
            max_bytes: old_bound,
        },
        &Cancellation::default(),
    );
    assert!(
        matches!(result.unwrap_err().kind(), ErrorKind::Limit { resource: Resource::ConstructionBytes, observed, .. } if *observed == required as u128)
    );
    assert!(
        program
            .evaluate_with_construction_limits(
                &model,
                Limits::default(),
                ConstructionLimits {
                    max_bytes: required
                },
                &Cancellation::default()
            )
            .is_ok()
    );
}
