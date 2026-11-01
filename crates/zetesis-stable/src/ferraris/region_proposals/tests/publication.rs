//! Failed publication preserves emitted and pending coverage until teardown.

use super::*;

fn values(region: &Region) -> Vec<usize> {
    (0..1_usize << region.len())
        .filter(|value| {
            (0..region.len()).all(|atom| {
                region
                    .decision(atom)
                    .is_none_or(|held| (value & (1 << atom) != 0) == held)
            })
        })
        .collect()
}

#[test]
fn earlier_emissions_and_a_refused_parent_keep_the_full_partition() {
    let driver = driver(tautologies(3), 2);
    let slate = driver.slate(8);
    let lease = driver.allowance.lease(&driver.cancellation);
    let mut hand = slate.admit(0, Attempt::Outside, &lease).unwrap();
    // Three admitted splits leave one sibling per step, with the all-held leaf in hand.
    for _ in 0..3 {
        let Step::Split(atom) = step(&slate, &mut hand) else {
            panic!("an open tautology splits");
        };
        let mut pending = Some(hand);
        hand = slate
            .hand_over(0, &mut pending, atom, reserve_regions)
            .unwrap();
        assert!(pending.is_none(), "an admitted split consumes the parent");
    }
    let Step::Candidate(last) = step(&slate, &mut hand) else {
        panic!("the held path is a leaf");
    };
    slate.emit(last);
    let mut sibling = slate.frontier.take_local(0).unwrap();
    let Step::Candidate(previous) = step(&slate, &mut sibling) else {
        panic!("its one-atom sibling is a leaf");
    };
    slate.emit(previous);
    let mut parent = slate.frontier.take_local(0).unwrap();
    let mut budget = Budget {
        quota: slate.allowance.lease(slate.cancellation),
        limits: slate.limits,
        cancellation: slate.cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut counts = super::super::RegionCounts::default();
    let Step::Split(atom) = slate
        .step(
            &mut parent,
            &mut Workspace::default(),
            &mut budget,
            &mut counts,
        )
        .unwrap()
    else {
        panic!("the next sibling has one open atom");
    };
    assert_eq!(budget.statistics.decisions, 1);
    let prefix = budget.statistics;
    let frontier = slate.frontier.statistics();
    let mut pending = Some(parent);
    let error = slate
        .hand_over(0, &mut pending, atom, |queue, _| {
            reserve_regions(queue, usize::MAX)
        })
        .unwrap_err();
    let parent = pending.take().expect("refusal retains the whole parent");
    assert_eq!(error, Incomplete::Allocation);
    assert_eq!(
        budget.statistics, prefix,
        "publication adds no decision or work charge"
    );
    assert_eq!(slate.frontier.statistics(), frontier);
    assert_eq!(slate.frontier.termination.unresolved(), 2);
    let mut represented: Vec<_> = slate
        .outcome()
        .candidates
        .iter()
        .map(|model| model.atoms().fold(0usize, |bits, atom| bits | (1 << atom)))
        .collect();
    represented.extend(values(&parent.region));
    for queued in slate.frontier.queue(0).iter() {
        represented.extend(values(&queued.region));
    }
    represented.sort_unstable();
    assert_eq!(
        represented,
        (0..8).collect::<Vec<_>>(),
        "no loss or duplicate coverage"
    );
    assert!(stopped(&slate).is_none());
    assert!(!slate.frontier.termination.is_closed());
    // Explicit terminal teardown follows the proved pending boundary.
    slate.stop(error);
    drop(parent);
    assert_eq!(stopped(&slate), Some(Incomplete::Allocation));
    assert_eq!(emitted(&slate), 2);
    assert!(!slate.frontier.termination.is_resolved());
}
