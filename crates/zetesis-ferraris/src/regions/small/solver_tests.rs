//! The learned search against exhaustive enumeration on small random
//! clause-and-threshold problems, under each refinement of the search.

use zetesis_algebra::Cancellation;

use super::{Arena, NO_FAMILY, Outcome, Pb, Refinements, SmallPolicy, Solver, TRUE, idx, lit, var};

/// A deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }
}

/// Clauses over `vars` variables, and thresholds `out <-> sum >= bound`
/// sharing their weighted conditions in families.
struct Problem {
    vars: usize,
    clauses: Vec<Vec<u32>>,
    thresholds: Vec<(u32, Vec<u32>, Vec<u64>, i128)>,
}

fn problem(rng: &mut Rng) -> Problem {
    let base = 9 + rng.below(3);
    let mut thresholds = Vec::new();
    let mut vars = base;
    for _ in 0..1 {
        let conditions: Vec<u32> = (0..3 + rng.below(3))
            .map(|_| idx(rng.below(base)))
            .collect();
        let weights: Vec<u64> = conditions.iter().map(|_| 1 + rng.next() % 3).collect();
        let total: u64 = weights.iter().sum();
        for _ in 0..4 + rng.below(2) {
            let bound = i128::from(rng.next() % (total + 2));
            thresholds.push((idx(vars), conditions.clone(), weights.clone(), bound));
            vars += 1;
        }
    }
    // Near the satisfiability threshold of three-literal clauses, so that
    // searches under assumptions conflict deep below them.
    let clauses = (0..base * 3 + rng.below(base))
        .map(|_| {
            (0..3)
                .map(|_| lit(idx(rng.below(vars)), rng.next() & 1 == 1))
                .collect()
        })
        .collect();
    Problem {
        vars,
        clauses,
        thresholds,
    }
}

fn holds(assignment: u64, l: u32) -> bool {
    (assignment >> var(l) & 1 == 1) != (l & 1 == 1)
}

fn satisfies(problem: &Problem, assignment: u64, assumed: &[u32]) -> bool {
    assumed.iter().all(|&l| holds(assignment, l))
        && problem
            .clauses
            .iter()
            .all(|c| c.iter().any(|&l| holds(assignment, l)))
        && problem
            .thresholds
            .iter()
            .all(|(out, conditions, weights, bound)| {
                let sum: i128 = conditions
                    .iter()
                    .zip(weights)
                    .filter(|&(&c, _)| assignment >> c & 1 == 1)
                    .map(|(_, &w)| i128::from(w))
                    .sum();
                (assignment >> out & 1 == 1) == (sum >= *bound)
            })
}

fn satisfiable(problem: &Problem, assumed: &[u32]) -> bool {
    (0..1_u64 << problem.vars).any(|a| satisfies(problem, a, assumed))
}

fn solver(problem: &Problem, refinements: Refinements, local_keep: usize) -> Solver {
    let policy = SmallPolicy {
        refinements,
        local_keep,
        ..SmallPolicy::default()
    };
    let mut solver = Solver::new(problem.vars, policy);
    let mut clauses = Arena::default();
    for clause in &problem.clauses {
        clauses.push(clause);
    }
    let pbs = problem
        .thresholds
        .iter()
        .map(|(out, conditions, weights, bound)| Pb {
            out: *out,
            conditions: conditions.clone(),
            weights: weights.clone(),
            bound: *bound,
            sure: 0,
            open: 0,
            heaviest: 0,
            family: NO_FAMILY,
        })
        .collect();
    solver.install(&clauses, pbs);
    solver
}

fn check(refinements: Refinements, local_keep: usize) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let cancellation = Cancellation::default();
    for _ in 0..120 {
        let problem = problem(&mut rng);
        let mut solver = solver(&problem, refinements, local_keep);
        // Several searches on one solver: what one leaves behind must not
        // change the next one's verdict.
        for round in 0..6 {
            let assumed: Vec<u32> = (0..=rng.below(4))
                .map(|_| lit(idx(rng.below(problem.vars)), rng.next() & 1 == 1))
                .collect();
            let expected = satisfiable(&problem, &assumed);
            let outcome = solver
                .solve(&assumed, u64::MAX, &cancellation, None)
                .expect("not cancelled");
            match outcome {
                Outcome::Satisfiable => {
                    assert!(expected, "a completion of an unsatisfiable problem");
                    let model = (0..problem.vars)
                        .filter(|&v| solver.value(idx(v)) == TRUE)
                        .fold(0_u64, |a, v| a | 1 << v);
                    assert!(satisfies(&problem, model, &assumed), "not a completion");
                }
                Outcome::Unsatisfiable(_) => {
                    assert!(!expected, "a refutation of a satisfiable problem");
                    let core = solver.core.clone();
                    assert!(
                        core.iter()
                            .all(|l| assumed.contains(l) || solver.inconsistent),
                        "a core reaching past the assumptions"
                    );
                    assert!(
                        solver.inconsistent || !satisfiable(&problem, &core),
                        "a core that refutes nothing: round {round}, assumed {assumed:?}, core {core:?}"
                    );
                }
                Outcome::Unknown => panic!("an unbounded search gave up"),
            }
        }
    }
}

#[test]
fn the_measured_search_agrees_with_enumeration() {
    check(Refinements::MEASURED, usize::MAX);
}

#[test]
fn restriction_local_learning_agrees_with_enumeration() {
    let local = Refinements::MEASURED.with(Refinements::LOCAL, true);
    check(local, usize::MAX);
    check(local, 0);
    check(local, 2);
}

#[test]
fn threshold_families_agree_with_enumeration() {
    check(
        Refinements::MEASURED.with(Refinements::FAMILY, true),
        usize::MAX,
    );
}

#[test]
fn every_refinement_together_agrees_with_enumeration() {
    check(Refinements::ALL, 0);
    check(Refinements::ALL, usize::MAX);
}

#[test]
fn root_probes_stay_within_a_quarter_of_the_walk() {
    let mut scratch = super::SmallScratch::default();
    assert!(scratch.probe_affordable(), "nothing spent yet");
    let started = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .expect("a second ago");
    scratch.first_offer = Some(started);
    scratch.statistics.root_nanos = 200_000_000;
    assert!(scratch.probe_affordable(), "a fifth of the walk");
    scratch.statistics.root_nanos = 300_000_000;
    assert!(
        !scratch.probe_affordable(),
        "more than a quarter of the walk"
    );
}

#[test]
fn steering_leaves_an_optimizing_walk_alone_unless_asked() {
    let steered = SmallPolicy {
        refinements: Refinements::MEASURED.with(Refinements::STEER, true),
        ..SmallPolicy::measured()
    };
    let mut scratch = super::SmallScratch::default();
    scratch.set_policy(steered);
    assert!(scratch.steering(), "no objective: steering on");
    scratch.note_objective(true);
    assert!(!scratch.steering(), "an objective: steering off");
    scratch.set_policy(SmallPolicy {
        refinements: steered.refinements.with(Refinements::STEER_OBJECTIVE, true),
        ..steered
    });
    assert!(scratch.steering(), "steer_objective=1: steering on");
}
