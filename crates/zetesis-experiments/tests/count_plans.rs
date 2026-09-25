//! Bounded source-derived candidate restrictions through the public library API.
//!
//! The manual experiment includes fresh preparation, grounding and exact native
//! enumeration as separate intervals. Enumeration includes restriction encoding
//! and bounded model capture; comparison, sorting and JSON output stay outside.
//! No device, process RSS, clingo timing or lazy source execution is measured.

use std::{fmt::Write as _, time::Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zetesis_cpu::Cancellation;
use zetesis_sat::{StableModels, Statistics};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CountPlanLimits, CountPlanStatus, ExpansionLimits,
    FormulaLimits, prepare_formula,
};

const MAX_MODELS: usize = 1_024;
const MAX_MODEL_ATOMS: usize = 262_144;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ordinary,
    SourceCountPlan,
}

#[derive(Debug, Default, Serialize)]
struct Planning {
    status: &'static str,
    consequences: usize,
    work: u64,
    storage_bytes: u64,
    restriction_nodes: usize,
    restriction_roots: usize,
}

impl Planning {
    fn read(admitted: &AdmittedFormula) -> Self {
        let (status, statistics, restriction) = match admitted.count_plan() {
            CountPlanStatus::NotRequested => {
                return Self {
                    status: "not_requested",
                    ..Self::default()
                };
            }
            CountPlanStatus::NoPlan(statistics) => ("no_plan", statistics, None),
            CountPlanStatus::Ready(plan) => {
                assert!(admitted.theory().same_instance(plan.original_theory()));
                ("ready", plan.statistics(), Some(plan.restriction()))
            }
            CountPlanStatus::Incomplete(error) => panic!("optional planning incomplete: {error}"),
        };
        Self {
            status,
            consequences: statistics.consequences,
            work: statistics.work,
            storage_bytes: statistics.storage_bytes,
            restriction_nodes: restriction.map_or(0, |theory| theory.nodes().len()),
            restriction_roots: restriction.map_or(0, |theory| theory.roots().len()),
        }
    }
}

#[derive(Debug, Serialize)]
struct Measurement {
    mode: Mode,
    preparation_ns: u64,
    grounding_ns: u64,
    enumeration_ns: u64,
    planning: Planning,
    search_work: u64,
    search_decisions: u64,
    candidate_queries: u64,
    candidates: u64,
    countermodel_queries: u64,
    candidate_restrictions: u64,
    full_models: usize,
}

struct Sample {
    admitted: AdmittedFormula,
    models: Vec<Vec<usize>>,
    measurement: Measurement,
}

fn elapsed(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).expect("nanosecond interval fits u64")
}

fn measure(source: &str, mode: Mode) -> Sample {
    // Copy source bytes before the preparation timer, identically for both modes.
    let source = source.to_owned();
    let started = Instant::now();
    let prepared = prepare_formula(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let preparation_ns = elapsed(started);
    let cancellation = Cancellation::default();
    let started = Instant::now();
    let admitted = match mode {
        Mode::Ordinary => prepared.ground(),
        Mode::SourceCountPlan => {
            prepared.ground_with_count_plan(CountPlanLimits::default(), &cancellation, None)
        }
    }
    .unwrap();
    let grounding_ns = elapsed(started);
    let planning = Planning::read(&admitted);
    assert!(admitted.objective_declarations().is_empty());
    assert!(!admitted.objectives().is_present());
    let started = Instant::now();
    let (mut models, statistics) = enumerate(&admitted);
    let enumeration_ns = elapsed(started);
    models.sort_unstable();
    // Preserve a multiset for comparison; check duplicates without erasing them.
    assert!(models.windows(2).all(|adjacent| adjacent[0] != adjacent[1]));
    let measurement = Measurement {
        mode,
        preparation_ns,
        grounding_ns,
        enumeration_ns,
        planning,
        search_work: statistics.search.work,
        search_decisions: statistics.search.decisions,
        candidate_queries: statistics.candidate_queries,
        candidates: statistics.candidates,
        countermodel_queries: statistics.countermodel_queries,
        candidate_restrictions: statistics.candidate_restrictions,
        full_models: models.len(),
    };
    Sample {
        admitted,
        models,
        measurement,
    }
}

fn enumerate(admitted: &AdmittedFormula) -> (Vec<Vec<usize>>, Statistics) {
    let mut search = StableModels::new(
        admitted.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    if let CountPlanStatus::Ready(plan) = admitted.count_plan() {
        search.restrict_candidates(plan.restriction()).unwrap();
    }
    let mut models = Vec::new();
    models.try_reserve_exact(MAX_MODELS).unwrap();
    let mut stored_atoms = 0_usize;
    for model in search.by_ref() {
        let model = model.unwrap();
        assert!(model.theory().same_instance(admitted.theory()));
        assert!(models.len() < MAX_MODELS, "model capture incomplete");
        let count = model.atoms().count();
        stored_atoms = stored_atoms
            .checked_add(count)
            .filter(|&total| total <= MAX_MODEL_ATOMS)
            .expect("atom capture incomplete");
        let mut atoms = Vec::new();
        atoms.try_reserve_exact(count).unwrap();
        atoms.extend(model.atoms());
        models.push(atoms);
    }
    assert!(search.exhausted());
    assert_eq!(
        usize::try_from(search.statistics().stable_models).unwrap(),
        models.len()
    );
    (models, search.statistics())
}

fn compare(reference: &Sample, actual: &Sample) {
    assert_eq!(
        reference.admitted.source().text(),
        actual.admitted.source().text()
    );
    assert_eq!(reference.admitted.atoms(), actual.admitted.atoms());
    assert_eq!(
        reference.admitted.theory().nodes(),
        actual.admitted.theory().nodes()
    );
    assert_eq!(
        reference.admitted.theory().roots(),
        actual.admitted.theory().roots()
    );
    assert_eq!(
        reference.admitted.formula_origins(),
        actual.admitted.formula_origins()
    );
    assert_eq!(reference.models, actual.models);
}

fn grouped_choices(groups: usize, width: usize) -> String {
    let rows: Vec<_> = (1..=groups)
        .map(|group| {
            (1..=width)
                .map(|value| format!("p({group},{value})"))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect();
    let whole = rows.join(";");
    let mut capacities = String::new();
    for row in &rows {
        write!(capacities, "{{{row}}}1.").unwrap();
    }
    format!("{groups}{{{whole}}}{groups}.{capacities}")
}

#[test]
fn domain_facts_do_not_supply_canonical_eligibility() {
    // The initial source profile requires literal canonical true eligibility.
    // Proving these domain atoms true would need another source consequence.
    let source = "g(1..2).v(1..2).2{p(G,V):g(G),v(V)}2.{p(G,V):v(V)}1:-g(G).";
    let ordinary = measure(source, Mode::Ordinary);
    let planned = measure(source, Mode::SourceCountPlan);
    compare(&ordinary, &planned);
    assert_eq!(planned.measurement.planning.status, "no_plan");
    assert_eq!(planned.models.len(), 4);
}

#[test]
fn source_count_plans_preserve_cartesian_selections() {
    for (groups, width) in [(2, 2), (3, 2), (3, 3)] {
        let source = grouped_choices(groups, width);
        let ordinary = measure(&source, Mode::Ordinary);
        let planned = measure(&source, Mode::SourceCountPlan);
        compare(&ordinary, &planned);
        assert_eq!(planned.measurement.planning.status, "ready");
        assert_eq!(planned.measurement.planning.consequences, groups);
        assert_eq!(planned.measurement.candidate_restrictions, 1);
        assert_eq!(
            planned.models.len(),
            width.pow(u32::try_from(groups).unwrap())
        );
        for model in &planned.models {
            for group in 1..=groups {
                let count = model
                    .iter()
                    .filter(|&&atom| {
                        let atom = &planned.admitted.atoms()[atom];
                        atom.predicate().name() == "p"
                            && atom.values()[0]
                                == zetesis_core::Value::Number(i32::try_from(group).unwrap())
                    })
                    .count();
                assert_eq!(count, 1);
            }
        }
    }
}

const QUEENS: [&str; 6] = [
    include_str!("../../../examples/correctness/standalone/n-queens/variant-01.lp"),
    include_str!("../../../examples/correctness/standalone/n-queens/variant-02.lp"),
    include_str!("../../../examples/correctness/standalone/n-queens/variant-03.lp"),
    include_str!("../../../examples/correctness/standalone/n-queens/variant-04.lp"),
    include_str!("../../../examples/correctness/standalone/n-queens/variant-05.lp"),
    include_str!("../../../examples/correctness/standalone/n-queens/variant-06.lp"),
];

#[test]
#[ignore = "bounded work/timing experiment; run serially in a quiet release measurement window"]
fn report_source_count_plan_measurements() {
    let mut fixtures: Vec<_> = [(2, 4), (3, 3), (4, 3)]
        .into_iter()
        .map(|(groups, width)| {
            (
                format!("groups-{groups}-width-{width}"),
                grouped_choices(groups, width),
                width.pow(u32::try_from(groups).unwrap()),
            )
        })
        .collect();
    fixtures.extend(
        QUEENS
            .into_iter()
            .enumerate()
            .map(|(index, source)| (format!("queens-{:02}", index + 1), source.to_owned(), 92)),
    );
    for (name, source, expected_models) in fixtures {
        let source_sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
        let reference = measure(&source, Mode::Ordinary);
        assert_eq!(reference.models.len(), expected_models);
        // The same reference remains live in every interval. Rotate two complete
        // fresh conditions; retain every finished record without outlier removal.
        for round in 0..4 {
            let modes = if round % 2 == 0 {
                [Mode::Ordinary, Mode::SourceCountPlan]
            } else {
                [Mode::SourceCountPlan, Mode::Ordinary]
            };
            for mode in modes {
                let sample = measure(&source, mode);
                compare(&reference, &sample);
                println!("COUNT_PLAN {}", serde_json::to_string(&serde_json::json!({
                    "schema_version": 1, "fixture": name, "source_sha256": source_sha256,
                    "source_bytes": source.len(), "round": round, "sample": sample.measurement,
                    "original_subject_equal": true, "full_models_equal": true,
                    "grounder": "eager_formula", "backend": "scalar_reduct",
                    "native_limits": "CountPlanLimits/FormulaLimits/ExpansionLimits/AdmissionOptions/zetesis_sat::Limits defaults at qualified source revision",
                    "capture_max_models": MAX_MODELS, "capture_max_model_atoms": MAX_MODEL_ATOMS,
                    "enumeration_includes": "candidate encoding, full reduct checking and bounded model capture",
                    "objective_scope": "objective-free inputs only"
                })).unwrap());
            }
        }
    }
}
