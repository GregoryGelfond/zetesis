//! Finite observation bindings retain each original comparison's shared values.

#[path = "support/observation_reference.rs"]
mod observation_reference;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, check};
use zetesis_themelios::observation::{ErrorKind, EvaluationError, Feature, Limits, Resource};
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
fn finite_equality_chains_retain_their_complete_guards() {
    for (source, expected) in CHAINS {
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
    for (query, _) in CHAINS {
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

const HIDDEN: &str = "{hidden}. #show. #show (X,Y):X=(1;2)=Y.";

#[test]
fn equality_displays_preserve_hidden_family_multiplicity() {
    let input = admit(HIDDEN);
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
        assert_eq!(shown, ["(1,1)", "(2,2)"]);
    }
}

#[test]
fn chain_evaluation_obeys_its_exact_work_limit() {
    let input = admit("#show. #show (X,Y):0<X=(1;2)=Y<3.");
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
fn chain_bindings_obey_completed_substitution_limits() {
    let input = admit("#show. #show X:X=Y=1..2.");
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
fn chain_generation_propagates_cancellation() {
    let input = admit("#show. #show X:X=Y=1.");
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
#[ignore = "requires absolute CLINGO; exact original equality-chain queries"]
fn equality_chains_match_complete_clingo_displays() {
    for (source, expected) in CHAINS {
        observation_reference::compare(source, &serde_json::json!([expected]));
    }
    observation_reference::compare(
        HIDDEN,
        &serde_json::json!([["(1,1)", "(2,2)"], ["(1,1)", "(2,2)"],]),
    );
}
