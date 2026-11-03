//! Strict incumbent bounds prove the optima the inclusive schedule proves, and
//! a tie pass after them returns the same optimal family, each answer once.

use std::{convert::Infallible, num::NonZeroUsize};

use zetesis_algebra::Cancellation;
use zetesis_core::Model;
use zetesis_reference_support::{formula, hybrid};
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, OptimumSearch,
    PreparedInput, SemanticOutcome, Session, SolveConfig,
};
use zetesis_themelios::{AdmittedFormula, HybridFormula};

use crate::support::models::Family;

/// Three of the six pairs tie at the optimum, cost two; the others cost three.
const PAIRS: &str = "2 { p(1..4) } 2. w(1,1). w(2,1). w(3,1). w(4,2).
#minimize { W,X : p(X), w(X,W) }.";
/// Two of six answers tie at both priorities; `#show.` displays every answer
/// alike.
const PRIORITIES: &str = "{a;b;c}. :- a, b. #show.
#minimize { 1@2,x:a; 1@2,y:b; 1@1,z:c; 2@2,n:not a, not b }.";
/// A single optimum, which the strict bound meets before any tie could exist.
const SINGLE: &str = "1 {a;b} 1. #minimize { 5,a:a; 2,b:b }.";

const SCHEDULES: [OptimumSearch; 3] = [
    OptimumSearch::Inclusive,
    OptimumSearch::StrictWithTiePass,
    OptimumSearch::StrictForOneAnswer,
];

/// The program's two admissions: the eager formula and the lazy core with its
/// streamed templates.
struct Program {
    eager: AdmittedFormula,
    lazy: HybridFormula,
}

impl Program {
    fn new(source: &str) -> Self {
        Self {
            eager: formula(source),
            lazy: hybrid(source),
        }
    }

    /// The serial walk and four region workers, each over both admissions.
    fn routes(&self) -> impl Iterator<Item = (PreparedInput<'_>, SolveConfig)> {
        [Grounder::Eager, Grounder::Lazy]
            .into_iter()
            .flat_map(move |grounder| {
                [1, 4].map(move |workers| {
                    let input = match grounder {
                        Grounder::Lazy => PreparedInput::hybrid(&self.lazy),
                        _ => PreparedInput::formula(&self.eager),
                    };
                    (
                        input,
                        SolveConfig {
                            backend: Backend::Cpu,
                            grounder,
                            workers: NonZeroUsize::new(workers).unwrap(),
                            ..SolveConfig::default()
                        },
                    )
                })
            })
    }

    /// The serial walks, whose candidate order is fixed: they place a limit
    /// or a cancellation in a chosen pass.
    fn serial_routes(&self) -> impl Iterator<Item = (PreparedInput<'_>, SolveConfig)> {
        self.routes()
            .filter(|(_, config)| config.workers == NonZeroUsize::MIN)
    }
}

/// When the session's search should be cancelled.
#[derive(Clone, Copy)]
enum Moment {
    /// As the first strict bound is installed, inside the strict pass.
    StrictBound,
    /// As the tie pass begins, after the strict pass proved its optimum.
    TiePass,
}

/// The objective bounds a session installs, in order, and the tie passes it
/// begins; optionally cancels its search at one of them.
#[derive(Default)]
struct Schedule {
    strict: Vec<bool>,
    tie_passes: usize,
    cancel: Option<(Moment, Cancellation)>,
}

impl Schedule {
    fn cancelling(moment: Moment, cancellation: &Cancellation) -> Self {
        Self {
            cancel: Some((moment, cancellation.clone())),
            ..Self::default()
        }
    }

    fn arrive(&self, moment: Moment) {
        if let Some((at, cancellation)) = &self.cancel
            && std::mem::discriminant(at) == std::mem::discriminant(&moment)
        {
            cancellation.cancel();
        }
    }
}

impl ExecutionObserver for Schedule {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match event {
            ExecutionObservation::ObjectiveBound { strict, .. } => {
                self.strict.push(strict);
                if strict {
                    self.arrive(Moment::StrictBound);
                }
            }
            ExecutionObservation::ObjectiveTiePass { .. } => {
                self.tie_passes += 1;
                self.arrive(Moment::TiePass);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Every answer the session returns, in order, with its final outcome.
fn solve(
    input: PreparedInput<'_>,
    config: SolveConfig,
    schedule: &mut Schedule,
    cancellation: Cancellation,
) -> (Vec<Model>, SemanticOutcome) {
    let mut session = Session::new_observed(input, config, cancellation, schedule).unwrap();
    let mut answers = Vec::new();
    while let Some(answer) = session.next_observed(schedule) {
        answers.push(answer.unwrap().interpretation().clone());
    }
    assert!(session.next().is_none());
    (answers, session.outcome().unwrap())
}

fn run(
    input: PreparedInput<'_>,
    config: SolveConfig,
    optimum_search: OptimumSearch,
    models: usize,
) -> (Vec<Model>, SemanticOutcome, Schedule) {
    let mut schedule = Schedule::default();
    let (answers, outcome) = solve(
        input,
        SolveConfig {
            optimum_search,
            models,
            ..config
        },
        &mut schedule,
        Cancellation::default(),
    );
    (answers, outcome, schedule)
}

fn costs(outcome: &SemanticOutcome) -> Vec<(i32, i64)> {
    outcome.incumbent().unwrap().score.costs().to_vec()
}

#[test]
fn every_schedule_returns_the_inclusive_optimal_family() {
    for source in [PAIRS, PRIORITIES, SINGLE] {
        let program = Program::new(source);
        for (input, config) in program.routes() {
            let (expected, baseline, _) = run(input, config, OptimumSearch::Inclusive, 0);
            assert_eq!(baseline.completion(), Some(Completion::Exhausted));
            assert!(baseline.optimum_proved());
            for schedule in SCHEDULES {
                let (answers, outcome, _) = run(input, config, schedule, 0);
                assert_eq!(
                    answers.into_iter().collect::<Family>(),
                    expected.iter().cloned().collect::<Family>(),
                    "{source} {schedule:?} {config:?}"
                );
                assert_eq!(outcome.completion(), Some(Completion::Exhausted));
                assert!(outcome.optimum_proved());
                assert_eq!(costs(&outcome), costs(&baseline), "{source} {schedule:?}");
                let best = outcome.incumbent().unwrap();
                assert_eq!(best.tied_models, baseline.incumbent().unwrap().tied_models);
                assert!(!best.ties_pruned);
                assert_eq!(
                    outcome.complete_answer_count(),
                    baseline.complete_answer_count()
                );
            }
        }
    }
}

#[test]
fn the_tie_pass_returns_each_optimal_answer_once() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let (answers, outcome, schedule) = run(input, config, OptimumSearch::StrictWithTiePass, 0);
        assert_eq!(schedule.tie_passes, 1, "{config:?}");
        assert_eq!(answers.len(), 3, "{config:?}");
        assert_eq!(answers.into_iter().collect::<Family>().len(), 3);
        assert_eq!(outcome.retained_models(), 3);
        assert_eq!(outcome.incumbent().unwrap().tied_models, 3);
        assert_eq!(outcome.complete_answer_count(), Some(3));
    }
}

#[test]
fn the_tie_pass_follows_strict_bounds_with_one_inclusive_bound() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let (_, _, schedule) = run(input, config, OptimumSearch::StrictWithTiePass, 0);
        let (last, earlier) = schedule.strict.split_last().unwrap();
        assert!(!earlier.is_empty(), "{config:?}");
        assert!(earlier.iter().all(|&strict| strict), "{config:?}");
        assert!(!last, "{config:?}");
    }
}

#[test]
fn several_requested_answers_count_every_optimal_tie() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let (answers, outcome, schedule) = run(input, config, OptimumSearch::StrictWithTiePass, 2);
        assert_eq!(schedule.tie_passes, 1);
        assert_eq!(answers.len(), 2);
        assert_eq!(outcome.incumbent().unwrap().tied_models, 3, "{config:?}");
        assert!(!outcome.incumbent().unwrap().ties_pruned);
    }
}

#[test]
fn one_requested_answer_runs_no_tie_pass() {
    let program = Program::new(PAIRS);
    for schedule_kind in [
        OptimumSearch::StrictWithTiePass,
        OptimumSearch::StrictForOneAnswer,
    ] {
        for (input, config) in program.routes() {
            let (answers, outcome, schedule) = run(input, config, schedule_kind, 1);
            assert_eq!(schedule.tie_passes, 0, "{schedule_kind:?} {config:?}");
            assert!(schedule.strict.iter().all(|&strict| strict));
            assert_eq!(answers.len(), 1);
            assert_eq!(outcome.completion(), Some(Completion::Exhausted));
            assert!(outcome.optimum_proved());
            assert_eq!(costs(&outcome), [(0, 2)]);
        }
    }
}

#[test]
fn one_answer_under_strict_bounds_establishes_no_family_size() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let (_, outcome, _) = run(input, config, OptimumSearch::DEFAULT, 1);
        let best = outcome.incumbent().unwrap();
        assert!(best.ties_pruned, "{config:?}");
        assert!((1..=3).contains(&best.tied_models));
        assert_eq!(outcome.complete_answer_count(), None);
    }
}

#[test]
fn the_default_bounds_strictly_only_for_one_answer() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let (_, _, one) = run(input, config, OptimumSearch::DEFAULT, 1);
        assert!(!one.strict.is_empty());
        assert!(one.strict.iter().all(|&strict| strict), "{config:?}");
        for models in [0, 2] {
            let (_, outcome, several) = run(input, config, OptimumSearch::DEFAULT, models);
            assert!(!several.strict.is_empty());
            assert!(several.strict.iter().all(|&strict| !strict), "{config:?}");
            assert_eq!(several.tie_passes, 0);
            assert_eq!(outcome.incumbent().unwrap().tied_models, 3);
        }
    }
}

#[test]
fn the_inclusive_schedule_installs_no_strict_bound() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        for models in [0, 1] {
            let (_, outcome, schedule) = run(input, config, OptimumSearch::Inclusive, models);
            assert!(!schedule.strict.is_empty());
            assert!(schedule.strict.iter().all(|&strict| !strict), "{config:?}");
            assert_eq!(schedule.tie_passes, 0);
            assert_eq!(outcome.incumbent().unwrap().tied_models, 3);
            assert_eq!(outcome.complete_answer_count(), Some(3));
        }
    }
}

#[test]
fn cancellation_in_the_strict_pass_proves_no_optimum() {
    let program = Program::new(PAIRS);
    for (input, config) in program.serial_routes() {
        let cancellation = Cancellation::default();
        let mut schedule = Schedule::cancelling(Moment::StrictBound, &cancellation);
        let config = SolveConfig {
            optimum_search: OptimumSearch::StrictWithTiePass,
            models: 0,
            ..config
        };
        let (_, outcome) = solve(input, config, &mut schedule, cancellation);
        assert_eq!(schedule.tie_passes, 0, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(!outcome.optimum_proved());
        assert_eq!(outcome.complete_answer_count(), None);
    }
}

#[test]
fn cancellation_in_the_tie_pass_establishes_no_family() {
    let program = Program::new(PAIRS);
    for (input, config) in program.serial_routes() {
        let cancellation = Cancellation::default();
        let mut schedule = Schedule::cancelling(Moment::TiePass, &cancellation);
        let config = SolveConfig {
            optimum_search: OptimumSearch::StrictWithTiePass,
            models: 0,
            ..config
        };
        let (answers, outcome) = solve(input, config, &mut schedule, cancellation);
        assert_eq!(schedule.tie_passes, 1, "{config:?}");
        assert!(answers.len() < 3, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(outcome.complete_answer_count(), None);
    }
}

#[test]
fn cancellation_in_the_tie_pass_keeps_the_proved_optimum() {
    // The strict pass exhausted before the tie pass began, so the optimum is
    // proved whatever stops the pass; only the family is incomplete. The
    // cancellation arrives as the pass begins, on every route alike.
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        let cancellation = Cancellation::default();
        let mut schedule = Schedule::cancelling(Moment::TiePass, &cancellation);
        let config = SolveConfig {
            optimum_search: OptimumSearch::StrictWithTiePass,
            models: 0,
            ..config
        };
        let (_, outcome) = solve(input, config, &mut schedule, cancellation);
        assert_eq!(schedule.tie_passes, 1, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(outcome.optimum_proved(), "{config:?}");
        assert_eq!(costs(&outcome), [(0, 2)]);
    }
}

#[test]
fn a_candidate_limit_in_the_strict_pass_proves_no_optimum() {
    let program = Program::new(PAIRS);
    for (input, config) in program.serial_routes() {
        let (_, strict, _) = run(input, config, OptimumSearch::StrictForOneAnswer, 1);
        let spent = strict.candidate_progress();
        assert!(spent >= 2, "the strict pass needs more than one candidate");
        let (_, outcome, schedule) = run(
            input,
            SolveConfig {
                max_candidates: spent - 1,
                ..config
            },
            OptimumSearch::StrictWithTiePass,
            0,
        );
        assert_eq!(schedule.tie_passes, 0, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(!outcome.optimum_proved());
        assert_eq!(outcome.complete_answer_count(), None);
    }
}

#[test]
fn a_candidate_limit_in_the_tie_pass_establishes_no_family() {
    let program = Program::new(PAIRS);
    for (input, config) in program.serial_routes() {
        // The strict pass alone fits; with the tie pass's own candidates the
        // run exceeds the limit, which covers both passes together.
        let (_, strict, _) = run(input, config, OptimumSearch::StrictForOneAnswer, 1);
        let spent = strict.candidate_progress();
        let (answers, outcome, schedule) = run(
            input,
            SolveConfig {
                max_candidates: spent + 1,
                ..config
            },
            OptimumSearch::StrictWithTiePass,
            0,
        );
        assert_eq!(schedule.tie_passes, 1, "{config:?}");
        assert!(answers.len() < 3, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(outcome.candidate_progress() <= spent + 1);
        assert_eq!(outcome.complete_answer_count(), None);
    }
}

#[test]
fn a_candidate_limit_in_the_tie_pass_keeps_the_proved_optimum() {
    let program = Program::new(PAIRS);
    for (input, config) in program.serial_routes() {
        let (_, strict, _) = run(input, config, OptimumSearch::StrictForOneAnswer, 1);
        let spent = strict.candidate_progress();
        let (_, outcome, schedule) = run(
            input,
            SolveConfig {
                max_candidates: spent + 1,
                ..config
            },
            OptimumSearch::StrictWithTiePass,
            0,
        );
        assert_eq!(schedule.tie_passes, 1, "{config:?}");
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert!(outcome.optimum_proved(), "{config:?}");
        assert_eq!(costs(&outcome), [(0, 2)]);
    }
}

#[test]
fn counting_counts_every_optimal_tie_under_every_schedule() {
    let program = Program::new(PAIRS);
    for (input, config) in program.routes() {
        for optimum_search in SCHEDULES {
            let mut session = Session::builder(
                input,
                SolveConfig {
                    optimum_search,
                    ..config
                },
                Cancellation::default(),
            )
            .counting()
            .start()
            .unwrap();
            for answer in session.by_ref() {
                answer.unwrap();
            }
            let outcome = session.outcome().unwrap();
            assert_eq!(
                outcome.complete_answer_count(),
                Some(3),
                "{optimum_search:?} {config:?}"
            );
        }
    }
}
