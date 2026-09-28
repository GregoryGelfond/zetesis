//! Ordinary finite policy admits useful enumeration without weakening explicit stops.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_core::Predicate;
use zetesis_cpu::Cancellation;
use zetesis_sat::Incomplete;
use zetesis_solve::{
    AnswerSet, Backend, Completion, Interruption, Oracle, PreparedInput, SemanticOutcome, Session,
    SolveConfig,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const PATH_VERTICES: u32 = 24;

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..Default::default()
    }
}

fn path_family(vertices: u32) -> BTreeSet<u32> {
    // Bit i denotes selected(i+1). Adjacent selected bits violate precisely one
    // path edge. This reference never uses source lowering or solver formulas.
    (0..1_u32 << vertices)
        .filter(|mask| mask & (mask >> 1) == 0)
        .collect()
}

fn path_count(vertices: u32) -> usize {
    // Partition independent sets by absence/presence of the last vertex.
    let (mut previous, mut current) = (1, 1);
    for _ in 0..vertices {
        (previous, current) = (current, previous + current);
    }
    current
}

fn mask(answer: &AnswerSet, vertices: u32) -> u32 {
    let expected = Predicate::new("selected", 1).unwrap();
    let mut selected = 0;
    for atom in answer.interpretation().atoms() {
        assert_eq!(atom.predicate(), expected);
        assert_eq!(atom.values().len(), 1);
        let zetesis_core::ValueNodeRef::Number(vertex) = atom.values().at(0).unwrap().descriptor()
        else {
            panic!("selected must have one numeric vertex")
        };
        let vertex = u32::try_from(vertex).unwrap();
        assert!((1..=vertices).contains(&vertex));
        let bit = 1 << (vertex - 1);
        assert_eq!(selected & bit, 0, "each vertex occurs once");
        selected |= bit;
    }
    selected
}

struct Observed {
    remaining: BTreeSet<u32>,
    yielded: usize,
    outcome: SemanticOutcome,
}

fn stream(vertices: u32, configuration: SolveConfig) -> Observed {
    let owner = admit_formula(
        format!("#const n={vertices}. {{ selected(1..n) }}. :- selected(X), selected(X+1)."),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let mut remaining = path_family(vertices);
    assert_eq!(remaining.len(), path_count(vertices));
    let mut session = Session::builder(
        PreparedInput::formula(&owner),
        configuration,
        Cancellation::default(),
    )
    .start()
    .unwrap();
    let mut yielded = 0;
    for answer in session.by_ref() {
        let answer = answer.unwrap();
        let selected = mask(&answer, vertices);
        assert!(
            remaining.remove(&selected),
            "duplicate or unexpected answer"
        );
        yielded += 1;
    }
    assert!(session.next().is_none());
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.verified_models(), u64::try_from(yielded).unwrap());
    assert!(!outcome.unsatisfiable());
    Observed {
        remaining,
        yielded,
        outcome,
    }
}

#[test]
fn ordinary_defaults_complete_the_path_family() {
    for oracle in [Oracle::Auto, Oracle::Countermodel] {
        let observed = stream(PATH_VERTICES, SolveConfig { oracle, ..config() });
        assert!(observed.remaining.is_empty());
        assert_eq!(observed.yielded, path_count(PATH_VERTICES));
        assert_eq!(observed.outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(observed.outcome.interruption(), None);
        let statistics = observed.outcome.countermodel_statistics().unwrap();
        assert_eq!(
            statistics.candidates,
            u64::try_from(observed.yielded).unwrap()
        );
        assert!(statistics.search.work <= config().max_search_work);
    }
}

#[test]
fn a_work_ceiling_below_the_family_cost_interrupts() {
    // The exhausted enumeration's own work fixes a ceiling below it, so the
    // ceiling bites under whichever search method is the default.
    let exhausted = stream(
        PATH_VERTICES,
        SolveConfig {
            oracle: Oracle::Countermodel,
            ..config()
        },
    );
    assert!(exhausted.remaining.is_empty());
    let cost = exhausted
        .outcome
        .countermodel_statistics()
        .unwrap()
        .search
        .work;
    let ceiling = cost / 2;
    let observed = stream(
        PATH_VERTICES,
        SolveConfig {
            oracle: Oracle::Countermodel,
            max_search_work: ceiling,
            ..config()
        },
    );
    assert!(observed.yielded > 0);
    assert!(
        !observed.remaining.is_empty(),
        "completion={:?}, interruption={:?}, search={:?}",
        observed.outcome.completion(),
        observed.outcome.interruption(),
        observed.outcome.countermodel_statistics().unwrap().search
    );
    assert_eq!(observed.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        observed.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::WorkLimit))
    );
    let statistics = observed.outcome.countermodel_statistics().unwrap();
    assert!(statistics.search.work <= ceiling);
    assert!(statistics.candidates >= observed.outcome.verified_models());
}

#[test]
fn explicit_candidate_limits_remain_incomplete() {
    let maximum = 4;
    let observed = stream(
        8,
        SolveConfig {
            max_candidates: maximum,
            ..config()
        },
    );
    assert!(!observed.remaining.is_empty());
    assert_eq!(observed.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        observed.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::CandidateLimit))
    );
    assert_eq!(observed.outcome.candidate_progress(), maximum);
    assert_eq!(observed.outcome.verified_models(), maximum);
}

#[test]
fn explicit_decision_limits_remain_incomplete() {
    let maximum = 8;
    // One worker: a decision ceiling shared by several walkers may stop them
    // all before any reaches a leaf; the scalar walk reaches its first leaf
    // within the atom count.
    let observed = stream(
        8,
        SolveConfig {
            max_search_decisions: maximum,
            workers: NonZeroUsize::MIN,
            ..config()
        },
    );
    assert!(observed.yielded > 0);
    assert!(!observed.remaining.is_empty());
    assert_eq!(observed.outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        observed.outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::DecisionLimit))
    );
    assert_eq!(
        observed
            .outcome
            .countermodel_statistics()
            .unwrap()
            .search
            .decisions,
        maximum
    );
}
