//! Complete scalar/range source regressions, independently recorded with clingo
//! 5.8.2. Of 118 sources, 95 currently have exact native model parity, 7 valid
//! sources have explicit refused boundaries, and 16 sources are unsafe. The
//! historical integer-maximum singleton-range timeout is deliberately excluded.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use themelios_program::term::EvalError;
use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, FormulaResource, ProfileFeature, admit_formula,
};

type Models = BTreeSet<BTreeSet<String>>;
struct Case {
    name: String,
    source: String,
    valid: bool,
    native: String,
    expected: Models,
}
fn cases() -> Vec<Case> {
    include_str!("../fixtures/scalar-bindings.jsonl")
        .lines()
        .map(|line| {
            let row: Json = serde_json::from_str(line).expect("recorded scalar case");
            Case {
                name: row["name"].as_str().expect("case name").to_owned(),
                source: row["source"].as_str().expect("unchanged source").to_owned(),
                valid: row["valid"].as_bool().expect("source validity"),
                native: row["native"]
                    .as_str()
                    .expect("reviewed native boundary")
                    .to_owned(),
                expected: row["models"]
                    .as_array()
                    .expect("recorded models")
                    .iter()
                    .map(atoms)
                    .collect(),
            }
        })
        .collect()
}
fn atoms(values: &Json) -> BTreeSet<String> {
    values
        .as_array()
        .expect("complete canonical atom identities")
        .iter()
        .map(|value| value.as_str().expect("canonical ground atom").to_owned())
        .collect()
}
// This fixed corpus has scalar numbers, simple symbols and ordinary quoted
// strings. Preserve their complete identities without reraising clingo's i32
// minimum spelling, which the pinned source tier itself rejects as a literal.
fn atom_text<'a>(atom: impl Into<zetesis_core::catalog::AtomRef<'a>>) -> String {
    let atom = atom.into();
    let name = atom.predicate().name();
    if atom.values().is_empty() {
        return name.to_owned();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value.descriptor() {
            zetesis_core::ValueNodeRef::Number(number) => number.to_string(),
            zetesis_core::ValueNodeRef::Symbol(symbol) => symbol.to_owned(),
            zetesis_core::ValueNodeRef::String(value) => {
                serde_json::to_string(value).expect("quoted scalar string")
            }
            zetesis_core::ValueNodeRef::Infimum => "#inf".to_owned(),
            zetesis_core::ValueNodeRef::Supremum => "#sup".to_owned(),
            zetesis_core::ValueNodeRef::Function { .. }
            | zetesis_core::ValueNodeRef::Tuple { .. } => value.to_string(),
        })
        .collect();
    format!("{name}({})", values.join(","))
}
fn exhaustive(input: &AdmittedFormula) -> Models {
    let count = input.atoms().len();
    assert!(
        count <= 16,
        "bounded independent exhaustive candidate carrier"
    );
    let cancellation = Cancellation::default();
    let mut models = Models::new();
    for bits in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            input.theory(),
            (0..count).filter(|&atom| bits & (1 << atom) != 0),
        )
        .expect("same-theory candidate");
        if check(input.theory(), &candidate, Limits::default(), &cancellation)
            .expect("complete independent reduct check")
            .accepted()
        {
            assert!(
                models.insert(
                    candidate
                        .atoms()
                        .map(|atom| atom_text(input.atoms().at(atom).unwrap()))
                        .collect()
                )
            );
        }
    }
    models
}
fn refusal(error: &FormulaFailure, expected: &str) {
    assert!(!error.diagnostics().is_empty(), "located refusal");
    match expected {
        "UnsafeVariable" => assert!(matches!(error, FormulaFailure::UnsafeVariable { .. })),
        "Undefined" => assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Undefined,
                ..
            })
        )),
        "Overflow" => assert!(matches!(
            error,
            FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                error: EvalError::Overflow,
                ..
            })
        )),
        "SupportRounds" => assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::SupportRounds,
                limit: 4,
                observed: 5,
                ..
            }
        )),
        "Term" | "NegatedComparison" => {
            let expected = match expected {
                "Term" => ProfileFeature::Term,
                "NegatedComparison" => ProfileFeature::NegatedComparison,
                _ => unreachable!("matched feature"),
            };
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile { feature, .. })) if *feature == expected)
            );
        }
        other => panic!("unreviewed refusal: {other}"),
    }
}
#[test]
fn scalar_and_interval_admissions_match_complete_models_with_explicit_boundaries() {
    let cases = cases();
    assert_eq!(cases.len(), 118);
    assert_eq!(cases.iter().filter(|case| case.valid).count(), 102);
    let mut admitted = 0;
    let mut refused = 0;
    for case in cases {
        // A growing positive upper relation must refuse, despite clingo pruning
        // this particular source by a mandatory negative gate. Four rounds make
        // the resource regression small and independent of default work tuning.
        let limits = if case.native == "SupportRounds" {
            FormulaLimits {
                max_support_rounds: 4,
                ..FormulaLimits::default()
            }
        } else {
            FormulaLimits::default()
        };
        let result = admit_formula(
            case.source.clone(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits,
        );
        // Keep historical native labels and original reference models intact.
        // Evaluated choice heads, separate bounds and mixed arithmetic families
        // promote these cases without altering the independent reference record.
        let mixed_arithmetic = matches!(
            case.name.as_str(),
            "undefined_assignment_drops_instance" | "undefined_head_drops_instance"
        );
        if case.native == "admit"
            || mixed_arithmetic
            || matches!(
                case.name.as_str(),
                "head_conditional_choice_arithmetic"
                    | "head_arithmetic_choice_global"
                    | "assign_integer_bounds"
                    | "head_double_negative_self"
                    | "range_inf_upper_empty"
                    | "range_sup_lower_empty"
                    | "interval_body_positive"
                    | "interval_body_negative"
            )
        {
            assert!(case.valid);
            let input = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            if mixed_arithmetic {
                assert_eq!(input.warnings().len(), 1);
            }
            assert_eq!(
                exhaustive(&input),
                case.expected,
                "{}: {}",
                case.name,
                case.source
            );
            admitted += 1;
        } else {
            let Err(error) = result else {
                panic!("{}: expected {}", case.name, case.native);
            };
            refusal(
                &error,
                if case.name == "range_negative_literal_unsafe" {
                    // The finite range syntax is admitted; single negation still
                    // cannot bind the source variable used by the head.
                    "UnsafeVariable"
                } else {
                    &case.native
                },
            );
            refused += 1;
        }
    }
    assert_eq!((admitted, refused), (95, 23));
}

fn clingo(case: &Case) -> Models {
    // clingo refuses an invalid source (65) and reports it undecided; a valid
    // source is decided.
    let exits: &[i32] = if case.valid { &oracle::DECIDED } else { &[65] };
    let run = oracle::run_accepting(
        &case.source,
        &["0", "--outf=2", "--warn=none"],
        exits,
        oracle::Limits::default(),
    );
    let json = oracle::json(&run);
    if !case.valid {
        assert_eq!(json["Result"].as_str(), Some("UNKNOWN"));
        assert!(String::from_utf8_lossy(run.stderr()).contains("unsafe"));
        return Models::new();
    }
    assert_eq!(json["Models"]["More"].as_str(), Some("no"));
    assert!(matches!(
        json["Result"].as_str(),
        Some("SATISFIABLE" | "UNSATISFIABLE")
    ));
    let mut models = Models::new();
    let mut count = 0;
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                count += 1;
                assert!(
                    models.insert(atoms(&witness["Value"])),
                    "duplicate full model"
                );
            }
        }
    }
    assert_eq!(json["Models"]["Number"].as_u64(), Some(count));
    models
}
#[test]
#[ignore = "requires independent clingo; 118 bounded reference source programs"]
fn scalar_and_interval_reference_models_and_unsafe_cases_match_fresh_clingo() {
    for case in cases() {
        assert_eq!(
            clingo(&case),
            case.expected,
            "{}: {}",
            case.name,
            case.source
        );
    }
}

#[test]
#[ignore = "requires independent clingo; bounded interval facts"]
fn nonnumeric_fact_ranges_match_clingo() {
    for source in [
        "p(a..b).",
        "p(1..a).",
        "p(a..1).",
        "p(\"a\"..\"b\").",
        "p(#inf..#sup).",
        "p(f(a)..f(b)).",
    ] {
        let case = Case {
            name: source.into(),
            source: source.into(),
            valid: true,
            native: "admit".into(),
            expected: Models::from([BTreeSet::new()]),
        };
        let input = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .expect("defined nonnumeric endpoints");
        assert_eq!(exhaustive(&input), case.expected, "{source}");
        assert_eq!(clingo(&case), case.expected, "{source}");
    }
}
