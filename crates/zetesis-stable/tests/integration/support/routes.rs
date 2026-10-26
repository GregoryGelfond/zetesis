//! The region search's routes, shared by the route controls.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use zetesis_ferraris::Theory;
use zetesis_stable::{BatchLimits, Cancellation, Engagement, Limits, StableModels};

use super::batching::residual;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Route {
    Scalar,
    Producers,
    /// Native region workers, started at the first pull.
    Native,
    /// The native walk that never engages its workers: it stays on the
    /// calling thread, its coordinator retaining one checker across pulls.
    Coordinator,
}

pub(crate) const ROUTES: [Route; 4] = [
    Route::Scalar,
    Route::Producers,
    Route::Native,
    Route::Coordinator,
];

impl Route {
    /// A search of `theory` on this route under the default limits, with four
    /// workers where the route has workers.
    pub(crate) fn search(self, theory: &Theory, cancellation: Cancellation) -> StableModels {
        self.search_with(
            theory,
            NonZeroUsize::new(4).unwrap(),
            Limits::default(),
            cancellation,
        )
    }

    /// A search of `theory` on this route under `limits`, with `workers` where
    /// the route has workers.
    pub(crate) fn search_with(
        self,
        theory: &Theory,
        workers: NonZeroUsize,
        limits: Limits,
        cancellation: Cancellation,
    ) -> StableModels {
        match self {
            Self::Scalar => StableModels::new(theory, limits, cancellation),
            Self::Producers => {
                StableModels::with_region_producers(theory, workers, limits, cancellation)
            }
            Self::Native => StableModels::with_region_workers_engaging(
                theory,
                workers,
                Engagement::Immediate,
                limits,
                cancellation,
            ),
            Self::Coordinator => StableModels::with_region_workers_engaging(
                theory,
                workers,
                Engagement::After(std::time::Duration::MAX),
                limits,
                cancellation,
            ),
        }
        .unwrap()
    }

    pub(crate) fn collect(self, search: &mut StableModels) -> BTreeSet<Vec<usize>> {
        let mut answers = BTreeSet::new();
        if matches!(self, Self::Producers) {
            while !search.exhausted() {
                for answer in search
                    .next_batch(
                        BatchLimits {
                            max_candidates: NonZeroUsize::new(64).unwrap(),
                            max_pending_bytes: 1024 * 1024,
                        },
                        residual,
                    )
                    .unwrap()
                {
                    assert!(answers.insert(answer.atoms().collect()));
                }
            }
        } else {
            for answer in search.by_ref() {
                assert!(answers.insert(answer.unwrap().atoms().collect()));
            }
        }
        assert!(search.exhausted());
        answers
    }
}

/// A search of `original` on `route` under `limits`, with three workers where
/// the route has workers.
pub(crate) fn search(
    route: Route,
    original: &Theory,
    limits: Limits,
    control: Cancellation,
) -> StableModels {
    route.search_with(original, NonZeroUsize::new(3).unwrap(), limits, control)
}
