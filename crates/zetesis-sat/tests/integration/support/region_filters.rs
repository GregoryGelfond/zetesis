//! Borrowed original-only filter fixtures shared by the route controls.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zetesis_cpu::regions::Region;
use zetesis_ferraris::{Node, Theory};
use zetesis_sat::{
    BatchLimits, Cancellation, Incomplete, Limits, RegionFeasibility, RegionFilter,
    RegionFilterWorker, StableModels,
};
use zetesis_theory_support::theories::theory;

use super::batching::residual;

#[derive(Clone, Copy, Debug)]
pub enum Route {
    Scalar,
    Producers,
    Native,
}

pub const ROUTES: [Route; 3] = [Route::Scalar, Route::Producers, Route::Native];

impl Route {
    pub fn search(self, theory: &Theory, cancellation: Cancellation) -> StableModels {
        let workers = NonZeroUsize::new(4).unwrap();
        match self {
            Self::Scalar => StableModels::new(theory, Limits::default(), cancellation),
            Self::Producers => StableModels::with_region_producers(
                theory,
                workers,
                Limits::default(),
                cancellation,
            ),
            Self::Native => {
                StableModels::with_region_workers(theory, workers, Limits::default(), cancellation)
            }
        }
        .unwrap()
    }

    pub fn collect(self, search: &mut StableModels) -> BTreeSet<Vec<usize>> {
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

pub fn choices(atoms: usize) -> Theory {
    let mut nodes = vec![Node::falsum()];
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let offset = nodes.len();
        nodes.extend([
            Node::atom(atom),
            Node::implies(offset, 0),
            Node::or_pair([offset, offset + 1]),
        ]);
        roots.push(offset + 2);
    }
    theory(atoms, nodes, roots)
}

#[derive(Clone, Copy, Debug)]
pub enum Condition {
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
pub struct Filter {
    subject: Theory,
    condition: Condition,
    pub live: AtomicUsize,
}

impl Filter {
    pub fn new(subject: &Theory, condition: Condition) -> Arc<Self> {
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

impl RegionFilter for Filter {
    fn worker(
        &self,
        theory: &Theory,
        cancellation: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
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

impl RegionFilterWorker for Worker<'_> {
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
