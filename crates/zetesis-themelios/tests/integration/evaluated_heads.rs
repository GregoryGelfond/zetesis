//! Evaluated heads retain original grouping, atom identities and frozen reducts.

use std::collections::BTreeSet;
use std::fs::{self};
use std::time::Duration;

use serde_json::{Value as Json, json};
use zetesis_core::Sign;
use zetesis_ferraris::{Node, Theory};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, BundleAdmissionOptions, BundleLimits,
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource, ProfileFeature, SourceBundle, admit_bundle_formula, admit_formula,
};

type Names = BTreeSet<String>;
type Models = BTreeSet<Names>;

fn cases() -> Vec<Json> {
    serde_json::from_str(include_str!("../fixtures/evaluated-heads.json")).unwrap()
}
fn input(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    limited(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        &FormulaLimits::default(),
    )
}
fn limited(
    source: &str,
    options: AdmissionOptions,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(source.into(), options, expansion, *limits)
}
fn name<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let sign = if atom.predicate().sign() == Sign::Negative {
        "-"
    } else {
        ""
    };
    if atom.values().is_empty() {
        return format!("{sign}{}", atom.predicate().name());
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Infimum => "#inf".to_owned(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".to_owned(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
            zetesis_core::ValueNodeRef::Number(number) => number.to_string(),
            zetesis_core::ValueNodeRef::String(string) => serde_json::to_string(string).unwrap(),
            zetesis_core::ValueNodeRef::Symbol(symbol) => symbol.to_owned(),
        })
        .collect();
    format!("{sign}{}({})", atom.predicate().name(), values.join(","))
}
fn names(value: &Json) -> Names {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}
fn expected(value: &Json) -> Models {
    value.as_array().unwrap().iter().map(names).collect()
}
// Independent topological evaluation: every subtree false in M is falsum in
// F^M. No production evaluator, reduct mask, SAT search or subset enumeration
// helper is used to decide these finite stable models.
fn values(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut result = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let value = match *node {
            Node::False => false,
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(left, right) => result[left] && result[right],
            Node::Or(left, right) => result[left] || result[right],
            Node::Implies(left, right) => !result[left] || result[right],
        };
        result.push(value && frozen.is_none_or(|outer| outer[index]));
    }
    result
}
fn holds(theory: &Theory, values: &[bool]) -> bool {
    theory.roots().iter().all(|&root| values[root])
}
fn selected(admitted: &AdmittedFormula, mask: usize) -> Names {
    admitted
        .atoms()
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, atom)| name(atom))
        .collect()
}
fn complete(admitted: &AdmittedFormula) -> Models {
    assert!(admitted.atoms().len() <= 6, "tiny exhaustive carrier");
    let mut result = Models::new();
    for mask in 0..1_usize << admitted.atoms().len() {
        let outer = values(admitted.theory(), mask, None);
        if !holds(admitted.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                admitted.theory(),
                &values(admitted.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            assert!(result.insert(selected(admitted, mask)));
        }
    }
    result
}

#[test]
fn complete_models_match_explicit_source_expansions_and_independent_expected_data() {
    let mut model_count = 0;
    for case in cases() {
        let label = case["name"].as_str().unwrap();
        let admitted = input(case["source"].as_str().unwrap())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        let expanded = input(case["expanded"].as_str().unwrap()).unwrap();
        let predicted = expected(&case["models"]);
        assert_eq!(complete(&admitted), predicted, "{label}: original");
        assert_eq!(
            complete(&expanded),
            predicted,
            "{label}: explicit expansion"
        );
        assert_eq!(
            admitted.formula_origins().len(),
            admitted.theory().roots().len()
        );
        assert!(
            admitted
                .formula_origins()
                .iter()
                .all(|origins| !origins.is_empty())
        );
        model_count += predicted.len();
    }
    assert_eq!(cases().len(), 37);
    assert_eq!(model_count, 68);
}
fn remap(
    mask: usize,
    from: zetesis_core::catalog::Atoms<'_>,
    to: zetesis_core::catalog::Atoms<'_>,
) -> usize {
    from.iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .fold(0, |bits, (_, atom)| {
            bits | (1 << to.iter().position(|other| atom == other).unwrap())
        })
}
#[test]
fn source_expansions_preserve_every_original_and_frozen_pair() {
    let mut pairs = 0;
    for case in cases() {
        let source = input(case["source"].as_str().unwrap()).unwrap();
        let expanded = input(case["expanded"].as_str().unwrap()).unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect::<BTreeSet<_>>(),
            "{}: exact carrier",
            case["name"]
        );
        assert!(source.atoms().len() <= 6);
        for mask in 0..1_usize << source.atoms().len() {
            let outer = values(source.theory(), mask, None);
            let other = values(
                expanded.theory(),
                remap(mask, source.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(source.theory(), &outer),
                holds(expanded.theory(), &other)
            );
            for inner in 0..1_usize << source.atoms().len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), inner, Some(&outer))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, source.atoms(), expanded.atoms()),
                            Some(&other)
                        )
                    ),
                    "{}: M={mask}, J={inner}",
                    case["name"]
                );
                pairs += 1;
            }
        }
    }
    assert!(pairs > 1_000);
}
// Every M-false subtree becomes falsum, including non-atomic implications.
// This evaluates JSON trees directly, without a production DAG or compiler.
fn truth(formula: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    if frozen.is_some_and(|outer| !truth(formula, outer, None)) {
        return false;
    }
    if let Some(atom) = formula.as_str() {
        return tested.contains(atom);
    }
    if formula == &Json::Bool(false) {
        return false;
    }
    let left = truth(&formula[1], tested, frozen);
    let right = truth(&formula[2], tested, frozen);
    match formula[0].as_str().unwrap() {
        "and" => left && right,
        "or" => left || right,
        "imp" => !left || right,
        other => panic!("unknown manual formula {other}"),
    }
}
fn manual_holds(theory: &Json, tested: &Names, frozen: Option<&Names>) -> bool {
    theory["roots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|root| truth(root, tested, frozen))
}
#[test]
fn cycles_choices_and_interval_distributivity_match_handwritten_frozen_formulas() {
    let examples = [
        (
            "d(1).a(X+1)|b(X+1):-d(X).a(2):-b(2).b(2):-a(2).",
            json!({"roots": ["d(1)", ["imp", "d(1)", ["or", "a(2)", "b(2)"]], ["imp", "a(2)", "b(2)"], ["imp", "b(2)", "a(2)"]]}),
        ),
        (
            "d(1).1{p(X+1..X+2):d(X)}1.",
            json!({"roots": ["d(1)", ["or", "p(2)", ["imp", "p(2)", false]], ["or", "p(3)", ["imp", "p(3)", false]], ["or", "p(2)", "p(3)"], ["imp", ["and", "p(2)", "p(3)"], false]]}),
        ),
        (
            "p(1..2)|q.",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], "q"]]}),
        ),
        (
            "p(1..2)|q(1..2).",
            json!({"roots": [["or", ["and", "p(1)", "p(2)"], ["and", "q(1)", "q(2)"]]]}),
        ),
        ("p(2..1)|q.", json!({"roots": []})),
    ];
    for (source, manual) in examples {
        let admitted = input(source).unwrap();
        for mask in 0..1_usize << admitted.atoms().len() {
            let outer_names = selected(&admitted, mask);
            let outer = values(admitted.theory(), mask, None);
            assert_eq!(
                holds(admitted.theory(), &outer),
                manual_holds(&manual, &outer_names, None)
            );
            for inner in 0..1_usize << admitted.atoms().len() {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&outer))
                    ),
                    manual_holds(&manual, &selected(&admitted, inner), Some(&outer_names)),
                    "{source}: M={mask}, J={inner}"
                );
            }
        }
    }
}

fn feature(error: &FormulaFailure, predicted: ProfileFeature) -> bool {
    matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { feature, .. })) if *feature == predicted)
}
#[test]
fn excluded_head_forms_remain_located_refusals() {
    for (source, predicted) in [
        // Nested arithmetic ranges now have complete original/frozen expansion controls in finite_pools.rs.
        ("p(a..b)|q.", ProfileFeature::Term),
        // Finite pools, including this former a(1;2)|b refusal, have dedicated tests.
        // Evaluated conditional heads share the scoped head-generation contract.
        // Evaluated signed singleton heads have model/reduct tests in negative_heads.rs.
        // Constructor choice heads are covered by finite_values.rs.
        ("{p(a..b)}.", ProfileFeature::Term),
    ] {
        let error = input(source).expect_err(source);
        assert!(feature(&error, predicted), "{source}: {error:?}");
        assert!(!error.diagnostics().is_empty());
    }
}
use crate::support::objective_boundaries;
use zetesis_clingo_support as oracle;

#[test]
fn evaluated_producers_preserve_scored_answers() {
    for source in [
        "d(1).a(2).a(X+1)|b:-d(X).#minimize{1@1:b}.",
        "d(1).a(2).a(X+1)|b:-d(X).c:-b.#minimize{1@1:c}.",
        "d(1).{a}.{p(X+1):d(X),not a}.#minimize{1:p(2)}.",
    ] {
        objective_boundaries::check(source);
    }
}
#[test]
fn generated_heads_do_not_supply_their_own_dependencies_or_hide_unsafe_slots() {
    for source in [
        "a(X+1)|b.",
        "1{p(1..N)}1.",
        "{p(X+1):not d(X)}.",
        "{p(2..1,X+1)}.",
        "d(1).a(X+1)|b(Y+1):-d(X).",
        "d(1).a(X+1)|b:-#count{X:d(X)}>=1.",
        "{p(1..2,_):d(1)}.d(1).",
    ] {
        let error = input(source).expect_err(source);
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error:?}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}
#[test]
fn evaluated_undefinedness_and_overflow_remain_whole_input_failures() {
    for (source, predicted) in [
        (
            "d(0).{p(1/X):d(X)}.",
            themelios_program::term::EvalError::Undefined,
        ),
        (
            "d(0).a(1/X)|b:-d(X).",
            themelios_program::term::EvalError::Undefined,
        ),
        (
            "n(2147483647).{p(N+1):n(N)}.",
            themelios_program::term::EvalError::Overflow,
        ),
    ] {
        let error = input(source).expect_err(source);
        assert!(
            matches!(error, FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, .. }) if error == predicted)
        );
    }
}

fn first_success(mut attempt: impl FnMut(u64) -> bool) -> u64 {
    let mut high = 1;
    while !attempt(high) {
        high *= 2;
        assert!(high <= 1_048_576, "bounded resource threshold probe");
    }
    let mut low = 0;
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if attempt(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}
#[test]
fn generated_slot_arity_and_range_width_limits_are_inclusive() {
    for (source, slots) in [
        ("d(1).a(X+1)|b(X+2):-d(X).", 3),
        ("d(1).{p(X+1..X+2,X)}:-d(X).", 2),
        ("1{p(X+1..X+2,X):X=1}1.", 2),
    ] {
        let mut options = AdmissionOptions::default();
        options.core_limits.max_variables_per_template = slots;
        assert!(
            limited(
                source,
                options,
                ExpansionLimits::default(),
                &FormulaLimits::default()
            )
            .is_ok()
        );
        options.core_limits.max_variables_per_template -= 1;
        assert!(
            matches!(limited(source, options, ExpansionLimits::default(), &FormulaLimits::default()), Err(FormulaFailure::Limit { resource: FormulaResource::Variables, observed, .. }) if observed == slots as u128)
        );
    }
    let source = "n(1).{p(N..N+1):n(N)}.";
    let limits = FormulaLimits {
        max_assignment_values: 2,
        ..FormulaLimits::default()
    };
    assert!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        )
        .is_ok()
    );
    assert!(matches!(
        limited(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &FormulaLimits {
                max_assignment_values: 1,
                ..limits
            }
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            observed: 2,
            ..
        })
    ));
    assert!(matches!(
        limited(
            "n(0).{p((N-2147483647-1)..(N+2147483647)):n(N)}.",
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            &limits
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::AssignmentValues,
            observed: 4_294_967_296,
            ..
        })
    ));
    let mut options = AdmissionOptions::default();
    options.core_limits.max_predicate_arity = 1;
    assert!(matches!(
        limited(
            "d(1).{p(X+1,X)}:-d(X).",
            options,
            ExpansionLimits::default(),
            &FormulaLimits::default()
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Arity,
            observed: 2,
            ..
        })
    ));
}
#[test]
fn construction_work_nodes_and_substitutions_fail_before_partial_admission() {
    for source in [
        "d(1).a(X+1)|b(X+2):-d(X).",
        "d(1).1{p(X..X+1,X-1..X):d(X)}1.",
        "p(1..2)|q(1..2).",
    ] {
        for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
            let attempt = |limit| {
                let mut limits = FormulaLimits::default();
                if resource == FormulaResource::Work {
                    limits.max_work = limit;
                } else {
                    limits.max_substitutions = limit;
                }
                limited(
                    source,
                    AdmissionOptions::default(),
                    ExpansionLimits::default(),
                    &limits,
                )
            };
            let threshold = first_success(|limit| attempt(limit).is_ok());
            assert!(
                matches!(attempt(threshold - 1), Err(FormulaFailure::Limit { resource: actual, observed, .. }) if actual == resource && observed == u128::from(threshold))
            );
            assert_eq!(
                complete(&attempt(threshold).unwrap()),
                complete(&input(source).unwrap())
            );
        }
        let attempt = |limit| {
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits {
                    max_term_work: usize::try_from(limit).unwrap(),
                    ..ExpansionLimits::default()
                },
                &FormulaLimits::default(),
            )
        };
        let threshold = first_success(|limit| attempt(limit).is_ok());
        assert!(matches!(
            attempt(threshold - 1),
            Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::TermWork,
                ..
            }))
        ));
        let admitted = input(source).unwrap();
        let mut limits = FormulaLimits::default();
        limits.theory.max_nodes = admitted.theory().nodes().len();
        assert!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            )
            .is_ok()
        );
        limits.theory.max_nodes -= 1;
        assert!(matches!(
            limited(
                source,
                AdmissionOptions::default(),
                ExpansionLimits::default(),
                &limits
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Nodes,
                ..
            })
        ));
    }
}
#[test]
fn generated_instances_preserve_every_duplicate_source_origin() {
    for rule in ["1{p(X+1..X+2):X=1}1.", "a(X+1)|b:-X=1."] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("entry.lp"),
            format!("#include \"other.lp\".\n{rule}"),
        )
        .unwrap();
        fs::write(directory.path().join("other.lp"), rule).unwrap();
        let bundle =
            SourceBundle::load(directory.path().join("entry.lp"), BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert!(!admitted.formula_origins().is_empty());
        for origins in admitted.formula_origins() {
            assert_eq!(
                origins
                    .iter()
                    .map(|origin| origin.source)
                    .collect::<BTreeSet<_>>()
                    .len(),
                2
            );
            for origin in origins {
                assert_eq!(
                    admitted
                        .bundle()
                        .get(origin.source)
                        .unwrap()
                        .source()
                        .slice(origin.span)
                        .unwrap(),
                    rule
                );
            }
        }
    }
}

fn clingo(source: &str) -> Models {
    let run = oracle::run(
        source,
        &["--models=0", "--outf=2"],
        oracle::Limits {
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576 + 65_536,
        },
    );
    let raw = oracle::json(&run);
    assert!(
        raw["Solver"]
            .as_str()
            .unwrap()
            .starts_with("clingo version 5.8.")
    );
    assert!(matches!(
        raw["Result"].as_str().unwrap(),
        "SATISFIABLE" | "UNSATISFIABLE"
    ));
    assert_eq!(raw["Models"]["More"], "no");
    let witnesses: Vec<_> = raw["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(
        raw["Models"]["Number"].as_u64().unwrap(),
        witnesses.len() as u64
    );
    let models: Models = witnesses
        .iter()
        .map(|witness| {
            assert!(witness["Costs"].is_null());
            names(&witness["Value"])
        })
        .collect();
    assert_eq!(
        models.len(),
        witnesses.len(),
        "fixtures show complete models without projection"
    );
    models
}
#[test]
#[ignore = "requires external clingo 5.8; each original and expansion has a bounded complete capture"]
fn fresh_clingo_original_and_expanded_sources_match_complete_models() {
    for case in cases() {
        let predicted = expected(&case["models"]);
        for field in ["source", "expanded"] {
            assert_eq!(
                clingo(case[field].as_str().unwrap()),
                predicted,
                "{}: {field}",
                case["name"]
            );
        }
    }
}
