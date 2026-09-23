//! The work-stealing region scheduler returns the exact answer-set family on
//! every run, whatever the worker count or interleaving.
//!
//! N-queens forces genuine region splitting and stealing across many workers, so
//! a model lost or duplicated by a steal, or a termination race in the
//! `outstanding` counter, would surface as a family that differs from the scalar
//! walk's or between runs. Each worker count is exercised many times so a
//! schedule-dependent fault has repeated opportunities to appear. The order in
//! which models arrive is the schedule's and is deliberately not asserted; only
//! the family is.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::Theory;
use zetesis_sat::{Cancellation, Limits, StableModels};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

/// A standalone eight-queens encoding: 92 answer sets, enough independent
/// choices to make the workers split and steal rather than run one subtree.
const QUEENS: &str = include_str!("../../../examples/kr-domains/standalone/n-queens/variant-02.lp");
const EXPECTED_MODELS: usize = 92;
/// Runs per worker count. Large enough to expose a schedule-dependent fault,
/// small enough to keep the portable suite quick: this test builds unoptimized,
/// so each enumeration is far dearer than a release run.
const RUNS: usize = 16;

fn theory() -> Theory {
    admit_formula(
        QUEENS.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .theory()
    .clone()
}

/// Enumerate the complete family under `workers` region walkers. Asserts the
/// walk covered the space and never emitted a model twice; returns the family.
fn family(theory: &Theory, workers: usize) -> BTreeSet<Vec<usize>> {
    let mut search = StableModels::with_region_workers(
        theory,
        NonZeroUsize::new(workers).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut models = BTreeSet::new();
    for model in search.by_ref() {
        let model = model.unwrap();
        assert!(
            models.insert(model.atoms().collect()),
            "a worker emitted the same model twice"
        );
    }
    assert!(
        search.exhausted(),
        "the walk stopped before covering the candidate space"
    );
    models
}

#[test]
fn many_workers_return_the_scalar_family_on_every_run() {
    let theory = theory();
    // One worker is the scalar regions walk: the family every schedule must equal.
    let expected = family(&theory, 1);
    assert_eq!(expected.len(), EXPECTED_MODELS);
    for workers in [8usize, 14] {
        for run in 0..RUNS {
            assert_eq!(
                family(&theory, workers),
                expected,
                "worker count {workers}, run {run} returned a different family"
            );
        }
    }
}
