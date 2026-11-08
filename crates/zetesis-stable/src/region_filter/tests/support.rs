//! Routes, filter fixtures, batch helpers and small theories for the region
//! check's composition tests. The filters implement the crate's own seam, which
//! no caller can implement.

use crate::region_filter::tests::injection::{Source as InjectedSource, Worker as InjectedWorker};
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zetesis_algebra::region::Region;
use zetesis_ferraris::{Interpretation, Node, Theory};
use zetesis_theory_support::theories::theory;

use crate::{
    BatchLimits, BatchVerdict, Cancellation, Engagement, Incomplete, Limits, RegionFeasibility,
    StableModels,
};

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

#[derive(Clone, Copy, Debug)]
pub(crate) enum Condition {
    Pass,
    ForbidHeld(usize),
    Require(usize),
    ForbidPair(usize, usize),
    FailPreparation,
    FailCheck,
    PanicOnDrop { fail_check: bool },
    Cancel,
}

#[derive(Debug)]
pub(crate) struct Filter {
    subject: Theory,
    condition: Condition,
    pub live: AtomicUsize,
}

impl Filter {
    pub(crate) fn new(subject: &Theory, condition: Condition) -> Arc<Self> {
        Arc::new(Self {
            subject: subject.clone(),
            condition,
            live: AtomicUsize::new(0),
        })
    }
}

struct Worker<'a>(&'a Filter);

impl Drop for Worker<'_> {
    fn drop(&mut self) {
        self.0.live.fetch_sub(1, Ordering::SeqCst);
        if matches!(self.0.condition, Condition::PanicOnDrop { .. }) {
            panic!("injected checker teardown failure");
        }
    }
}

impl InjectedSource for Filter {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn InjectedWorker + '_>, Incomplete> {
        cancellation.poll()?;
        if !theory.same_instance(&self.subject) {
            return Err(Incomplete::WrongTheory);
        }
        if matches!(self.condition, Condition::FailPreparation) {
            return Err(Incomplete::RegionFilter);
        }
        self.live.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Worker(self)))
    }
}

impl InjectedWorker for Worker<'_> {
    fn check(
        &mut self,
        theory: &Theory,
        region: &Region,
        cancellation: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        assert!(theory.same_instance(&self.0.subject));
        cancellation.poll()?;
        let refuted = match self.0.condition {
            Condition::Pass | Condition::PanicOnDrop { fail_check: false } => false,
            Condition::ForbidHeld(atom) => region.is_held(atom),
            Condition::Require(atom) => region.is_cut(atom),
            Condition::ForbidPair(left, right) => region.is_held(left) && region.is_held(right),
            Condition::FailCheck | Condition::PanicOnDrop { fail_check: true } => {
                return Err(Incomplete::RegionFilter);
            }
            Condition::Cancel => {
                cancellation.cancel();
                cancellation.poll()?;
                unreachable!("the check just cancelled its token")
            }
            Condition::FailPreparation => unreachable!("preparation refused this checker"),
        };
        Ok(if refuted {
            RegionFeasibility::Refuted
        } else {
            RegionFeasibility::NotRefuted
        })
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

/// A batch oracle that decides nothing: every candidate stays residual.
pub(crate) fn residual(
    _: &Theory,
    candidates: &[Interpretation],
) -> Result<Vec<BatchVerdict>, std::collections::TryReserveError> {
    let mut verdicts = Vec::new();
    verdicts.try_reserve_exact(candidates.len())?;
    verdicts.resize(candidates.len(), BatchVerdict::Residual);
    Ok(verdicts)
}

/// Batches of at most `count` candidates and 1 MiB of pending bytes.
pub(crate) fn batch(count: usize) -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(count).unwrap(),
        max_pending_bytes: 1024 * 1024,
    }
}

/// Independent choices: a | not a, for each atom.
pub(crate) fn choices(atoms: usize) -> Theory {
    let mut nodes = vec![Node::falsum()];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let a = nodes.len();
        nodes.push(Node::atom(atom));
        nodes.push(Node::implies(a, 0));
        nodes.push(Node::or_pair([a, a + 1]));
        roots.push(a + 2);
    }
    theory(atoms, nodes, roots)
}

/// Every stable model of `theory`: each interpretation the reduct check
/// accepts, as its atoms.
pub(crate) fn stable_models(theory: &Theory) -> BTreeSet<Vec<usize>> {
    (0..1_usize << theory.atom_count())
        .filter_map(|mask| {
            let model = Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
            )
            .unwrap();
            zetesis_ferraris::check(
                theory,
                &model,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
            .then(|| model.atoms().collect())
        })
        .collect()
}
