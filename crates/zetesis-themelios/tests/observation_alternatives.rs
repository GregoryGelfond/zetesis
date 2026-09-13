//! Finite expression alternatives retain typed results and honest refusal prefixes.

use std::collections::BTreeSet;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_themelios::observation::{ErrorKind, EvaluationError, Limits, Resource};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn input(expression: &str) -> AdmittedFormula {
    admit_formula(
        format!("#show. #show {expression}."),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn display(expression: &str) -> BTreeSet<String> {
    let input = input(expression);
    input
        .metadata()
        .observations()
        .render(
            &Model::default(),
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
fn unary_operators_apply_to_each_finite_alternative() {
    for (expression, expected) in [
        ("-(1..3)", vec!["-3", "-2", "-1"]),
        ("|(-2..0)|", vec!["0", "1", "2"]),
        ("~(0;1)", vec!["-2", "-1"]),
        ("-(a;-b;f(1);-f(2))", vec!["-a", "b", "-f(1)", "f(2)"]),
    ] {
        assert_eq!(
            display(expression),
            expected.into_iter().map(str::to_owned).collect(),
            "{expression}",
        );
    }
}

#[test]
fn tuples_preserve_the_cartesian_product_of_alternatives() {
    assert_eq!(
        display("((1;2),(3;4))"),
        ["(1,3)", "(1,4)", "(2,3)", "(2,4)"]
            .map(str::to_owned)
            .into_iter()
            .collect(),
    );
}

#[test]
fn an_empty_constructor_argument_has_no_product() {
    for expression in ["f(3..1,(1;2))", "((1;2),3..1)"] {
        assert!(display(expression).is_empty(), "{expression}");
    }
}

#[test]
fn invalid_alternatives_refuse_the_complete_evaluation() {
    for (expression, cause) in [
        ("-(1;\"x\")", EvaluationError::Undefined),
        ("|(a;1)|", EvaluationError::Undefined),
        ("-(2147483647;(-2147483647-1))", EvaluationError::Overflow),
        ("|((-2147483647-1);0)|", EvaluationError::Overflow),
        ("(1;2)/(1;0)", EvaluationError::Undefined),
        ("f((1;2),1/0)", EvaluationError::Undefined),
    ] {
        let input = input(expression);
        let error = input
            .metadata()
            .observations()
            .evaluate(&Model::default(), Limits::default(), &Control::default())
            .unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::Evaluation(cause), "{expression}");
        let location = error.location().expect("authored show directive");
        assert_eq!(
            input.source().slice(location.span).unwrap(),
            format!("#show {expression}."),
        );
    }
}

#[test]
fn finite_evaluation_refuses_every_incomplete_work_prefix() {
    // Enumerating all work ceilings of these small expressions exercises refusal
    // inside collection, copying and consumption. Each retry owns fresh scratch;
    // an earlier refusal must not change the immutable compiled observation.
    for expression in ["-(a;-f(2))", "((1;2),(3;4))", "(1..2)+(3;4)"] {
        let input = input(expression);
        let observations = input.metadata().observations();
        let run = |max_work| {
            observations.evaluate(
                &Model::default(),
                Limits {
                    max_work,
                    ..Limits::default()
                },
                &Control::default(),
            )
        };
        let complete = run(Limits::default().max_work).unwrap();
        let work = complete.statistics().work;
        assert!(work > 0 && work < 10_000, "bounded prefix control");
        for limit in 0..work {
            let error = run(limit).unwrap_err();
            assert!(
                matches!(error.kind(), ErrorKind::Limit {
                    resource: Resource::Work, observed, limit: found
                } if *found == u128::from(limit) && *observed > *found),
                "{expression}, limit {limit}: {error}",
            );
            assert!(error.statistics().work <= limit);
        }
        let exact = run(work).unwrap();
        assert_eq!(exact.symbols(), complete.symbols());
        assert_eq!(exact.statistics(), complete.statistics());
    }
}

#[test]
fn tuple_products_admit_all_simultaneous_payload() {
    let input = input("((1;2),(3;4))");
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
    // Four argument numbers remain live while four three-node tuples collect.
    let required = (4 + 4 * 3) * 16;
    assert_eq!(run(required).unwrap().symbols().len(), 4);
    assert!(
        matches!(run(required - 1).unwrap_err().kind(), ErrorKind::Limit {
        resource: Resource::LocalBytes, observed, limit
    } if *observed == required as u128 && *limit == (required - 1) as u128)
    );
}

#[test]
fn a_term_ceiling_cannot_publish_a_partial_pool() {
    let input = input("(1;2;3)");
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits {
                max_terms: 1,
                ..Limits::default()
            },
            &Control::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Limit {
            resource: Resource::Terms,
            observed: 2,
            limit: 1,
        },
    );
    assert!(error.statistics().work > 0);
    assert_eq!(
        display("(1;2;3)"),
        ["1", "2", "3"].map(str::to_owned).into_iter().collect()
    );
}
