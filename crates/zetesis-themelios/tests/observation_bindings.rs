//! Finite observation bindings retain each original comparison's shared values.

#[path = "support/observation_reference.rs"]
mod observation_reference;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, check};
use zetesis_themelios::observation::{
    AdmissionLimits, ErrorKind, EvaluationError, Feature, Limits, Resource, Symbol,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    admit_formula,
};

// The first source is the unchanged original L17 refusal witness. The remaining
// sources exercise its dependency and shared-operand contract in both directions.
const CHAINS: &[(&str, &[&str])] = &[
    ("#show. #show X:X=Y=1.", &["1"]),
    ("#show. #show (X,Y):1=X=Y.", &["(1,1)"]),
    ("#show. #show (X,Y):X=Y=Z,Z=1.", &["(1,1)"]),
    ("#show. #show (X,Y):Z=1,X=Y=Z.", &["(1,1)"]),
    ("#show. #show X:0<X=Y=1<2.", &["1"]),
    ("#show. #show X:2>X=Y=1>0.", &["1"]),
    ("#show. #show X:X=Y=1<0.", &[]),
    ("#show. #show X:0<X=Y=1<1.", &[]),
    ("#show. #show (X,Y):X=1..2=Y.", &["(1,1)", "(2,2)"]),
    ("#show. #show (X,Y):X=(1;2)=Y.", &["(1,1)", "(2,2)"]),
    ("#show. #show (X,Y):X=(1;2)=Y,X!=Y.", &[]),
    ("#show. #show (X,Y):0<X=(1;2)=Y<2.", &["(1,1)"]),
    ("#show. #show X:X=Y=(2..1).", &[]),
    ("#show. #show X:X=Y=f(1+1).", &["f(2)"]),
    ("#show. #show X:X=Y=(-f(1),2).", &["(-f(1),2)"]),
    ("#show. #show N:N=#count{X:X=Y=1}.", &["1"]),
    ("p(1). #show. #show yes:p(X):X=Y=1.", &["yes"]),
    ("p(2). #show. #show yes:p(X):X=Y=1.", &[]),
];

const STRUCTURES: &[(&str, &[&str])] = &[
    ("#show. #show X:f(X)=f(1).", &["1"]),
    ("#show. #show X:f(1)=f(X).", &["1"]),
    ("#show. #show X:f(X,X)=f(1,2).", &[]),
    ("#show. #show X:f(X,X+1)=f(1,2).", &["1"]),
    ("#show. #show X:f(X+1,X)=f(2,1).", &["1"]),
    ("#show. #show X:f(X,X+1)=f(1,3).", &[]),
    ("#show. #show (X,Y,Z):f(X,(Y,Z))=f(1,(2,3)).", &["(1,2,3)"]),
    ("#show. #show (X,Y):(X,Y)=(1,2).", &["(1,2)"]),
    ("#show. #show X: -f(X)=-f(1).", &["1"]),
    ("#show. #show X: -f(X)=f(1).", &[]),
    ("#show. #show X:f(X)=g(1).", &[]),
    ("n(1). #show. #show X:f(X)=f(Y),n(Y).", &["1"]),
    ("#show. #show X:f(X)=f(1..2).", &["1", "2"]),
    ("#show. #show X:a<f(X)=f(1).", &["1"]),
    ("#show. #show (X,Y):X=f(Y)=f(1).", &["(f(1),1)"]),
    ("#show. #show X:f(X,X)=f((1;2),2).", &["2"]),
    ("#show. #show N:N=#count{X:f(X,X)=f((1;2),2)}.", &["1"]),
];

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn display(input: &AdmittedFormula, model: &Model) -> Vec<String> {
    input
        .metadata()
        .observations()
        .render(
            model,
            input.metadata().output(),
            Limits::default(),
            &Control::default(),
        )
        .unwrap()
        .text()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn finite_equalities_retain_their_complete_guards() {
    for (source, expected) in CHAINS.iter().chain(STRUCTURES) {
        let input = admit(source);
        assert_eq!(
            display(&input, &Model::new(input.atoms().iter().cloned())),
            *expected,
            "{source}"
        );
    }
}

#[test]
fn equality_queries_preserve_the_original_formula() {
    let base = "{hidden}. p(1).p(2). #minimize{1@3:hidden}.";
    let original = admit(base);
    for (query, _) in CHAINS.iter().chain(STRUCTURES) {
        let shown = admit(&format!("{base}{query}"));
        // Only the fact-free queries leave this base's logical source unchanged.
        if query.starts_with("#show.") {
            assert_eq!(shown.atoms(), original.atoms());
            assert_eq!(shown.theory().nodes(), original.theory().nodes());
            assert_eq!(shown.theory().roots(), original.theory().roots());
            assert_eq!(shown.formula_origins(), original.formula_origins());
            assert_eq!(
                shown.objectives().priorities(),
                original.objectives().priorities()
            );
        }
    }
}

const HIDDEN: &[(&str, &[&str])] = &[
    (
        "{hidden}. #show. #show (X,Y):X=(1;2)=Y.",
        &["(1,1)", "(2,2)"],
    ),
    ("{hidden}. #show. #show X:f(X)=f(1..2).", &["1", "2"]),
];

#[test]
fn equality_displays_preserve_hidden_family_multiplicity() {
    for (source, expected) in HIDDEN {
        assert_hidden_family(source, expected);
    }
}

fn assert_hidden_family(source: &str, expected: &[&str]) {
    let input = admit(source);
    let mut family = Vec::new();
    for mask in 0..1_usize << input.atoms().len() {
        let candidate = Interpretation::new(
            input.theory(),
            (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        if check(
            input.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default(),
        )
        .unwrap()
        .accepted()
        {
            let model = Model::new(candidate.atoms().map(|index| input.atoms()[index].clone()));
            family.push((model.clone(), display(&input, &model)));
        }
    }
    assert_eq!(family.len(), 2);
    assert_eq!(family[0].0, Model::default());
    assert_eq!(family[1].0, Model::new(input.atoms().iter().cloned()));
    for (_, shown) in family {
        assert_eq!(shown, expected);
    }
}

#[test]
fn equality_evaluation_obeys_its_exact_work_limit() {
    for source in [
        "#show. #show (X,Y):0<X=(1;2)=Y<3.",
        "#show. #show X:f(X,X)=f((1;2),2).",
    ] {
        assert_exact_work(source);
    }
}

fn assert_exact_work(source: &str) {
    let input = admit(source);
    let run = |max_work| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_work,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    let full = run(Limits::default().max_work).unwrap();
    let work = full.statistics().work;
    let exact = run(work).unwrap();
    assert_eq!(exact.symbols(), full.symbols());
    assert_eq!(exact.statistics(), full.statistics());
    let error = run(work - 1).unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::Work,
            ..
        }
    ));
    assert!(error.location().is_some());
}

#[test]
fn chain_bindings_obey_the_live_payload_limit() {
    let input = admit("#show. #show X:X=Y=1.");
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    assert_eq!(run(32).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(31).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            ..
        }
    ));
}

#[test]
fn structural_captures_share_the_complete_value_budget() {
    let input = admit("#show. #show X:f(X)=f(1).");
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    // f(1) is two nodes and one name byte; X owns its one captured number.
    assert_eq!(run(49).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(48).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            ..
        }
    ));
}

#[test]
fn mismatched_structural_choices_release_partial_captures() {
    let input = admit("#show. #show X:f(X,X)=f((1;2),2).");
    // Two expanded f(_,2) values, the selected source value, the complete match
    // value and one captured number coexist. A rejected capture must be released
    // before the second alternative can fit this same ceiling.
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits {
                max_local_bytes: 212,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap();
    assert_eq!(result.symbols(), &[Symbol::Number(2)]);
}

#[test]
fn structural_binding_reserves_its_complete_value_slot() {
    let run = |max_variables| {
        admit_formula(
            "#show. #show X:f(X)=f(1).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                observation: AdmissionLimits {
                    max_variables,
                    ..AdmissionLimits::default()
                },
                ..FormulaLimits::default()
            },
        )
    };
    assert_eq!(display(&run(2).unwrap(), &Model::default()), ["1"]);
    let Err(FormulaFailure::Observation { error }) = run(1) else {
        panic!("the complete match value needs its own slot");
    };
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::Variables,
            ..
        }
    ));
    assert!(error.location().is_some());
}

#[test]
fn moved_operands_charge_their_retained_guard_nodes() {
    for (source, nodes) in [
        ("#show. #show X:X=Y=1.", 8),
        ("#show. #show X:f(X)=f(1).", 9),
        ("#show. #show X:X=1..2.", 10),
    ] {
        let run = |max_nodes| {
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits {
                    observation: AdmissionLimits {
                        max_nodes,
                        ..AdmissionLimits::default()
                    },
                    ..FormulaLimits::default()
                },
            )
        };
        run(nodes).unwrap_or_else(|error| panic!("{source}: {error}"));
        let Err(FormulaFailure::Observation { error }) = run(nodes - 1) else {
            panic!("{source}: every retained guard node must be charged");
        };
        assert_eq!(
            error.kind(),
            &ErrorKind::Limit {
                resource: Resource::Nodes,
                observed: u128::from(nodes),
                limit: u128::from(nodes - 1),
            },
            "{source}",
        );
        assert!(error.location().is_some());
    }
}

#[test]
fn structural_generation_refuses_an_undefined_consumer() {
    let input = admit("#show ok. #show X:f(X,1/0)=f(1,2).");
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &Control::default())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert!(error.location().is_some());
}

#[test]
fn equality_bindings_obey_completed_substitution_limits() {
    for source in ["#show. #show X:X=Y=1..2.", "#show. #show X:f(X)=f(1..2)."] {
        assert_binding_limit(source);
    }
}

fn assert_binding_limit(source: &str) {
    let input = admit(source);
    let run = |max_bindings| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_bindings,
                ..Limits::default()
            },
            &Control::default(),
        )
    };
    assert_eq!(run(2).unwrap().symbols().len(), 2);
    assert!(matches!(
        run(1).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::Bindings,
            ..
        }
    ));
}

#[test]
fn chain_generation_propagates_arithmetic_failure() {
    let input = admit("#show ok. #show X:X=Y=1/0.");
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &Control::default())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert!(error.location().is_some());
}

#[test]
fn equality_generation_propagates_cancellation() {
    for source in ["#show. #show X:X=Y=1.", "#show. #show X:f(X)=f(1)."] {
        assert_cancelled(source);
    }
}

fn assert_cancelled(source: &str) {
    let input = admit(source);
    let control = Control::default();
    control.cancel();
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &control)
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Stopped(zetesis_cpu::Stop::Cancelled)
    ));
}

#[test]
fn nongenerating_edges_cannot_establish_bindings() {
    for source in [
        "#show. #show X:0<X<3.",
        "#show. #show X:not X=Y=1.",
        "#show. #show X:not not X=Y=1.",
        "#show. #show X:X=Y=Z.",
        "#show. #show X:X=Y+1=2.",
        "#show. #show X:f(X)<f(3).",
        "#show. #show X:not f(X)=f(1).",
        "#show. #show X:not not f(X)=f(1).",
        "#show. #show X:f(X)=f(Y).",
    ] {
        let Err(FormulaFailure::Observation { error }) = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        ) else {
            panic!("expected unavailable binding: {source}");
        };
        assert_eq!(
            error.kind(),
            &ErrorKind::Unsupported(Feature::UnsafeVariable)
        );
        assert!(error.location().is_some());
    }
}

#[test]
#[ignore = "requires absolute CLINGO; exact original finite equality queries"]
fn finite_equalities_match_complete_clingo_displays() {
    for (source, expected) in CHAINS.iter().chain(STRUCTURES) {
        observation_reference::compare(source, &serde_json::json!([expected]));
    }
    for (source, expected) in HIDDEN {
        observation_reference::compare(source, &serde_json::json!([expected, expected]));
    }
}
