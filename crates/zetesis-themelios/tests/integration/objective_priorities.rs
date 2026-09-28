//! Finite priorities partition complete eligible objective rows.

use crate::support::source_cases;
use crate::support::source_oracle;
use crate::support::source_records;

use source_records::{admit, exhaustive};
use zetesis_themelios::{
    AdmissionFailure, ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource,
    ProfileFeature,
};

const CASES: &str = r##"
{"name":"priority_only_variable","source":"{p(1);p(2)}.#minimize{1@X:p(X)}.","priorities":[2,1],"records":[[[],[0,0]],[["p(1)"],[0,1]],[["p(2)"],[1,0]],[["p(1)","p(2)"],[1,1]]]}
{"name":"shared_numeric_row","source":"p(2,7;3,1).#minimize{W@P:p(W,P)}.","priorities":[7,1],"records":[[["p(2,7)","p(3,1)"],[2,3]]]}
{"name":"priority_expression","source":"{p(1);p(2)}.#minimize{1@(X+1):p(X)}.","priorities":[3,2],"records":[[[],[0,0]],[["p(1)"],[0,1]],[["p(2)"],[1,0]],[["p(1)","p(2)"],[1,1]]]}
{"name":"duplicate_priority_key","source":"p(1;2).#minimize{3@(X-X),k:p(X);3@0,k:p(1)}.","priorities":[0],"records":[[["p(1)","p(2)"],[3]]]}
{"name":"empty_binding_carrier","source":"#minimize{1@X:p(X)}.","priorities":[],"records":[[[],null]]}
{"name":"zero_priority_cost","source":"{p(7)}.#minimize{0@X:p(X)}.","priorities":[7],"records":[[[],[0]],[["p(7)"],[0]]]}
{"name":"cancelled_priority_cost","source":"p(7).#minimize{2@X,k:p(X);-2@X,k:p(X)}.","priorities":[7],"records":[[["p(7)"],[0]]]}
{"name":"maximized_priority","source":"{p(1);p(2)}.#maximize{X@X:p(X)}.","priorities":[2,1],"records":[[[],[0,0]],[["p(1)"],[0,-1]],[["p(2)"],[-2,0]],[["p(1)","p(2)"],[-2,-1]]]}
{"name":"weak_priority","source":"p(2,7;3,1).:~p(W,P).[W@P]","priorities":[7,1],"records":[[["p(2,7)","p(3,1)"],[2,3]]]}
{"name":"nonnumeric_priority","source":"p(foo;2).#minimize{1@X:p(X)}.","priorities":[2],"records":[[["p(foo)","p(2)"],[1]]]}
{"name":"excluded_priority","source":"p(foo).#minimize{1@X:p(X)}.","priorities":[],"records":[[["p(foo)"],null]]}
{"name":"literal_priority","source":"{a}.#minimize{1@foo:a;1@#inf:a;1@#sup:a}.","priorities":[],"records":[[[],null],[["a"],null]]}
{"name":"correlated_numeric_presence","source":"p(foo,7;2,foo).#minimize{W@P:p(W,P)}.","priorities":[],"records":[[["p(foo,7)","p(2,foo)"],null]]}
{"name":"filtered_priority","source":"p(1;2).#minimize{1@X:p(X),X!=1}.","priorities":[2],"records":[[["p(1)","p(2)"],[1]]]}
{"name":"excluded_maximize_normalization","source":"#maximize{(-2147483647-1)@word}.","priorities":[],"records":[[[],null]]}
{"name":"excluded_bound_normalization","source":"p(-2147483647-1,word;1,2).#maximize{W@P:p(W,P)}.","priorities":[2],"records":[[["p(-2147483648,word)","p(1,2)"],[-1]]]}
{"name":"constructed_priority","source":"p(1).#minimize{1@f(X):p(X);1@(X,2):p(X)}.","priorities":[],"records":[[["p(1)"],null]]}
{"name":"negative_priority_order","source":"p(-1;2).#minimize{1@X:p(X)}.","priorities":[2,-1],"records":[[["p(-1)","p(2)"],[1,1]]]}
"##;

#[test]
fn completed_priorities_preserve_scored_answers() {
    let raw: Vec<serde_json::Value> = CASES
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for (case, raw) in source_cases::cases(CASES.trim()).into_iter().zip(raw) {
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            raw["priorities"],
            "{}",
            case.name
        );
    }
}

#[test]
fn priority_variables_require_positive_bindings() {
    for source in ["#minimize{1@X}.", "a.#minimize{foo@X:a}."] {
        assert!(matches!(
            admit(source, &FormulaLimits::default()),
            Err(FormulaFailure::UnsafeVariable { .. })
        ));
    }
}

#[test]
fn undefined_priorities_remain_errors() {
    for source in [
        "p(0).#minimize{1@(1/X):p(X)}.",
        "p(0).#minimize{foo@(1/X):p(X)}.",
        "p(2147483647).#minimize{1@(X+1):p(X)}.",
    ] {
        assert!(matches!(
            admit(source, &FormulaLimits::default()),
            Err(FormulaFailure::Expansion(
                ExpansionFailure::Evaluation { .. }
            ))
        ));
    }
}

#[test]
fn numeric_priorities_retain_normalization_errors() {
    let source = "p(-2147483647-1,word;-2147483647-1,2).#maximize{W@P:p(W,P)}.";
    assert!(matches!(
        admit(source, &FormulaLimits::default()),
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Profile {
                feature: ProfileFeature::NumericOverflow,
                ..
            }
        )))
    ));
}

#[test]
fn specialization_preserves_the_original_theory() {
    let source = "{p(1);p(2)}.";
    let ordinary = admit(source, &FormulaLimits::default()).unwrap();
    let optimized = admit(
        &format!("{source}#minimize{{1@X:p(X)}}."),
        &FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(optimized.atoms(), ordinary.atoms());
    assert_eq!(optimized.theory().nodes(), ordinary.theory().nodes());
    assert_eq!(optimized.theory().roots(), ordinary.theory().roots());
    assert_eq!(optimized.formula_origins(), ordinary.formula_origins());
}

#[test]
fn specialization_retains_objective_origins() {
    let source = "p(1;2).\n#minimize{1@X:p(X)}.";
    let input = admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.objectives().templates().len(), 2);
    assert_eq!(input.objective_origins().len(), 2);
    assert_eq!(input.objective_origins()[0], input.objective_origins()[1]);
    assert!(input.objective_origins()[0].iter().any(|location| {
        input.source().slice(location.span).unwrap() == "#minimize{1@X:p(X)}."
    }));
}

#[test]
fn specialization_limits_are_inclusive() {
    let source = "p(1;2).#minimize{1@X:p(X)}.";
    let mut limits = FormulaLimits::default();
    limits.objective.max_templates = 1;
    let error = admit(source, &limits).unwrap_err();
    let FormulaFailure::Limit {
        resource: FormulaResource::ObjectiveElements,
        limit: 1,
        observed: 2,
        location,
    } = error
    else {
        panic!("unexpected specialization boundary: {error}");
    };
    limits.objective.max_templates = 2;
    let input = admit(source, &limits).unwrap();
    assert_eq!(input.objectives().templates().len(), 2);
    assert_eq!(location.source, input.source().id());
    assert_eq!(
        input.source().slice(location.span).unwrap(),
        "#minimize{1@X:p(X)}."
    );
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn priority_sources_match_complete_clingo_records() {
    for case in source_cases::cases(CASES.trim()) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
