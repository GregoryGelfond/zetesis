//! Finite observation bindings retain each original comparison's shared values.

use crate::support::observation_reference;

use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, check};
use zetesis_reference_support::formula;
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

const EXTREMA: &[(&str, &[&str])] = &[
    ("#show. #show X:f(X)=#min{f(1)}.", &["1"]),
    ("#show. #show X:#min{f(1)}=f(X).", &["1"]),
    ("#show. #show X:f(X)=#max{f(1);f(2)}.", &["2"]),
    ("#show. #show (X,Y):(X,Y)=#min{(1,2);(2,1)}.", &["(1,2)"]),
    ("#show. #show X: -f(X)=#min{-f(1)}.", &["1"]),
    ("#show. #show (X,Y):f(X)=#min{f(1)}=f(Y).", &["(1,1)"]),
    ("#show. #show X:f(X)=#min{f(1)}<f(X+1).", &["1"]),
    ("#show. #show X:f(X)=#min{f(1)}>f(X+1).", &[]),
    ("#show. #show X:g(0)>#min{f(1)}=f(X).", &["1"]),
    ("#show. #show X:g(0)<#min{f(1)}=f(X).", &[]),
    ("#show. #show X:f(X)=#min{g(1)}.", &[]),
    ("#show. #show X:f(X)=#min{}.", &[]),
    ("#show. #show X:f(X)=#max{}.", &[]),
    ("#show. #show X:f(X,X)=#min{f(1,2)}.", &[]),
    ("#show. #show X:f(X,X+1)=#min{f(1,2)}.", &["1"]),
    ("p(1).p(2). #show. #show X:f(X)=#min{f(Y):p(Y)}.", &["1"]),
];

fn display(input: &AdmittedFormula, model: &Model) -> Vec<String> {
    input
        .metadata()
        .observations()
        .render(
            model,
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .text()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn finite_equalities_retain_their_complete_guards() {
    for (source, expected) in CHAINS.iter().chain(STRUCTURES).chain(EXTREMA) {
        let input = formula(source);
        assert_eq!(
            display(
                &input,
                &Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap()
            ),
            *expected,
            "{source}"
        );
    }
}

#[test]
fn equality_queries_preserve_the_original_formula() {
    let base = "{hidden}. p(1).p(2). #minimize{1@3:hidden}.";
    let original = formula(base);
    for (query, _) in CHAINS.iter().chain(STRUCTURES).chain(EXTREMA) {
        let shown = formula(&format!("{base}{query}"));
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
    let input = formula(source);
    let family = family(&input);
    assert_eq!(family.len(), 2);
    assert_eq!(family[0].0, Model::default());
    assert_eq!(
        family[1].0,
        Model::from_positions(input.atom_catalog(), 0..input.atoms().len()).unwrap()
    );
    for (_, shown) in family {
        assert_eq!(shown, expected);
    }
}

fn family(input: &AdmittedFormula) -> Vec<(Model, Vec<String>)> {
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
            &Cancellation::default(),
        )
        .unwrap()
        .accepted()
        {
            let model = Model::from_positions(input.atom_catalog(), candidate.atoms()).unwrap();
            family.push((model.clone(), display(input, &model)));
        }
    }
    family
}

const EXTREMUM_FAMILY: &str = "{p(1);p(2)}. #show. #show X:f(X)=#min{f(Y):p(Y)}.";

#[test]
fn extremum_capture_uses_each_original_model() {
    let actual = family(&formula(EXTREMUM_FAMILY));
    let expected: &[(&[i32], &[&str])] = &[
        (&[], &[]),
        (&[1], &["1"]),
        (&[2], &["2"]),
        (&[1, 2], &["1"]),
    ];
    assert_eq!(actual.len(), expected.len());
    for (numbers, shown) in expected {
        let model = Model::new(numbers.iter().map(|number| {
            Atom::new(
                Predicate::new("p", 1).unwrap(),
                vec![Value::Number(*number)],
            )
            .unwrap()
        }))
        .unwrap();
        let shown = shown.iter().map(|value| (*value).to_owned()).collect();
        assert!(actual.contains(&(model, shown)));
    }
}

#[test]
fn equality_evaluation_obeys_its_exact_work_limit() {
    for source in [
        "#show. #show (X,Y):0<X=(1;2)=Y<3.",
        "#show. #show X:f(X,X)=f((1;2),2).",
        "#show. #show X:f(X)=#min{f(1);f(2)}.",
    ] {
        assert_exact_work(source);
    }
}

fn assert_exact_work(source: &str) {
    let input = formula(source);
    let run = |max_work| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_work,
                ..Limits::default()
            },
            &Cancellation::default(),
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
    let input = formula("#show. #show X:X=Y=1.");
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Cancellation::default(),
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
    for (source, bytes) in [
        ("#show. #show X:f(X)=f(1).", 49),
        ("#show. #show X:f(X)=#min{f(1)}.", 82),
    ] {
        assert_complete_value_budget(source, bytes);
    }
}

fn assert_complete_value_budget(source: &str, bytes: usize) {
    let input = formula(source);
    let run = |max_local_bytes| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_local_bytes,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
    };
    // Each owned f(1) is two nodes and one name byte; X owns one number.
    // The extremum binder retains its result before the match takes its copy.
    assert_eq!(run(bytes).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(bytes - 1).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            ..
        }
    ));
}

#[test]
fn mismatched_structural_choices_release_partial_captures() {
    let input = formula("#show. #show X:f(X,X)=f((1;2),2).");
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
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result.symbols(), &[Symbol::Number(2)]);
}

#[test]
fn structural_binding_reserves_its_complete_value_slot() {
    for (source, slots) in [
        ("#show. #show X:f(X)=f(1).", 2),
        ("#show. #show X:f(X)=#min{f(1)}.", 3),
    ] {
        assert_complete_value_slot(source, slots);
    }
}

fn assert_complete_value_slot(source: &str, slots: u32) {
    let run = |max_variables| {
        admit_formula(
            source.into(),
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
    assert_eq!(display(&run(slots).unwrap(), &Model::default()), ["1"]);
    let Err(FormulaFailure::Observation { error }) = run(slots - 1) else {
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
        ("#show. #show X:f(X)=#min{f(1)}.", 12),
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
    for source in [
        "#show ok. #show X:f(X,1/0)=f(1,2).",
        "#show ok. #show X:f(X,1/0)=#min{f(1,2)}.",
    ] {
        assert_undefined(source);
    }
}

fn assert_undefined(source: &str) {
    let input = formula(source);
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
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

#[test]
fn extremum_capture_counts_its_local_substitutions() {
    let input = formula("#show. #show X:f(X)=#min{f(1);f(2)}.");
    let run = |max_bindings| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_bindings,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
    };
    assert_eq!(run(3).unwrap().symbols(), &[Symbol::Number(1)]);
    assert!(matches!(
        run(2).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::Bindings,
            ..
        }
    ));
}

fn assert_binding_limit(source: &str) {
    let input = formula(source);
    let run = |max_bindings| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_bindings,
                ..Limits::default()
            },
            &Cancellation::default(),
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
    let input = formula("#show ok. #show X:X=Y=1/0.");
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert!(error.location().is_some());
}

#[test]
fn equality_generation_propagates_cancellation() {
    for source in [
        "#show. #show X:X=Y=1.",
        "#show. #show X:f(X)=f(1).",
        "#show. #show X:f(X)=#min{f(1)}.",
    ] {
        assert_cancelled(source);
    }
}

fn assert_cancelled(source: &str) {
    let input = formula(source);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let error = input
        .metadata()
        .observations()
        .evaluate(&Model::default(), Limits::default(), &cancellation)
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
        "#show. #show X:f(X)<f(3).",
        "#show. #show X:not f(X)=f(1).",
        "#show. #show X:not not f(X)=f(1).",
        "#show. #show X:f(X)=f(Y).",
        "#show. #show X:f(X)<#min{f(1)}.",
        "#show. #show X:not f(X)=#min{f(1)}.",
        "#show. #show X:not not f(X)=#min{f(1)}.",
        "#show. #show X:f(X)=#min{f(X)}.",
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

const FINITE_BINDINGS: &[(&str, &[&str])] = &[
    ("#show. #show 1:f((X;2))=f(1).", &["1"]),
    ("#show. #show X:(f(X);g(X))=f(1).", &["1"]),
    ("#show. #show X:X+1=2.", &["1"]),
    ("#show. #show X: -(-f(X))=f(1).", &["1"]),
    ("#show. #show X:f(X)=#count{}.", &[]),
    ("#show. #show X:f(X)=#sum{2147483647,a;1,b}.", &[]),
];

#[test]
fn finite_binding_shapes_produce_the_expected_displays() {
    for (source, expected) in FINITE_BINDINGS {
        let input = formula(source);
        assert_eq!(display(&input, &Model::default()), *expected, "{source}");
    }
}

#[test]
fn observation_bindings_preserve_hidden_answer_identity() {
    let singleton =
        Model::new([Atom::new(Predicate::new("hidden", 0).unwrap(), vec![]).unwrap()]).unwrap();
    for (source, _) in FINITE_BINDINGS {
        let hidden = formula(&format!("{{hidden}}. {source}"));
        let answers = family(&hidden);
        assert_eq!(answers.len(), 2, "{source}");
        assert!(
            answers.iter().any(|(model, _)| model == &Model::default()),
            "{source}"
        );
        assert!(
            answers.iter().any(|(model, _)| model == &singleton),
            "{source}"
        );
    }
}

#[test]
#[ignore = "requires clingo: finite binding shapes have complete references"]
fn finite_binding_shapes_have_complete_references() {
    for (source, expected) in FINITE_BINDINGS {
        observation_reference::compare(source, &serde_json::json!([expected]));
    }
}

#[test]
#[ignore = "requires clingo: finite equalities match complete clingo displays; exact original finite equality queries"]
fn finite_equalities_match_complete_clingo_displays() {
    for (source, expected) in CHAINS.iter().chain(STRUCTURES).chain(EXTREMA) {
        observation_reference::compare(source, &serde_json::json!([expected]));
    }
    for (source, expected) in HIDDEN {
        observation_reference::compare(source, &serde_json::json!([expected, expected]));
    }
    observation_reference::compare(
        EXTREMUM_FAMILY,
        &serde_json::json!([[], ["1"], ["2"], ["1"]]),
    );
}
