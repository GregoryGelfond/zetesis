//! A refused served-query split retains its exact owner before abandonment.

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
fn refused_parent_and_peer_partition_the_query_before_retirement() {
    let theory = saturation(2, &every_clause(2));
    let (search, _) = walk(&theory, QueryDivision::Immediate, Limits::default());
    let shared = &search.shared;
    let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
    let truth = vec![true; theory.nodes().len()];
    let cell = Arc::new(QueryCell::new(&truth, &candidate, 2).unwrap());
    let root = Entry::new(
        Region::all_open(theory.atom_count()),
        Narrower::new(&theory).knowledge(),
    );
    let (mut parent, peer) = root.split_with(0, |state| (state.clone(), state));
    parent.prefer(1);
    let (held, decided) = parent.region.decision_words();
    let allocations = (held.as_ptr(), decided.as_ptr());
    let cut_knowledge = parent.state.clone();
    let mut pending = Some(InnerEntry::new(parent, &cell));
    let refused = shared.inner.publish_split(
        1,
        &mut pending,
        1,
        cut_knowledge,
        &shared.frontier.termination,
        |queue, _| reserve_entries(queue, usize::MAX),
    );
    let Err(error) = refused else {
        panic!("capacity overflow must refuse publication");
    };
    let parent = pending.take().expect("refusal retains the whole parent");
    assert_eq!(error, Incomplete::Allocation);
    let (held, decided) = parent.entry.region.decision_words();
    assert_eq!((held.as_ptr(), decided.as_ptr()), allocations);
    assert_eq!(parent.entry.split_atom(), Some(1));
    assert!(Arc::ptr_eq(&parent.cell, &cell));
    assert_eq!(cell.truth, truth);
    assert!(cell.candidate.theory().same_instance(candidate.theory()));
    assert_eq!(
        cell.candidate.atoms().collect::<Vec<_>>(),
        candidate.atoms().collect::<Vec<_>>()
    );
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 2);
    let mut represented = values(&parent.entry.region);
    represented.extend(values(&peer.region));
    represented.sort_unstable();
    assert_eq!(
        represented,
        (0..1_usize << theory.atom_count()).collect::<Vec<_>>()
    );
    assert!(cell.stop.get().is_none());
    assert_eq!(shared.stopped(), None);
    // A peer's ordinary retirement races the selected terminal abandonment.
    // Whichever retires last sees the stop and cannot establish stability.
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let retired = scope.spawn(|| {
            start.wait();
            let stable = cell.retire();
            drop(peer);
            stable
        });
        start.wait();
        abandon(shared, parent, error);
        assert!(!retired.join().unwrap());
    });
    assert_eq!(cell.outstanding.load(Ordering::Acquire), 0);
    assert_eq!(cell.stop.get(), Some(&Incomplete::Allocation));
    assert_eq!(shared.stopped(), Some(Incomplete::Allocation));
    assert!(!cell.is_resolved());
    assert!(!shared.frontier.termination.is_resolved());
    assert!(shared.inner.take_local(1).is_none());
}
