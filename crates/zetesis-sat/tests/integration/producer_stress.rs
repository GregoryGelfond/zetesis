//! Repeated batched production preserves the scalar answer-set family across
//! producer counts and candidate slate sizes.
//!
//! The GPU route's region producers fill bounded slates from per-producer
//! deques with work stealing. This test guards the *family*: a candidate lost
//! or duplicated by a steal, a hand dropped at a slate's end, or a slate that
//! ends early shows as a family that differs from the scalar walk's, and a
//! count that never reaches zero hangs until the enumeration's deadline. It does
//! NOT establish that stealing occurred or qualify scaling: a serialized
//! schedule can return the same family; the deterministic producer tests cover
//! the transitions one at a time. Each combination runs repeatedly to sample
//! interleavings; this is not exhaustive schedule coverage. Membership is left
//! to the exact host completion, so every produced candidate is decided as the
//! scalar walk decides it.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::support::batching::residual;
use crate::support::choice_theories;
use crate::support::formula_theories as theories;
use zetesis_ferraris::Theory;
use zetesis_reference_support::formula;
use zetesis_sat::{BatchLimits, Cancellation, Limits, StableModels};
use zetesis_test_support::counts::nonzero;

/// A standalone eight-queens encoding: 92 answer sets over a branching
/// region tree.
const QUEENS: &str =
    include_str!("../../../../examples/correctness/standalone/n-queens/variant-02.lp");
/// Runs per combination: sixty-four enumerations per theory across the
/// sixteen combinations, which samples interleavings while keeping the
/// unoptimized portable suite within tens of seconds.
const RUNS: usize = 4;
/// Far above any correct run of these small instances, so it never fires in
/// normal operation but turns a hang into a test failure.
const HANG_GUARD: Duration = Duration::from_mins(1);

/// The slate sizes of one enumeration: a fixed size, or a growing sequence
/// that doubles from one.
#[derive(Clone, Copy, Debug)]
enum Slates {
    Fixed(usize),
    Growing,
}

impl Slates {
    fn size(self, slate: u32) -> usize {
        match self {
            Self::Fixed(size) => size,
            Self::Growing => 1usize << slate.min(10),
        }
    }
}

/// Enumerate the complete family through `producers` producers and the given
/// slate sizes. Asserts no candidate was proposed twice and the walk covered
/// the space; returns the family.
fn family(theory: &Theory, producers: usize, slates: Slates) -> BTreeSet<Vec<usize>> {
    let mut search = StableModels::with_region_producers(
        theory,
        nonzero(producers),
        Limits::default(),
        Cancellation::with_deadline(Instant::now() + HANG_GUARD).unwrap(),
    )
    .unwrap();
    let mut models = BTreeSet::new();
    let mut slate = 0;
    while !search.exhausted() {
        let size = slates.size(slate);
        slate += 1;
        let limits = BatchLimits {
            max_candidates: nonzero(size),
            max_pending_bytes: 64 * 1024 * 1024,
        };
        for model in search.next_batch(limits, residual).unwrap() {
            assert!(
                models.insert(model.atoms().collect()),
                "a model was returned twice"
            );
        }
    }
    models
}

fn scalar(theory: &Theory) -> BTreeSet<Vec<usize>> {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    let models = search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect())
        .collect();
    assert!(search.exhausted());
    models
}

#[test]
fn producers_return_the_scalar_family_on_every_run() {
    let queens = formula(QUEENS).theory().clone();
    let inputs = [queens, choice_theories::choices(5), theories::mixed()];
    for theory in &inputs {
        let expected = scalar(theory);
        for producers in [2usize, 4, 8, 14] {
            for slates in [
                Slates::Fixed(1),
                Slates::Fixed(3),
                Slates::Fixed(64),
                Slates::Growing,
            ] {
                for run in 0..RUNS {
                    assert_eq!(
                        family(theory, producers, slates),
                        expected,
                        "{producers} producers, {slates:?}, run {run} returned a different family"
                    );
                }
            }
        }
    }
}
