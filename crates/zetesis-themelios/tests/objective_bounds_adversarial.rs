//! Objective restrictions constrain classical candidates; the original reduct
//! and independently verified complete stable-model/cost contracts stay fixed.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use serde_json::Value as Json;
use zetesis_core::{Atom, Model, Term, Value};
use zetesis_cpu::Control;
use zetesis_ferraris::{Interpretation, Limits, check, models};
use zetesis_objective::{ObjectiveProgram, ObjectiveTemplate, Score};
use zetesis_sat::{Limits as SearchLimits, StableModels};
use zetesis_themelios::objective_bound::{
    ObjectiveBoundErrorKind, ObjectiveBoundLimits, ObjectiveBoundResource, ObjectivePlan,
    ObjectivePlanLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

type Records = BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)>;

struct Case {
    name: String,
    source: String,
    records: Records,
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/objective-bounds.jsonl")
        .lines()
        .map(|line| {
            let value: Json = serde_json::from_str(line).expect("recorded clingo source");
            Case {
                name: value["name"].as_str().unwrap().to_owned(),
                source: value["source"].as_str().unwrap().to_owned(),
                records: value["models"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| (names(&row[0]), costs(&row[1])))
                    .collect(),
            }
        })
        .collect()
}

fn names(value: &Json) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect()
}

fn costs(value: &Json) -> Option<Vec<i64>> {
    value
        .as_array()
        .map(|values| values.iter().map(|value| value.as_i64().unwrap()).collect())
}

fn text(atom: &Atom) -> String {
    let predicate = atom.predicate().name();
    if atom.values().is_empty() {
        return predicate.to_owned();
    }
    let values: Vec<_> = atom
        .values()
        .iter()
        .map(|value| match value {
            Value::Infimum => "#inf".to_owned(),
            Value::Supremum => "#sup".to_owned(),
            Value::Structured(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::Symbol(value) => value.clone(),
            Value::String(value) => serde_json::to_string(value).unwrap(),
        })
        .collect();
    format!("{predicate}({})", values.join(","))
}

fn admit(source: &str) -> AdmittedFormula {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("source must be admitted: {source}: {error}"))
}

fn relation(input: &AdmittedFormula, mask: usize) -> Model {
    Model::new(
        input
            .atoms()
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, atom)| atom.clone()),
    )
}

fn interpretation(input: &AdmittedFormula, mask: usize) -> Interpretation {
    Interpretation::new(
        input.theory(),
        (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
    )
    .unwrap()
}

fn score(input: &AdmittedFormula, mask: usize) -> Score {
    zetesis_objective::evaluate(
        input.objectives(),
        &relation(input, mask),
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .expect("complete independent objective evaluation")
    .score()
    .clone()
}

fn stable(input: &AdmittedFormula) -> Vec<usize> {
    assert!(input.atoms().len() <= 10, "bounded exhaustive reference");
    (0..1 << input.atoms().len())
        .filter(|&mask| {
            check(
                input.theory(),
                &interpretation(input, mask),
                Limits::default(),
                &Control::default(),
            )
            .expect("independent complete Ferraris subset enumeration")
            .accepted()
        })
        .collect()
}

fn records(input: &AdmittedFormula, stable: &[usize]) -> Records {
    stable
        .iter()
        .map(|&mask| {
            let score = score(input, mask);
            (
                relation(input, mask).atoms().iter().map(text).collect(),
                score
                    .is_present()
                    .then(|| score.costs().iter().map(|&(_, value)| value).collect()),
            )
        })
        .collect()
}

fn plan(input: &AdmittedFormula) -> ObjectivePlan {
    ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        ObjectivePlanLimits::default(),
        &Control::default(),
    )
    .expect("complete objective key/eligibility plan in original atom order")
}

fn bounded_models(
    input: &AdmittedFormula,
    restriction: &zetesis_ferraris::Theory,
) -> BTreeSet<usize> {
    let mut search =
        StableModels::new(input.theory(), SearchLimits::default(), Control::default()).unwrap();
    search.restrict_candidates(restriction).unwrap();
    assert!(search.theory().same_instance(input.theory()));
    let mut result = BTreeSet::new();
    for candidate in search.by_ref() {
        let candidate = candidate.expect("completed restricted original-reduct search");
        assert!(candidate.theory().same_instance(input.theory()));
        let mask = candidate.atoms().fold(0, |mask, atom| mask | (1 << atom));
        assert!(result.insert(mask), "no duplicate semantic projections");
    }
    assert!(
        search.exhausted(),
        "restricted region was completely covered"
    );
    result
}

#[test]
fn every_verified_incumbent_retains_exactly_improving_and_tied_original_models() {
    let cases = cases();
    assert_eq!(cases.len(), 24);
    let mut total = 0;
    let mut nonlexical = false;
    for case in cases {
        let input = admit(&case.source);
        nonlexical |= input.atoms().windows(2).any(|pair| pair[0] > pair[1]);
        let original_nodes = input.theory().nodes().to_vec();
        let original_roots = input.theory().roots().to_vec();
        let stable = stable(&input);
        assert_eq!(records(&input, &stable), case.records, "{}", case.name);
        total += stable.len();
        let plan = plan(&input);
        assert!(plan.original().same_instance(input.theory()));
        let all_scores: Vec<_> = (0..1 << input.atoms().len())
            .map(|mask| score(&input, mask))
            .collect();
        for &incumbent in &stable {
            let incumbent_score = &all_scores[incumbent];
            let bound = plan
                .bound(
                    incumbent_score,
                    ObjectiveBoundLimits::default(),
                    &Control::default(),
                )
                .unwrap();
            assert!(bound.original().same_instance(input.theory()));
            for (mask, score) in all_scores.iter().enumerate() {
                let candidate = Interpretation::new(
                    bound.theory(),
                    (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
                )
                .unwrap();
                let actual = models(
                    bound.theory(),
                    &candidate,
                    Limits::default(),
                    &Control::default(),
                )
                .unwrap();
                assert_eq!(
                    actual,
                    score.compare_costs(incumbent_score) != Ordering::Greater,
                    "{}: candidate={mask}, incumbent={incumbent}",
                    case.name
                );
            }
            let expected = stable
                .iter()
                .copied()
                .filter(|&mask| {
                    all_scores[mask].compare_costs(incumbent_score) != Ordering::Greater
                })
                .collect();
            assert_eq!(
                bounded_models(&input, bound.theory()),
                expected,
                "{}",
                case.name
            );
        }
        assert_eq!(input.theory().nodes(), original_nodes);
        assert_eq!(input.theory().roots(), original_roots);
    }
    assert_eq!(total, 97);
    assert!(
        nonlexical,
        "catalog order must differ from storage order in this campaign"
    );
}

#[test]
fn repeated_strict_incumbent_improvements_keep_all_optimal_ties_without_duplicates() {
    for case in cases() {
        let input = admit(&case.source);
        let exhaustive = stable(&input);
        let optimum = exhaustive
            .iter()
            .map(|&mask| score(&input, mask))
            .min_by(Score::compare_costs)
            .unwrap();
        let expected: BTreeSet<_> = exhaustive
            .into_iter()
            .filter(|&mask| score(&input, mask).compare_costs(&optimum) == Ordering::Equal)
            .collect();
        let plan = plan(&input);
        let mut search =
            StableModels::new(input.theory(), SearchLimits::default(), Control::default()).unwrap();
        let mut seen = BTreeSet::new();
        let mut best: Option<Score> = None;
        while let Some(result) = search.next() {
            let model = result.unwrap();
            let mask = model.atoms().fold(0, |mask, atom| mask | (1 << atom));
            assert!(
                seen.insert(mask),
                "{}: semantic blocks must survive restart",
                case.name
            );
            let candidate_score = score(&input, mask);
            if best
                .as_ref()
                .is_none_or(|best| candidate_score.compare_costs(best) == Ordering::Less)
            {
                let bound = plan
                    .bound(
                        &candidate_score,
                        ObjectiveBoundLimits::default(),
                        &Control::default(),
                    )
                    .unwrap();
                search.restrict_candidates(bound.theory()).unwrap();
                best = Some(candidate_score);
            }
        }
        assert!(search.exhausted());
        assert_eq!(
            best.unwrap().compare_costs(&optimum),
            Ordering::Equal,
            "{}",
            case.name
        );
        let actual: BTreeSet<_> = seen
            .into_iter()
            .filter(|&mask| score(&input, mask).compare_costs(&optimum) == Ordering::Equal)
            .collect();
        assert_eq!(
            actual, expected,
            "{}: every tied optimum remains reachable",
            case.name
        );
    }
}

fn constant_score(values: &[(i32, i32)]) -> Score {
    let program = ObjectiveProgram::new(
        values
            .iter()
            .map(|&(priority, weight)| {
                ObjectiveTemplate::new(
                    Term::Constant(Value::Number(weight)),
                    priority,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            })
            .collect(),
        zetesis_objective::AdmissionLimits::default(),
    )
    .unwrap();
    zetesis_objective::evaluate(
        &program,
        &Model::new([]),
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .unwrap()
    .score()
    .clone()
}

#[test]
fn foreign_numeric_score_slots_are_compared_with_missing_priorities_as_zero() {
    // This tests the arithmetic bound contract; foreign scores are deliberately
    // not certified incumbents and are never used here to claim an optimum.
    let input = admit("{a;b}. #minimize{1@2:a;-3@-1:b}.");
    let plan = plan(&input);
    for values in [
        vec![],
        vec![(3, 1)],
        vec![(3, -1)],
        vec![(2, 1), (0, -1)],
        vec![(-2, 1)],
    ] {
        let ceiling = constant_score(&values);
        let bound = plan
            .bound(
                &ceiling,
                ObjectiveBoundLimits::default(),
                &Control::default(),
            )
            .unwrap();
        for mask in 0..1 << input.atoms().len() {
            let candidate = Interpretation::new(
                bound.theory(),
                (0..input.atoms().len()).filter(|index| mask & (1 << index) != 0),
            )
            .unwrap();
            assert_eq!(
                models(
                    bound.theory(),
                    &candidate,
                    Limits::default(),
                    &Control::default()
                )
                .unwrap(),
                score(&input, mask).compare_costs(&ceiling) != Ordering::Greater
            );
        }
    }
}

#[test]
fn exact_global_key_limits_count_coalesced_keys_and_all_complete_bindings() {
    let input = admit("{a;b}. #minimize{2@1,k:a;2@1,k:b}.");
    let exact = ObjectivePlanLimits {
        max_keys: 1,
        max_bindings: 2,
        max_key_bytes: 26,
        ..ObjectivePlanLimits::default()
    };
    let plan = ObjectivePlan::new(
        input.theory(),
        input.atoms(),
        input.objectives(),
        exact,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(
        (
            plan.statistics().keys,
            plan.statistics().bindings,
            plan.statistics().key_bytes
        ),
        (1, 2, 26)
    );
    for (limits, resource) in [
        (
            ObjectivePlanLimits {
                max_keys: 0,
                ..exact
            },
            ObjectiveBoundResource::Keys,
        ),
        (
            ObjectivePlanLimits {
                max_bindings: 1,
                ..exact
            },
            ObjectiveBoundResource::Bindings,
        ),
        (
            ObjectivePlanLimits {
                max_key_bytes: 25,
                ..exact
            },
            ObjectiveBoundResource::KeyBytes,
        ),
        (
            ObjectivePlanLimits {
                max_work: 0,
                ..exact
            },
            ObjectiveBoundResource::Work,
        ),
        (
            ObjectivePlanLimits {
                max_nodes: 0,
                ..exact
            },
            ObjectiveBoundResource::Nodes,
        ),
    ] {
        let error = ObjectivePlan::new(
            input.theory(),
            input.atoms(),
            input.objectives(),
            limits,
            &Control::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), ObjectiveBoundErrorKind::Limit(resource));
    }
}

#[test]
fn optional_bound_refusals_leave_the_original_available_for_complete_search() {
    let input = admit("{a;b}. #minimize{2@1,k:a;2@1,k:b}.");
    let baseline = stable(&input);
    let plan = plan(&input);
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(matches!(
        plan.bound(
            &score(&input, baseline[0]),
            ObjectiveBoundLimits::default(),
            &cancelled
        )
        .unwrap_err()
        .kind(),
        ObjectiveBoundErrorKind::Control(_)
    ));
    let error = plan
        .bound(
            &score(&input, baseline[0]),
            ObjectiveBoundLimits {
                max_work: 0,
                ..ObjectiveBoundLimits::default()
            },
            &Control::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        ObjectiveBoundErrorKind::Limit(ObjectiveBoundResource::Work)
    );
    let valid = plan
        .bound(
            &score(&input, baseline[0]),
            ObjectiveBoundLimits::default(),
            &Control::default(),
        )
        .unwrap();
    assert!(valid.original().same_instance(input.theory()));
    assert_eq!(stable(&input), baseline, "refusal is not semantic pruning");
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("zetesis-bound-oracle-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("oracle directory: {error}"),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("oracle fixture cleanup");
    }
}

fn clingo(source: &str) -> Records {
    let directory = Directory::new();
    let input = directory.0.join("source.lp");
    let output = directory.0.join("models.json");
    let errors = directory.0.join("stderr.txt");
    fs::write(&input, source).unwrap();
    let mut child = Command::new("clingo")
        .args(["--outf=2", "--models=0", "--opt-mode=enum"])
        .arg(&input)
        .stdout(Stdio::from(File::create(&output).unwrap()))
        .stderr(Stdio::from(File::create(&errors).unwrap()))
        .spawn()
        .expect("installed optional clingo oracle");
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() >= Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("external oracle timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(fs::metadata(&output).unwrap().len() <= 65_536);
    assert!(fs::metadata(&errors).unwrap().len() <= 65_536);
    let value: Json = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(value["Models"]["More"], "no");
    let mut records = Records::new();
    let mut count = 0_u64;
    for witness in value["Call"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
    {
        assert!(records.insert((names(&witness["Value"]), costs(&witness["Costs"]))));
        count += 1;
    }
    assert_eq!(value["Models"]["Number"].as_u64(), Some(count));
    records
}

#[test]
#[ignore = "requires external clingo; records every full model and cost, not only optima"]
fn recorded_objective_bound_sources_match_fresh_complete_clingo_records() {
    for case in cases() {
        assert_eq!(clingo(&case.source), case.records, "{}", case.name);
    }
}
