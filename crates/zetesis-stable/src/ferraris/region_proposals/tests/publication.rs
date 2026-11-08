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
            .copy_and_hand_over(0, &mut pending, atom, reserve_regions)
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
    let copied = super::super::regions::copy_knowledge(&parent.state, None, &mut budget);
    counts.add(copied.counts).unwrap();
    let cut_knowledge = copied.result.unwrap();
    let prefix = budget.statistics;
    let frontier = slate.frontier.statistics();
    let mut pending = Some(parent);
    let error = slate
        .hand_over(0, &mut pending, atom, cut_knowledge, |queue, _| {
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

fn split_copy_prefix() -> (SearchStatistics, super::super::RegionCounts, u64) {
    let driver = driver(tautologies(1), 2);
    let slate = driver.slate(1);
    let mut budget = Budget {
        quota: slate.allowance.lease(slate.cancellation),
        limits: slate.limits,
        cancellation: slate.cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut root = slate.admit(0, Attempt::Outside, &budget.quota).unwrap();
    assert!(
        slate
            .parts
            .reading(slate.theory, slate.narrower)
            .unwrap()
            .arithmetic
            .is_some()
    );
    let mut counts = super::super::RegionCounts::default();
    assert!(matches!(
        slate
            .step(
                &mut root,
                &mut Workspace::default(),
                &mut budget,
                &mut counts,
            )
            .unwrap(),
        Step::Split(_)
    ));
    let prefix = budget.statistics;
    let copied = super::super::regions::copy_knowledge(&root.state, None, &mut budget);
    assert!(copied.result.is_ok());
    assert!(copied.counts.work > 0);
    assert_eq!(budget.statistics.work - prefix.work, copied.counts.work);
    (prefix, counts, copied.counts.work)
}

#[test]
fn every_refused_copy_prefix_keeps_the_slate_incomplete() {
    let (prefix, before_copy, copy_work) = split_copy_prefix();
    for allowed_copy_work in 0..copy_work {
        let mut driver = driver(tautologies(1), 2);
        driver.allowance = SharedBudget::new(
            SearchLimits {
                max_work: prefix.work + allowed_copy_work,
                ..Default::default()
            },
            SearchStatistics::default(),
        );
        let slate = driver.slate(1);
        let mut workspace = Workspace::default();
        slate.produce(0, &mut workspace);
        assert_eq!(stopped(&slate), Some(Incomplete::WorkLimit));
        assert_eq!(
            workspace.counts.regions, 1,
            "no child is published or narrowed after a refused copy"
        );
        assert_eq!(workspace.counts.leaves, 0);
        assert_eq!(workspace.counts.refuted, 0);
        assert_eq!(emitted(&slate), 0);
        assert_eq!(slate.frontier.termination.unresolved(), 1);
        assert!(slate.frontier.termination.is_closed());
        assert!(!slate.frontier.termination.is_resolved());
        assert!(slate.frontier.queue(0).is_empty());
        assert!(slate.frontier.queue(1).is_empty());
        let mut committed = SearchStatistics::default();
        driver.allowance.record(&mut committed);
        assert_eq!(
            committed.decisions, 1,
            "the existing decision debit precedes the copy"
        );
        assert!(committed.work >= prefix.work);
        assert!(committed.work <= prefix.work + allowed_copy_work);
        assert_eq!(
            workspace.counts.work - before_copy.work,
            committed.work - prefix.work,
            "the same lease and one copy receipt preserve the executed prefix"
        );
    }
}

#[test]
fn a_refused_copy_retains_the_exact_pending_parent() {
    let (prefix, _, _) = split_copy_prefix();
    let mut driver = driver(tautologies(1), 2);
    driver.allowance = SharedBudget::new(
        SearchLimits {
            max_work: prefix.work,
            ..Default::default()
        },
        SearchStatistics::default(),
    );
    let slate = driver.slate(1);
    let mut budget = Budget {
        quota: slate.allowance.lease(slate.cancellation),
        limits: slate.limits,
        cancellation: slate.cancellation,
        statistics: SearchStatistics::default(),
    };
    let mut root = slate.admit(0, Attempt::Outside, &budget.quota).unwrap();
    let mut counts = super::super::RegionCounts::default();
    let Step::Split(atom) = slate
        .step(
            &mut root,
            &mut Workspace::default(),
            &mut budget,
            &mut counts,
        )
        .unwrap()
    else {
        panic!("the root remains open");
    };
    root.prefer(atom);
    let interval = root.region.clone();
    let known = format!("{:?}", root.state);
    let before = counts;
    let receipts = slate.frontier.statistics();
    let mut pending = Some(root);
    let error = slate
        .split(0, &mut pending, atom, &mut counts, &mut budget)
        .unwrap_err();
    let parent = pending
        .as_ref()
        .expect("the caller still owns its unresolved parent");
    assert_eq!(error, Incomplete::WorkLimit);
    assert_eq!(parent.region, interval);
    assert_eq!(parent.split_atom(), Some(atom));
    assert_eq!(format!("{:?}", parent.state), known);
    assert_eq!(
        counts, before,
        "the failed first copy chunk executed no work"
    );
    assert_eq!(budget.statistics, prefix);
    assert_eq!(slate.frontier.statistics(), receipts);
    assert_eq!(slate.frontier.termination.unresolved(), 1);
    assert_eq!(stopped(&slate), None);
    assert!(!slate.frontier.termination.is_closed());
    // The actual producer uses this same ordering before terminal teardown.
    slate.stop(error);
    drop(pending);
    assert_eq!(stopped(&slate), Some(Incomplete::WorkLimit));
    assert!(!slate.frontier.termination.is_resolved());
}
