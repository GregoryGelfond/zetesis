//! Independent complete display multisets; observation never changes the reduct.

use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use themelios_base::source::SourceId;
use zetesis_core::Model;
use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{Interpretation, check};
use zetesis_themelios::observation::{ErrorKind, Feature, Limits, Resource};
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits,
    FormulaFailure, FormulaLimits, ProfileFeature, admit_formula,
};

type Record = (Vec<String>, Option<Vec<i64>>);

fn cases() -> Vec<Json> {
    include_str!("fixtures/observations.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn admit(source: &str) -> Result<AdmittedFormula, FormulaFailure> {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
}
fn displayed(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0;
    for (index, character) in text.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else {
            match character {
                '"' => quoted = true,
                '(' => depth += 1,
                ')' => depth -= 1,
                _ if character.is_whitespace() && depth == 0 => {
                    if start < index {
                        values.push(text[start..index].to_owned());
                    }
                    start = index + character.len_utf8();
                }
                _ => {}
            }
        }
    }
    assert!(!quoted && depth == 0);
    if start < text.len() {
        values.push(text[start..].to_owned());
    }
    values.sort();
    values
}
fn reference(case: &Json) -> Vec<Record> {
    let witnesses = case["witnesses"].as_array().unwrap();
    let mut records: Vec<Record> = witnesses
        .iter()
        .map(|witness| {
            let mut values: Vec<String> = witness["Value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect();
            values.sort();
            let costs = witness["Costs"]
                .as_array()
                .map(|costs| costs.iter().map(|cost| cost.as_i64().unwrap()).collect());
            (values, costs)
        })
        .collect();
    if case["reference_result"] == "OPTIMUM FOUND" {
        let summary = &case["reference_summary"];
        let best: Vec<_> = summary["Costs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cost| cost.as_i64().unwrap())
            .collect();
        let first = records
            .iter()
            .position(|(_, cost)| cost.as_ref() == Some(&best))
            .unwrap();
        assert!(
            records[first + 1..].contains(&records[first]),
            "first final incumbent must recur in optN enumeration"
        );
        records = records.into_iter().skip(first + 1).collect();
        assert!(records.iter().all(|(_, cost)| cost.as_ref() == Some(&best)));
        assert_eq!(summary["Optimal"].as_u64().unwrap(), records.len() as u64);
    }
    records.sort();
    records
}
fn complete(input: &AdmittedFormula) -> Vec<Record> {
    assert!(input.atoms().len() <= 10);
    let mut records = Vec::new();
    for mask in 0..1_usize << input.atoms().len() {
        let candidate = Interpretation::new(
            input.theory(),
            (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        if !check(
            input.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Control::default(),
        )
        .unwrap()
        .accepted()
        {
            continue;
        }
        let model = Model::new(candidate.atoms().map(|index| input.atoms()[index].clone()));
        let rendered = input
            .metadata()
            .observations()
            .render(
                &model,
                input.metadata().output(),
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
        let objective = zetesis_objective::evaluate(
            input.objectives(),
            &model,
            zetesis_objective::Limits::default(),
            &Control::default(),
        )
        .unwrap();
        let score = objective.score();
        records.push((
            displayed(rendered.text()),
            score
                .is_present()
                .then(|| score.costs().iter().map(|&(_, cost)| cost).collect()),
        ));
    }
    if input.objectives().is_present() {
        let best = records.iter().map(|(_, cost)| cost.clone()).min().unwrap();
        records.retain(|(_, cost)| *cost == best);
    }
    records.sort();
    records
}
#[test]
fn admitted_sources_match_complete_recorded_display_and_cost_multisets() {
    let cases = cases();
    assert_eq!(cases.len(), 63);
    let mut admitted = 0;
    let mut records = 0;
    for case in cases
        .into_iter()
        .filter(|case| case["expected_refusal"].is_null())
    {
        let source = case["source"].as_str().unwrap();
        let input = admit(source).unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let expected = reference(&case);
        assert_eq!(complete(&input), expected, "{}: {source}", case["name"]);
        records += expected.len();
        admitted += 1;
    }
    assert_eq!((admitted, records), (52, 69));
}
#[test]
fn every_outside_profile_source_has_an_explicit_typed_refusal() {
    let mut refused = 0;
    for case in cases()
        .into_iter()
        .filter(|case| !case["expected_refusal"].is_null())
    {
        if case["expected_refusal"] == "Undefined" {
            let input = admit(case["source"].as_str().unwrap()).unwrap();
            let model = Model::new(input.atoms().iter().cloned());
            let error = input
                .metadata()
                .observations()
                .evaluate(&model, Limits::default(), &Control::default())
                .unwrap_err();
            assert_eq!(
                error.kind(),
                &ErrorKind::Evaluation(zetesis_themelios::observation::EvaluationError::Undefined)
            );
            assert!(error.location().is_some());
            refused += 1;
            continue;
        }
        let Err(error) = admit(case["source"].as_str().unwrap()) else {
            panic!("{} unexpectedly admitted", case["name"]);
        };
        match case["expected_refusal"].as_str().unwrap() {
            "Syntax" => assert!(matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Syntax(_)))
            )),
            "StrongNegation" => assert!(matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Profile {
                    feature: ProfileFeature::StrongNegation,
                    ..
                }))
            )),
            expected => {
                let feature = match expected {
                    "Term" => Feature::Term,
                    "Body" => Feature::Body,
                    "UnsafeVariable" => Feature::UnsafeVariable,
                    "AnonymousOutput" => Feature::AnonymousOutput,
                    other => panic!("unknown classification {other}"),
                };
                let FormulaFailure::Observation { error } = error else {
                    panic!("{}: expected observation refusal: {error}", case["name"]);
                };
                assert_eq!(
                    error.kind(),
                    &ErrorKind::Unsupported(feature),
                    "{}",
                    case["name"]
                );
                assert!(error.location().is_some());
            }
        }
        refused += 1;
    }
    assert_eq!(refused, 11);
}
#[test]
fn metadata_preserves_the_original_formula_and_source_identity() {
    let plain = admit("p(1). {q}. a:-missing.").unwrap();
    let observed = admit_formula(
        "p(1). {q}. a:-missing. #show f(X):p(X). #show f(X):p(X). #show missing.".into(),
        AdmissionOptions {
            source_id: SourceId::new(71),
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(plain.atoms(), observed.atoms());
    assert_eq!(plain.theory().nodes(), observed.theory().nodes());
    assert_eq!(plain.theory().roots(), observed.theory().roots());
    assert_eq!(plain.analyzed_program(), observed.analyzed_program());
    assert!(!observed.metadata().output().is_explicit());
    assert_eq!(
        observed
            .metadata()
            .observations()
            .origins()
            .map(<[themelios_base::span::Location]>::len)
            .sum::<usize>(),
        3
    );
    for origins in observed.metadata().observations().origins() {
        for &location in origins {
            assert_eq!(location.source, SourceId::new(71));
            assert!(
                observed
                    .source()
                    .slice(location.span)
                    .unwrap()
                    .starts_with("#show")
            );
        }
    }
}
#[test]
fn source_and_runtime_limits_are_independent_inclusive_and_never_partial() {
    let source = "p(1;2). #show. #show pair(X,X):p(X).";
    let input = admit(source).unwrap();
    let model = Model::new(input.atoms().iter().cloned());
    let program = input.metadata().observations();
    let evaluation = program
        .evaluate(&model, Limits::default(), &Control::default())
        .unwrap();
    assert_eq!(evaluation.symbols().len(), 2);
    let exact = Limits {
        max_work: evaluation.statistics().work,
        ..Default::default()
    };
    assert!(program.evaluate(&model, exact, &Control::default()).is_ok());
    let error = program
        .evaluate(
            &model,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &Control::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::Work,
            ..
        }
    ));
    for (limits, resource) in [
        (
            Limits {
                max_bindings: 0,
                ..Default::default()
            },
            Resource::Bindings,
        ),
        (
            Limits {
                max_terms: 1,
                ..Default::default()
            },
            Resource::Terms,
        ),
        (
            Limits {
                max_symbol_nodes: 2,
                ..Default::default()
            },
            Resource::Nodes,
        ),
        (
            Limits {
                max_symbol_depth: 1,
                ..Default::default()
            },
            Resource::Depth,
        ),
        (
            Limits {
                max_symbol_bytes: 3,
                ..Default::default()
            },
            Resource::Bytes,
        ),
        (
            Limits {
                max_output_bytes: 0,
                ..Default::default()
            },
            Resource::OutputBytes,
        ),
    ] {
        assert!(
            matches!(program.evaluate(&model, limits, &Control::default()).unwrap_err().kind(), ErrorKind::Limit { resource: actual, .. } if *actual == resource)
        );
    }
    let cancelled = Control::default();
    cancelled.cancel();
    assert_eq!(
        program
            .render(
                &model,
                input.metadata().output(),
                Limits::default(),
                &cancelled
            )
            .unwrap_err()
            .kind(),
        &ErrorKind::Stopped(Stop::Cancelled)
    );
    let deadline = Control::with_deadline(Instant::now());
    assert_eq!(
        program
            .evaluate(&model, Limits::default(), &deadline)
            .unwrap_err()
            .kind(),
        &ErrorKind::Stopped(Stop::Deadline)
    );
}

#[test]
fn public_model_spelling_is_validated_before_output() {
    use zetesis_core::{Atom, Predicate, Value};
    let program = zetesis_themelios::observation::ObservationProgram::default();
    let selection = zetesis_themelios::OutputSelection::default();
    for model in [
        Model::new([Atom::new(Predicate::new("bad\nAnswer: 9", 0).unwrap(), vec![]).unwrap()]),
        Model::new([Atom::new(
            Predicate::new("p", 1).unwrap(),
            vec![Value::Symbol("bad name".into())],
        )
        .unwrap()]),
        Model::new([Atom::new(
            Predicate::new("p", 1).unwrap(),
            vec![Value::String("bad\0value".into())],
        )
        .unwrap()]),
    ] {
        assert_eq!(
            program
                .render(&model, &selection, Limits::default(), &Control::default())
                .unwrap_err()
                .kind(),
            &ErrorKind::InvalidSymbol
        );
    }
}

#[test]
fn all_source_ceilings_preflight_owned_templates_and_origin_copies() {
    use zetesis_themelios::observation::AdmissionLimits;
    for (limits, resource) in [
        (
            AdmissionLimits {
                max_directives: 0,
                ..Default::default()
            },
            Resource::Directives,
        ),
        (
            AdmissionLimits {
                max_nodes: 0,
                ..Default::default()
            },
            Resource::Nodes,
        ),
        (
            AdmissionLimits {
                max_depth: 0,
                ..Default::default()
            },
            Resource::Depth,
        ),
        (
            AdmissionLimits {
                max_bytes: 0,
                ..Default::default()
            },
            Resource::Bytes,
        ),
        (
            AdmissionLimits {
                max_variables: 0,
                ..Default::default()
            },
            Resource::Variables,
        ),
        (
            AdmissionLimits {
                max_body_elements: 0,
                ..Default::default()
            },
            Resource::BodyElements,
        ),
        (
            AdmissionLimits {
                max_arity: 0,
                ..Default::default()
            },
            Resource::Arity,
        ),
        (
            AdmissionLimits {
                max_origins: 0,
                ..Default::default()
            },
            Resource::Origins,
        ),
    ] {
        let result = admit_formula(
            "p(1). #show f(X):p(X).".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits {
                observation: limits,
                ..Default::default()
            },
        );
        assert!(
            matches!(result, Err(FormulaFailure::Observation { error }) if matches!(error.kind(), ErrorKind::Limit { resource: actual, .. } if *actual == resource)),
            "{resource:?}"
        );
    }
    let deep = format!("#show {}x{}.", "f(".repeat(65), ")".repeat(65));
    let result = admit_formula(
        deep,
        AdmissionOptions {
            max_syntax_depth: 1_000,
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Observation { error }) if matches!(error.kind(), ErrorKind::Limit { resource: Resource::Depth, .. }))
    );
}

#[test]
fn original_bundle_constants_and_duplicate_locations_remain_separate_from_terms() {
    let directory = Directory::new();
    fs::write(
        directory.0.join("root.lp"),
        "#include \"other.lp\". #const k=2. p(k). #show. #show f(X):p(X).",
    )
    .unwrap();
    fs::write(directory.0.join("other.lp"), "#show f(X):p(X).").unwrap();
    let bundle = zetesis_themelios::SourceBundle::load(
        directory.0.join("root.lp"),
        zetesis_themelios::BundleLimits::default(),
    )
    .unwrap();
    let input = zetesis_themelios::admit_bundle_formula(
        bundle,
        zetesis_themelios::BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let model = Model::new(input.atoms().iter().cloned());
    assert_eq!(
        input
            .metadata()
            .observations()
            .render(
                &model,
                input.metadata().output(),
                Limits::default(),
                &Control::default()
            )
            .unwrap()
            .text(),
        "f(2)"
    );
    let locations: Vec<_> = input
        .metadata()
        .observations()
        .origins()
        .flatten()
        .copied()
        .collect();
    assert_eq!(locations.len(), 2);
    assert_ne!(locations[0].source, locations[1].source);
    for location in locations {
        assert!(
            input
                .bundle()
                .get(location.source)
                .unwrap()
                .source()
                .slice(location.span)
                .unwrap()
                .starts_with("#show")
        );
    }
}

#[test]
fn show_traversal_preserves_closed_logical_values_and_raw_body_limits() {
    for source in ["#show f(1). p(f(1)).", "p(f(1)). #show f(1)."] {
        let program = admit(source).unwrap();
        let model = Model::new(program.atoms().iter().cloned());
        let rendered = program
            .metadata()
            .observations()
            .render(
                &model,
                program.metadata().output(),
                Limits::default(),
                &Control::default(),
            )
            .unwrap();
        assert_eq!(rendered.text(), "p(f(1)) f(1)");
    }
    let result = admit_formula(
        "p. #show x:p,p.".into(),
        AdmissionOptions {
            max_body_elements: 1,
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Expansion(ExpansionFailure::Admission(
            AdmissionFailure::Limit {
                resource: zetesis_themelios::InputLimit::BodyElements,
                observed: 2,
                ..
            }
        )))
    ));
}

#[test]
fn shared_prefix_signature_lookups_charge_each_compared_name() {
    let prefix = "long_shared_prefix_".repeat(8);
    let first = format!("{prefix}a");
    let last = format!("{prefix}z");
    let source = format!("{first}. {last}. #show {first}/0. #show {last}/0. #show x.");
    let expanded = format!("{source} #show {prefix}b/0. #show {prefix}c/0.");
    let base = admit(&source).unwrap();
    let input = admit(&expanded).unwrap();
    let model = Model::new(input.atoms().iter().cloned());
    let render = |input: &AdmittedFormula, limits| {
        input.metadata().observations().render(
            &model,
            input.metadata().output(),
            limits,
            &Control::default(),
        )
    };
    let before = render(&base, Limits::default()).unwrap();
    let after = render(&input, Limits::default()).unwrap();
    assert_eq!(before.text(), after.text());
    assert!(after.statistics().work > before.statistics().work + 2 * prefix.len() as u64);
    let exact = Limits {
        max_work: after.statistics().work,
        ..Default::default()
    };
    assert!(render(&input, exact).is_ok());
    let error = render(
        &input,
        Limits {
            max_work: exact.max_work - 1,
            ..exact
        },
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        ErrorKind::Limit {
            resource: Resource::Work,
            ..
        }
    ));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "zetesis-observation-oracle-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("{error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn oracle(source: &str) -> Json {
    let directory = Directory::new();
    let input = directory.0.join("original.lp");
    let output = directory.0.join("output.json");
    let errors = directory.0.join("stderr");
    fs::write(&input, source).unwrap();
    let mut child = Command::new("clingo")
        .args(["--models=0", "--outf=2", "--opt-mode=optN"])
        .arg(input)
        .stdout(Stdio::from(File::create(&output).unwrap()))
        .stderr(Stdio::from(File::create(&errors).unwrap()))
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        let stopped = start.elapsed() >= Duration::from_secs(3)
            || fs::metadata(&output).unwrap().len() > 65_536
            || fs::metadata(&errors).unwrap().len() > 65_536;
        if stopped {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded clingo reference refused");
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(matches!(status.code(), Some(0 | 10 | 20 | 30 | 65)));
    assert!(
        fs::metadata(&output).unwrap().len() <= 65_536
            && fs::metadata(&errors).unwrap().len() <= 65_536
    );
    serde_json::from_slice(&fs::read(output).unwrap()).unwrap()
}
#[test]
#[ignore = "requires external clingo; complete bounded displayed-symbol multisets"]
fn unchanged_sources_match_fresh_clingo_reference() {
    for case in cases() {
        let raw = oracle(case["source"].as_str().unwrap());
        assert_eq!(raw["Result"], case["reference_result"], "{}", case["name"]);
        if raw["Result"] == "UNKNOWN" {
            continue;
        }
        assert_eq!(raw["Models"]["More"], "no");
        let witnesses: Vec<_> = raw["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .cloned()
            .collect();
        assert_eq!(
            raw["Models"]["Number"].as_u64().unwrap(),
            witnesses.len() as u64
        );
        let actual = serde_json::json!({"witnesses":witnesses, "reference_result":raw["Result"], "reference_summary":raw["Models"]});
        assert_eq!(reference(&actual), reference(&case), "{}", case["name"]);
    }
}
