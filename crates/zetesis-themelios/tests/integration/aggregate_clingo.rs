//! Aggregate reference cases include recursive formulas whose classical truth
//! tables agree but whose reducts have different subset models.
//!
//! The fixture contains 330 sources: 321 admitted comparisons/assignments and
//! nine explicit refusals, including two unsafe programs. The portable test
//! compares every admission with independent exhaustive reduct enumeration;
//! the optional test reruns the full external reference campaign. A recorded
//! unsupported case is a refusal regression, never a compatibility pass.
//! Formal references: Ferraris (arXiv:0812.1462),
//! Proposition 12; Abstract Gringo (arXiv:1507.06576), equation 22 and Theorem 1.

use crate::support::atom_models::atom;
use crate::support::finite_bindings::Models;
use crate::support::objective_dependency_records as objective_dependencies;
use zetesis_reference_support::{admit, formula, resource_limits};

use std::collections::BTreeSet;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;
use zetesis_core::{Atom, Model};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AggregateErrorKind, Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmittedFormula, ExpansionFailure, FormulaFailure, FormulaLimits,
    FormulaResource, ProfileFeature,
};

struct Case {
    name: String,
    source: String,
    valid: bool,
    models: Models,
    costs: Option<Vec<i64>>,
}

fn models(items: &Json) -> Models {
    items
        .as_array()
        .expect("recorded complete models")
        .iter()
        .map(oracle::atoms)
        .collect()
}

fn cases() -> Vec<Case> {
    include_str!("../fixtures/aggregate-campaign.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded campaign row");
            Case {
                name: row[0].as_str().expect("stable case name").to_owned(),
                source: row[1].as_str().expect("source program").to_owned(),
                valid: row[2].as_bool().expect("source validity"),
                models: models(&row[3]),
                costs: oracle::costs(&row[4]),
            }
        })
        .collect()
}

fn exhaustive(input: &AdmittedFormula) -> (BTreeSet<BTreeSet<Atom>>, Option<Vec<i64>>) {
    let count = input.atoms().len();
    assert!(count <= 16, "small independent exhaustive carrier");
    let cancellation = Cancellation::default();
    let mut best: Option<Vec<i64>> = None;
    let mut models = BTreeSet::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if !check(input.theory(), &candidate, Limits::default(), &cancellation)
            .expect("complete independent reduct subset enumeration")
            .accepted()
        {
            continue;
        }
        let atoms: BTreeSet<_> = candidate
            .atoms()
            .map(|atom| {
                input
                    .atoms()
                    .at(atom)
                    .unwrap()
                    .to_atom(zetesis_core::ValueLimits::default())
                    .unwrap()
            })
            .collect();
        let model = Model::from_positions(input.atom_catalog(), candidate.atoms()).unwrap();
        let evaluation = zetesis_objective::evaluate(
            input.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .expect("bounded objective evaluation of a verified model");
        let costs = evaluation.score().is_present().then(|| {
            evaluation
                .score()
                .costs()
                .iter()
                .map(|&(_, cost)| cost)
                .collect()
        });
        if models.is_empty() || costs < best {
            best.clone_from(&costs);
            models.clear();
        }
        if costs == best {
            assert!(models.insert(atoms), "distinct semantic answer sets");
        }
    }
    (models, best)
}

#[derive(Clone, Copy)]
enum Refusal {
    Unsafe,
    Feature(ProfileFeature),
    Evaluation,
}

fn expected_refusal(name: &str) -> Option<Refusal> {
    // These names encode a reviewed language boundary. No unexpected admission
    // error may turn into a skipped semantic comparison.
    match name {
        "count_global_head_unsafe"
        | "count_unsafe_local"
        | "set_body_duplicate_atom"
        | "set_body_signed_literal_keys" => Some(Refusal::Unsafe),
        "set_body_excluded_middle" => Some(Refusal::Feature(ProfileFeature::Aggregate)),
        // Finite aggregate tuple expressions are admitted, but reached 1/0
        // remains an evaluation failure. clingo's discarded-term result is
        // retained as reference data, not substituted for this source contract.
        // An assignment's family is its carrier: when every value the
        // aggregate can take divides by zero, in its own rule or in a rule
        // that reads its head, the family is entirely undefined and refused,
        // and clingo's answer, which drops the undefined instances, is
        // retained as reference data. A carrier value outside i32 is refused;
        // clingo's recorded answers wrap the sum to -2147483648 in both models.
        "sum_local_undefined"
        | "assignment_required_count_zero_divisor"
        | "assignment_downstream_zero_divisor"
        | "assignment_required_sum_overflow" => Some(Refusal::Evaluation),
        _ => None,
    }
}

fn assert_refusal(error: &FormulaFailure, expected: Refusal) {
    assert!(!error.diagnostics().is_empty(), "located source refusal");
    match expected {
        Refusal::Unsafe => assert!(matches!(error, FormulaFailure::UnsafeVariable { .. })),
        Refusal::Evaluation => assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
        )),
        Refusal::Feature(expected) => {
            let FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                feature,
                ..
            })) = error
            else {
                panic!("expected reviewed profile refusal, got {error:?}");
            };
            assert_eq!(*feature, expected);
        }
    }
}

#[test]
fn recorded_aggregate_comparisons_match_exhaustive_reduct_models() {
    let cases = cases();
    assert_eq!(cases.len(), 330);
    assert_eq!(cases.iter().filter(|case| case.valid).count(), 328);
    let mut admitted = 0;
    let mut refused = 0;
    for case in cases {
        let result = admit(&case.source, &FormulaLimits::default());
        if let Some(expected) = expected_refusal(&case.name) {
            let Err(error) = result else {
                panic!(
                    "{}: expected the explicitly reviewed source refusal",
                    case.name
                );
            };
            assert_refusal(&error, expected);
            refused += 1;
        } else {
            let input = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            let expected = case
                .models
                .iter()
                .map(|model| model.iter().map(|value| atom(value)).collect())
                .collect();
            assert_eq!(
                exhaustive(&input),
                (expected, case.costs),
                "{}: {}",
                case.name,
                case.source
            );
            admitted += 1;
        }
    }
    assert_eq!((admitted, refused), (321, 9));
}

#[test]
fn aggregate_translation_refusals_identify_original_rule_and_resource() {
    let rule = "a :- #sum{1:p;-1:q} >= 0.";
    let source = format!("p.\nq.\n{rule}");
    // The default budgets admit the reference; each case below lowers one.
    let input = formula(&source);
    for (expected, limit) in [
        (AggregateErrorKind::NodeLimit, 0),
        (AggregateErrorKind::WorkLimit, 0),
        (AggregateErrorKind::StateLimit, 0),
        (AggregateErrorKind::SubsetLimit, 1),
    ] {
        let error = admit(
            &source,
            &resource_limits::formula_aggregate(expected, limit),
        )
        .expect_err("bounded constructor refusal, never semantic UNSAT");
        let FormulaFailure::Aggregate { error, location } = error else {
            panic!("expected aggregate constructor refusal");
        };
        assert_eq!(error.kind(), expected);
        assert_eq!(
            input
                .source()
                .expect("source input")
                .slice(location.location().expect("parsed source").span)
                .expect("original span"),
            rule
        );
    }
    let error = admit(
        &source,
        &resource_limits::formula(FormulaResource::AggregateElements, 1),
    )
    .expect_err("preflight distinct tuple storage");
    let FormulaFailure::Limit {
        resource,
        limit,
        observed,
        location,
    } = error
    else {
        panic!("expected distinct tuple preflight refusal");
    };
    assert_eq!(
        (resource, limit, observed),
        (FormulaResource::AggregateElements, 1, 2)
    );
    assert_eq!(
        input
            .source()
            .expect("source input")
            .slice(location.location().expect("parsed source").span)
            .expect("original span"),
        rule
    );
}

#[test]
fn possible_producers_preserve_scored_answers() {
    for source in [
        "a :- #count{}=1. #minimize{1@7,k:a}.",
        "p. d(0;1). n(N) :- d(N),#count{1:p}=N. #minimize{1@9,k:n(0);1@7,k:n(1)}.",
    ] {
        objective_dependencies::check(source);
    }
}

fn compare_external(case: &Case) {
    // clingo refuses an invalid source (65) and reports it undecided; a valid
    // source is decided.
    let exits: &[i32] = if case.valid { &oracle::DECIDED } else { &[65] };
    let run = oracle::run_accepting(
        &case.source,
        &["0", "--outf=2", "--opt-mode=optN"],
        exits,
        oracle::Limits::default(),
    );
    let json = oracle::json(&run);
    if !case.valid {
        assert_eq!(json["Result"], "UNKNOWN", "{}", case.name);
        assert!(String::from_utf8_lossy(run.stderr()).contains("unsafe variables"));
        return;
    }
    assert_eq!(json["Models"]["More"], "no", "{}", case.name);
    assert_eq!(
        oracle::costs(&json["Models"]["Costs"]),
        case.costs,
        "{}",
        case.name
    );
    let witnesses: Vec<_> = json["Call"]
        .as_array()
        .expect("reference calls")
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(
        u64::try_from(witnesses.len()).expect("bounded witness count"),
        json["Models"]["Number"].as_u64().expect("raw model count")
    );
    let actual: Models = witnesses
        .iter()
        .filter(|witness| oracle::costs(&witness["Costs"]) == case.costs)
        .map(|witness| oracle::atoms(&witness["Value"]))
        .collect();
    assert_eq!(actual, case.models, "{}: {}", case.name, case.source);
    if case.costs.is_some() {
        assert_eq!(json["Result"], "OPTIMUM FOUND");
        assert_eq!(json["Models"]["Optimum"], "yes");
        assert_eq!(
            u64::try_from(actual.len()).expect("distinct optimum count"),
            json["Models"]["Optimal"].as_u64().expect("optimal count")
        );
    } else {
        assert_eq!(
            actual.len(),
            witnesses.len(),
            "nonoptimizing models are distinct semantic identities"
        );
    }
}

#[test]
#[ignore = "requires clingo: recorded recursive aggregate semantics match clingo"]
fn recorded_recursive_aggregate_semantics_match_clingo() {
    let cases = cases();
    assert_eq!(cases.len(), 330);
    assert_eq!(cases.iter().filter(|case| case.valid).count(), 328);
    for case in cases {
        compare_external(&case);
    }
}
