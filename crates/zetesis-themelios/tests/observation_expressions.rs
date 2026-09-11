//! Pure expression evaluation observes full models without extending logical grounding.

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_themelios::observation::{
    ConstructionLimits, ErrorKind, EvaluationError, Limits, Resource,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn terms(source: &str) -> Vec<String> {
    let input = admit(source);
    let model = Model::new(input.atoms().iter().cloned());
    input
        .metadata()
        .observations()
        .render(
            &model,
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
fn checked_integer_operators_construct_their_scalar_results() {
    assert_eq!(
        terms("p(2). #show. #show (X+3,X-3,X*3,7/X,7\\X,X**3,~X,X&3,X?4,X^3,|-X|):p(X)."),
        ["(5,-1,6,3,1,8,-3,2,6,1,2)"]
    );
}

#[test]
fn arithmetic_inside_constructors_preserves_structural_output() {
    assert_eq!(
        terms("p(2). #show. #show f(X+1,(-X,X*X)):p(X)."),
        ["f(3,(-2,4))"]
    );
}

#[test]
fn symbolic_negation_toggles_the_function_sign() {
    assert_eq!(
        terms("p(a). p(-b). p(f(1)). #show. #show -X:p(X)."),
        ["b", "-a", "-f(1)"]
    );
}

#[test]
fn comparison_chains_use_structural_term_order() {
    assert_eq!(
        terms("p(2). #show. #show x:p(X),1<X+1<f(0). #show y:not 1<2<0. #show z:not not 1<2<3."),
        ["x", "y", "z"]
    );
}

#[test]
fn boolean_default_negation_is_model_relative_truth() {
    assert_eq!(
        terms(
            "#show. #show a:#true. #show b:not #false. #show c:not not #true. #show d:#false. #show e:not #true."
        ),
        ["a", "b", "c"]
    );
}

#[test]
fn invalid_arithmetic_returns_the_pinned_cause() {
    for (expression, expected) in [
        ("1/0", EvaluationError::Undefined),
        ("1\\0", EvaluationError::Undefined),
        ("a+1", EvaluationError::Undefined),
        ("2**-1", EvaluationError::Undefined),
        ("2147483647+1", EvaluationError::Overflow),
        ("-(-2147483647-1)", EvaluationError::Overflow),
        ("|(-2147483647-1)|", EvaluationError::Overflow),
    ] {
        let input = admit(&format!("#show. #show {expression}."));
        let error = input
            .metadata()
            .observations()
            .evaluate(&Model::default(), Limits::default(), &Control::default())
            .unwrap_err();
        assert_eq!(
            error.kind(),
            &ErrorKind::Evaluation(expected),
            "{expression}"
        );
        assert!(error.location().is_some());
    }
}

#[test]
fn comparison_operands_share_the_construction_ceiling() {
    let input = admit("#show. #show x:1<2.");
    let construction = 4 * std::mem::size_of::<zetesis_themelios::observation::Symbol>();
    let run = |max_bytes| {
        input
            .metadata()
            .observations()
            .evaluate_with_construction_limits(
                &Model::default(),
                Limits::default(),
                ConstructionLimits { max_bytes },
                &Control::default(),
            )
    };
    assert_eq!(run(construction).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(construction - 1).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::ConstructionBytes,
            ..
        }
    ));
}

#[test]
fn expression_observations_preserve_the_logical_theory() {
    let source = "p(2). {q}. #minimize {1@2:q}.";
    let plain = admit(source);
    let shown = admit(&format!("{source} #show. #show f(X+10):p(X),X*2>3."));
    assert_eq!(plain.atoms(), shown.atoms());
    assert_eq!(plain.theory().nodes(), shown.theory().nodes());
    assert_eq!(plain.theory().roots(), shown.theory().roots());
}

#[test]
fn expression_observations_preserve_source_priority_presence() {
    let source = "p(2). {q}. #minimize {1@2:q}.";
    let plain = admit(source);
    let shown = admit(&format!("{source} #show. #show f(X+10):p(X),X*2>3."));
    assert_eq!(
        plain.objectives().priorities(),
        shown.objectives().priorities()
    );
}

#[test]
fn directed_equalities_are_ordered_by_their_dependencies() {
    assert_eq!(
        terms("#show. #show f(X,Y,Z):X=Y+1,Z=f(X),Y=2."),
        ["f(3,2,f(3))"]
    );
}

#[test]
fn equality_binding_accepts_the_reversed_scalar_side() {
    assert_eq!(terms("#show. #show X:2=X."), ["2"]);
}

#[test]
fn generated_bindings_obey_the_live_local_payload_limit() {
    let input = admit("#show. #show X:X=2.");
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
    assert_eq!(run(16).unwrap().symbols().len(), 1);
    assert!(matches!(
        run(15).unwrap_err().kind(),
        ErrorKind::Limit {
            resource: Resource::LocalBytes,
            observed: 16,
            ..
        }
    ));
}

#[test]
fn completed_directives_release_their_owned_bindings() {
    let input = admit("#show. #show X:X=2. #show Y:Y=3.");
    let result = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits {
                max_local_bytes: 16,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap();
    assert_eq!(result.symbols().len(), 2);
}

#[test]
fn circular_equalities_do_not_establish_a_finite_binding() {
    let result = admit_formula(
        "#show X:X=Y,Y=X.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(zetesis_themelios::FormulaFailure::Observation { error })
        if error.kind() == &ErrorKind::Unsupported(zetesis_themelios::observation::Feature::UnsafeVariable))
    );
}
