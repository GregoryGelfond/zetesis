//! A narrowing on the search budget sees cancellation at its next batch of
//! permits, at most `NARROWING_BATCH` charged reads after it was raised.

use std::cell::Cell;

use zetesis_cpu::regions::Region;
use zetesis_ferraris::{NARROWING_BATCH, Narrower, Node, Theory};

use super::super::{CandidateKnowledge, Conditions, RegionCounts, narrow};

use crate::search::{Budget, LocalQuota, Quota, SharedBudget};
use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};

/// A held root forces a chain of implications over `atoms` atoms: a closure
/// of many more charged reads than one batch.
fn implication_chain(atoms: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..atoms).map(Node::Atom).collect();
    let mut roots = vec![0];
    for atom in 0..atoms - 1 {
        roots.push(nodes.len());
        nodes.push(Node::Implies(atom, atom + 1));
    }
    Theory::new(
        atoms,
        nodes,
        roots,
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

/// Delegates to `inner`, and raises cancellation once it has made its first
/// grant, so the narrowing runs on with permits in hand.
struct CancelAfterFirstGrant<'c, Q> {
    inner: Q,
    cancellation: &'c Cancellation,
    granted: Cell<bool>,
}

impl<Q: Quota> Quota for CancelAfterFirstGrant<'_, Q> {
    fn work(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.inner.work(spent, ceiling)
    }
    fn decision(&self, spent: u64, ceiling: u64) -> Result<(), Incomplete> {
        self.inner.decision(spent, ceiling)
    }
    fn charge(&self, spent: u64, ceiling: u64, amount: u64) -> Result<(), Incomplete> {
        self.inner.charge(spent, ceiling, amount)
    }
    fn reserve_up_to(&self, spent: u64, ceiling: u64, wanted: u64) -> Result<u64, Incomplete> {
        let granted = self.inner.reserve_up_to(spent, ceiling, wanted)?;
        if !self.granted.replace(true) {
            self.cancellation.cancel();
        }
        Ok(granted)
    }
    fn refund(&self, unspent: u64) {
        self.inner.refund(unspent);
    }
}

/// Narrow the open region of a long chain with `quota`, cancelled after its
/// first grant; the stop and the reads it admitted.
fn cancelled_narrowing<Q: Quota>(quota: Q, cancellation: &Cancellation) -> (Incomplete, u64) {
    let theory = implication_chain(4 * usize::try_from(NARROWING_BATCH).unwrap());
    let narrower = Narrower::new(&theory);
    let mut budget = Budget {
        quota: CancelAfterFirstGrant {
            inner: quota,
            cancellation,
            granted: Cell::new(false),
        },
        limits: SearchLimits::default(),
        cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = RegionCounts::default();
    let stop = narrow(
        (&theory, &narrower, None),
        &Conditions::<(Theory, Narrower)>::default(),
        &mut Region::all_open(theory.atom_count()),
        &mut CandidateKnowledge::new(narrower.knowledge()),
        &mut zetesis_ferraris::NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    )
    .expect_err("cancellation stops the narrowing");
    (stop, counts.work)
}

#[test]
fn a_local_budget_sees_cancellation_within_one_batch() {
    let cancellation = Cancellation::default();
    let (stop, work) = cancelled_narrowing(LocalQuota, &cancellation);
    assert_eq!(stop, Incomplete::Cancelled);
    assert!(work <= NARROWING_BATCH, "{work} reads after cancellation");
}

#[test]
fn a_shared_lease_sees_cancellation_within_one_batch() {
    let cancellation = Cancellation::default();
    let shared = SharedBudget::new(SearchLimits::default(), SearchStatistics::default());
    let (stop, work) = cancelled_narrowing(shared.lease(&cancellation), &cancellation);
    assert_eq!(stop, Incomplete::Cancelled);
    assert!(work <= NARROWING_BATCH, "{work} reads after cancellation");
}
