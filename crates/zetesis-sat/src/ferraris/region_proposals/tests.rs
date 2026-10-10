use std::mem::size_of;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{CandidateKnowledge, Frontier, PendingRegion, Workspace};
use crate::{
    Cancellation, Incomplete, Limits, RegionFeasibility, RegionFilter, RegionFilterWorker,
    StableModels,
};
use zetesis_ferraris::{AdmissionLimits, Narrower, Node, Region, Theory};

fn index() -> Narrower {
    let theory = Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0), Node::atom(1)], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    Narrower::new(&theory)
}

/// A region after its first narrowing, knowing only the original theory.
fn entry() -> PendingRegion {
    let mut knowledge = CandidateKnowledge::default();
    knowledge.permanent(0, &index()).unwrap();
    (Region::all_open(2), knowledge)
}

#[test]
fn frontier_counts_unused_entry_slots() {
    let entry = entry();
    let entry_bytes = entry.0.retained_bytes()
        + size_of::<CandidateKnowledge>() as u128
        + entry.1.allocated_bytes();
    let mut frontier = Frontier::new(entry).unwrap();
    frontier.try_reserve(7).unwrap();
    let observed = frontier.statistics;
    assert_eq!(observed.regions, 1);
    assert!(observed.capacity >= 8);
    assert_eq!(
        observed.retained_bytes,
        size_of::<Vec<PendingRegion>>() as u128
            + entry_bytes
            + (observed.capacity - 1) as u128 * size_of::<PendingRegion>() as u128
    );
}

#[test]
fn removing_regions_preserves_capacity_and_peak() {
    let mut frontier = Frontier::new(entry()).unwrap();
    frontier.try_reserve(1).unwrap();
    frontier.push(entry());
    let populated = frontier.statistics;
    assert_eq!(populated.peak_regions, 2);
    let _active = frontier.pop().unwrap();
    let _other = frontier.pop().unwrap();
    let empty = frontier.statistics;
    assert_eq!(empty.regions, 0);
    assert_eq!(empty.capacity, populated.capacity);
    assert_eq!(
        empty.retained_bytes,
        size_of::<Vec<PendingRegion>>() as u128
            + empty.capacity as u128 * size_of::<PendingRegion>() as u128
    );
    assert_eq!(empty.peak_retained_bytes, populated.retained_bytes);
    assert_eq!(empty.peak_regions, 2);
    assert_eq!(empty.peak_capacity, populated.capacity);
}

#[test]
fn returning_active_knowledge_counts_its_new_capacity() {
    let mut frontier = Frontier::new(entry()).unwrap();
    let initial = frontier.statistics;
    let mut active = frontier.pop().unwrap();
    assert!(active.0.hold(1));
    // Restrictions added while a region was pending acquire fresh knowledge
    // only when it is narrowed, outside the frontier's ownership.
    active.1.permanent(1, &index()).unwrap();
    frontier.push(active);
    assert!(frontier.statistics.retained_bytes > initial.retained_bytes);
    assert_eq!(frontier.statistics.regions, 1);
    assert_eq!(
        frontier.statistics.peak_retained_bytes,
        frontier.statistics.retained_bytes
    );
}

#[derive(Debug)]
struct PanickingFilter {
    dropped: AtomicUsize,
    fail_check: bool,
}

struct PanickingWorker<'a>(&'a PanickingFilter);

impl Drop for PanickingWorker<'_> {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
        panic!("injected checker teardown failure");
    }
}

impl RegionFilter for PanickingFilter {
    fn worker(
        &self,
        _: &Theory,
        _: &Cancellation,
    ) -> Result<Box<dyn RegionFilterWorker + '_>, Incomplete> {
        Ok(Box::new(PanickingWorker(self)))
    }
}

impl RegionFilterWorker for PanickingWorker<'_> {
    fn check(
        &mut self,
        _: &Theory,
        _: &Region,
        _: &Cancellation,
    ) -> Result<RegionFeasibility, Incomplete> {
        if self.0.fail_check {
            Err(Incomplete::RegionFilter)
        } else {
            Ok(RegionFeasibility::NotRefuted)
        }
    }
}

fn panicking_search(fail_check: bool) -> (StableModels, Arc<PanickingFilter>) {
    let theory = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::default(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut search = StableModels::with_region_producers(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let filter = Arc::new(PanickingFilter {
        dropped: AtomicUsize::new(0),
        fail_check,
    });
    search.set_region_filter(filter.clone()).unwrap();
    (search, filter)
}

/// Prepare both slots directly to make simultaneous teardown obligations
/// independent of how many Rayon tasks a small candidate round happens to use.
fn retain_panicking_checkers(search: &mut StableModels) {
    let crate::ferraris::Proposer::Proposals(proposals) = &mut search.proposer else {
        panic!("expected joined production");
    };
    proposals.workers.with_dependent_mut(|owner, workspaces| {
        let filter = owner.as_ref().unwrap();
        for _ in 0..2 {
            let mut workspace = Workspace::default();
            filter
                .check(
                    &mut workspace.filter,
                    &search.theory,
                    &Region::all_open(search.theory.atom_count()),
                    &search.cancellation,
                    &mut None,
                )
                .unwrap();
            workspaces.push(workspace);
        }
    });
}

#[test]
fn teardown_contains_each_checker_panic() {
    let (mut search, filter) = panicking_search(false);
    retain_panicking_checkers(&mut search);
    let receipt = search.statistics();
    assert_eq!(search.stop(), Err(Incomplete::WorkerPanicked));
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 2);
    assert_eq!(search.statistics(), receipt);
    assert!(!search.exhausted());
    assert!(search.next().is_none());
    assert_eq!(search.stop(), Ok(()));
}

#[test]
fn iterator_drop_contains_checker_panics() {
    let (mut search, filter) = panicking_search(false);
    retain_panicking_checkers(&mut search);
    drop(search);
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 2);
}

#[test]
fn source_failure_survives_checker_drop_panic() {
    let (mut search, filter) = panicking_search(true);
    assert!(matches!(search.next(), Some(Err(Incomplete::RegionFilter))));
    assert_eq!(filter.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(search.statistics().region_filter.unwrap().failed, 1);
    assert!(!search.exhausted());
    assert!(search.next().is_none());
}
