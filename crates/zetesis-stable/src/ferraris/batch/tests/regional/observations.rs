//! Opt-in receipts preserve original slots, including partial regional attempts.
use super::*;
use crate::{RegionalSlotObservation, RegionalSlotState};

#[test]
fn scalar_and_parallel_receipts_reconcile_actual_queries_in_slate_order() {
    for workers in [1, 2] {
        let (mut search, ticket, present) = conditional_slate();
        let id = ticket.slate();
        let mut completion = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        search.enable_phase_timing();
        let before = search.statistics();
        completion.observe_next_regional_pass(13, 8192);
        let models = search
            .begin_regional_pass(
                ticket,
                all_regional(),
                subset_roots(present),
                &mut completion,
            )
            .unwrap()
            .finish(decisions(present))
            .unwrap();
        assert_eq!(models.len(), 1);
        let actual = completion.take_regional_observations().unwrap();
        assert!(completion.last_regional_observations().is_none());
        assert_eq!((actual.slate(), actual.attempt()), (id, 13));
        let rows = actual.slots().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[present].unwrap().state,
            RegionalSlotState::ValidatedCountermodel
        );
        assert_eq!(
            rows[1 - present].unwrap().state,
            RegionalSlotState::NoWitnessInSuppliedRegion
        );
        let mut visits = 0;
        let mut work = 0;
        let mut host = std::time::Duration::ZERO;
        for slot in rows {
            let q = slot.unwrap().query.unwrap();
            visits += q.visited;
            work += q.work;
            host += q.host_time.unwrap();
            assert_eq!(q.attempted_splits, q.published_splits);
            assert!(q.deepest.is_some());
        }
        let after = search.statistics();
        assert_eq!(
            visits,
            after.reduct.regions.regions - before.reduct.regions.regions
        );
        assert_eq!(work, after.search.work - before.search.work);
        assert_eq!(actual.host_time(), Some(host));
        let measured = if workers == 1 {
            after.phase_timings.unwrap().reduct
        } else {
            completion.last_statistics().unwrap().worker_reduct.unwrap()
        };
        assert_eq!(measured.calls, 2);
        assert_eq!(host, measured.elapsed);
        assert_eq!(actual.width(), Some(2));
        assert!(actual.peak_bytes() > actual.retained_bytes());
        assert!(actual.peak_bytes() <= 8192);
    }
}

#[test]
fn missing_optional_storage_preserves_query_work_and_verdicts() {
    let records = (2 * size_of::<Option<RegionalSlotObservation>>()) as u64;
    let mut reference = None;
    for bytes in [0, records, 8192] {
        let (mut search, ticket, present) = conditional_slate();
        let mut completion = CompletionExecutor::default();
        completion.observe_next_regional_pass(8, bytes);
        let pass = search
            .begin_regional_pass(
                ticket,
                all_regional(),
                subset_roots(present),
                &mut completion,
            )
            .unwrap();
        assert_eq!(pass.finish(decisions(present)).unwrap().len(), 1);
        let receipt = completion.take_regional_observations().unwrap();
        assert!(receipt.peak_bytes() <= bytes);
        assert!(receipt.host_time().is_none());
        let stats = search.statistics();
        if let Some(expected) = reference {
            assert_eq!(stats, expected);
        } else {
            reference = Some(stats);
        }
        if bytes == 0 {
            assert!(receipt.slots().is_none());
            assert_eq!(receipt.width(), None);
            assert_eq!(receipt.retained_bytes(), 0);
        } else if bytes == records {
            assert_eq!(receipt.retained_bytes(), records);
            assert_eq!(receipt.width(), None);
            for slot in receipt.slots().unwrap() {
                let q = slot.unwrap().query.unwrap();
                assert!(q.work > 0 && q.visited > 0);
                assert_eq!(q.deepest, None);
                assert_eq!(q.host_time, None);
                assert_eq!(q.attempted_splits, q.published_splits);
            }
        } else {
            assert_eq!(receipt.width(), Some(2));
        }
    }
}

#[test]
fn an_exhausted_work_allowance_distinguishes_a_stopped_query_and_unentered_slot() {
    let (mut search, ticket, present) = conditional_slate();
    let mut completion = CompletionExecutor::default();
    search.limits.search.max_work = search.statistics().search.work;
    completion.observe_next_regional_pass(7, 8192);
    assert!(matches!(
        search.begin_regional_pass(
            ticket,
            all_regional(),
            subset_roots(present),
            &mut completion
        ),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let receipt = completion.take_regional_observations().unwrap();
    let rows = receipt.slots().unwrap();
    assert_eq!(
        rows[0].unwrap().state,
        RegionalSlotState::Stopped(Incomplete::WorkLimit)
    );
    let prefix = rows[0].unwrap().query.unwrap();
    assert_eq!(
        (
            prefix.work,
            prefix.visited,
            prefix.attempted_splits,
            prefix.published_splits
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(receipt.width(), Some(0));
    assert_eq!(
        rows[1],
        Some(RegionalSlotObservation {
            state: RegionalSlotState::Unentered,
            query: None
        })
    );
}

#[test]
fn retries_have_distinct_receipts_and_terminal_slots_have_no_region_observation() {
    let (mut search, ticket, present) = conditional_slate();
    let id = ticket.slate();
    let mut completion = CompletionExecutor::default();
    completion.observe_next_regional_pass(3, 8192);
    let pass = search
        .begin_regional_pass(
            ticket,
            all_regional(),
            subset_roots(present),
            &mut completion,
        )
        .unwrap();
    drop(pass);
    let first = completion.take_regional_observations().unwrap();
    assert_eq!((first.slate(), first.attempt()), (id, 3));
    let mut tags = all_regional();
    tags[0] = BatchVerdict::NoProperSubset;
    let roots = vec![(present, Region::all_open(1))];
    completion.observe_next_regional_pass(4, 8192);
    let ticket = retry(&mut search, id);
    let pass = search
        .begin_regional_pass(ticket, tags, roots, &mut completion)
        .unwrap();
    assert_eq!(pass.finish(vec![BatchVerdict::Refuted]).unwrap().len(), 1);
    let second = completion.take_regional_observations().unwrap();
    assert_eq!((second.slate(), second.attempt()), (id, 4));
    assert!(second.slots().unwrap()[0].is_none());
    assert_eq!(
        second.slots().unwrap()[present].unwrap().state,
        RegionalSlotState::ValidatedCountermodel
    );
}

#[test]
fn a_certified_original_residual_has_no_entered_region_query() {
    let owner = theory(1, vec![Node::atom(0)], vec![0]);
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let ticket = new_slate(&mut search, one_candidate());
    let mut completion = CompletionExecutor::default();
    completion.observe_next_regional_pass(0, 8192);
    let pass = search
        .begin_regional_pass(
            ticket,
            vec![BatchVerdict::Residual],
            vec![(0, Region::all_open(1))],
            &mut completion,
        )
        .unwrap();
    assert_eq!(pass.finish(vec![]).unwrap().len(), 1);
    let receipt = completion.take_regional_observations().unwrap();
    assert_eq!(
        receipt.slots().unwrap(),
        [Some(RegionalSlotObservation {
            state: RegionalSlotState::Certified,
            query: None
        })]
    );
    assert_eq!(receipt.width(), Some(0));
    assert_eq!(receipt.retained_bytes(), receipt.peak_bytes());
}

#[test]
fn slate_width_sums_depth_bins_for_unequal_query_trees() {
    for workers in [1, 2] {
        let owner = theory(2, vec![], vec![]);
        let (mut search, ticket, present) = paired_slate(&owner, &[0, 1]);
        let mut empty = Region::all_open(2);
        assert!(empty.cut(0) && empty.cut(1));
        let mut completion = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        completion.observe_next_regional_pass(31, 8192);
        let pass = search
            .begin_regional_pass(
                ticket,
                all_regional(),
                vec![(0, empty), (present, Region::all_open(2))],
                &mut completion,
            )
            .unwrap();
        assert_eq!(pass.finish(decisions(present)).unwrap().len(), 1);
        let receipt = completion.take_regional_observations().unwrap();
        let rows = receipt.slots().unwrap();
        let first = rows[0].unwrap().query.unwrap();
        let second = rows[present].unwrap().query.unwrap();
        assert_eq!(
            (first.visited, first.published_splits, first.deepest),
            (1, 0, Some(0))
        );
        assert_eq!(
            (second.visited, second.published_splits, second.deepest),
            (3, 2, Some(2))
        );
        // Visits per level are [1] and [1,1,1]. A maximum of the workers'
        // individual widths would underreport the joined root level.
        assert_eq!(receipt.width(), Some(2));
    }
}

#[test]
fn a_stopped_certificate_preserves_the_settled_prefix_and_unentered_suffix() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 0),
            Node::implies(1, 1),
        ],
        vec![],
    );
    let open = || {
        let mut search =
            StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
        assert!(
            search
                .enable_certified_checking(TightPlanLimits::default())
                .unwrap()
        );
        admitted_slate(
            search,
            vec![
                Interpretation::new(&owner, []).unwrap(),
                Interpretation::new(&owner, [0]).unwrap(),
                Interpretation::new(&owner, [0, 1]).unwrap(),
            ],
        )
    };
    let (mut control, _) = open();
    let before = control.statistics().search.work;
    let mut first = [
        BatchVerdict::Residual,
        BatchVerdict::Refuted,
        BatchVerdict::Refuted,
    ];
    control.certify_pending(&mut first).unwrap();
    assert_eq!(first[0], BatchVerdict::NoProperSubset);
    let allowance = control.statistics().search.work - before;
    let (mut search, ticket) = open();
    search.limits.search.max_work = search.statistics().search.work + allowance;
    let roots = search
        .slate(&ticket)
        .unwrap()
        .candidates()
        .iter()
        .enumerate()
        .map(|(slot, c)| {
            let mut root = Region::all_open(2);
            for atom in 0..2 {
                if !c.contains(atom) {
                    assert!(root.cut(atom));
                }
            }
            (slot, root)
        })
        .collect();
    let mut executor = CompletionExecutor::default();
    executor.observe_next_regional_pass(1, 8192);
    assert!(matches!(
        search.begin_regional_pass(
            ticket,
            vec![BatchVerdict::Residual; 3],
            roots,
            &mut executor
        ),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    let r = executor.take_regional_observations().unwrap();
    assert_eq!(
        r.slots().unwrap(),
        [
            RegionalSlotState::Certified,
            RegionalSlotState::Stopped(Incomplete::WorkLimit),
            RegionalSlotState::Unentered
        ]
        .map(|state| Some(RegionalSlotObservation { state, query: None }))
    );
    assert_eq!(r.width(), Some(0));
}

#[test]
fn invalid_tickets_discard_a_previous_receipt_and_consume_the_observation_request() {
    let (mut search, ticket, present) = conditional_slate();
    let slate = ticket.slate();
    let mut executor = CompletionExecutor::default();
    executor.observe_next_regional_pass(11, 8192);
    drop(
        search
            .begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor)
            .unwrap(),
    );
    assert_eq!(executor.last_regional_observations().unwrap().attempt(), 11);
    let (_, foreign, _) = conditional_slate();
    executor.observe_next_regional_pass(12, 8192);
    assert!(matches!(
        search.begin_regional_pass(
            foreign,
            all_regional(),
            subset_roots(present),
            &mut executor
        ),
        Err(BatchError::SlateOrder)
    ));
    assert!(executor.last_regional_observations().is_none());
    let ticket = retry(&mut search, slate);
    let pass = search
        .begin_regional_pass(ticket, all_regional(), subset_roots(present), &mut executor)
        .unwrap();
    assert_eq!(pass.finish(decisions(present)).unwrap().len(), 1);
    assert!(executor.take_regional_observations().is_none());
}

#[test]
fn an_entirely_terminal_slate_records_exact_zero_without_observation_storage() {
    let (mut search, ticket, present) = conditional_slate();
    search.enable_phase_timing();
    let mut executor = CompletionExecutor::default();
    executor.observe_next_regional_pass(1, 0);
    assert_eq!(
        search
            .begin_regional_pass(ticket, decisions(present), vec![], &mut executor)
            .unwrap()
            .finish(vec![])
            .unwrap()
            .len(),
        1
    );
    let r = executor.take_regional_observations().unwrap();
    assert_eq!(r.slots(), Some([].as_slice()));
    assert_eq!(r.width(), Some(0));
    assert_eq!(r.host_time(), Some(std::time::Duration::ZERO));
    assert_eq!((r.peak_bytes(), r.retained_bytes()), (0, 0));
}

fn cut_outside(atoms: usize, open: usize) -> Region {
    let mut root = Region::all_open(atoms);
    for i in open..atoms {
        assert!(root.cut(i));
    }
    root
}

#[test]
fn a_nearly_settled_large_root_fits_two_depth_slots() {
    let atoms = 4097;
    let owner = theory(atoms, vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    for workers in [1, 2] {
        let (mut search, ticket, present) = paired_slate(&owner, &[0]);
        let roots = vec![(0, cut_outside(atoms, 0)), (present, cut_outside(atoms, 1))];
        let mut completion = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        completion.observe_next_regional_pass(1, 2048);
        let models = search
            .begin_regional_pass(ticket, all_regional(), roots, &mut completion)
            .unwrap()
            .finish(decisions(present))
            .unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].atoms().next().is_none());
        let measured = completion.last_regional_observations().unwrap();
        let slots = measured.slots().unwrap();
        assert_eq!(slots[0].unwrap().query.unwrap().deepest, Some(0));
        assert_eq!(slots[present].unwrap().query.unwrap().deepest, Some(1));
        assert_eq!(measured.width(), Some(2));
        assert!(measured.peak_bytes() <= 2048);
    }
}

#[test]
fn a_declined_chunk_does_not_discard_other_depths() {
    let atoms = 257;
    let owner = theory(atoms, vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    let search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    let candidates = vec![
        Interpretation::new(&owner, []).unwrap(),
        Interpretation::new(&owner, 0..atoms).unwrap(),
        Interpretation::new(&owner, [0]).unwrap(),
    ];
    let (mut search, ticket) = admitted_slate(search, candidates);
    let mut completion = CompletionExecutor::new(NonZeroUsize::new(3).unwrap()).unwrap();
    completion.observe_next_regional_pass(7, 1024);
    let roots = vec![
        (0, cut_outside(atoms, 0)),
        (1, Region::all_open(atoms)),
        (2, cut_outside(atoms, 1)),
    ];
    let pass = search
        .begin_regional_pass(
            ticket,
            vec![BatchVerdict::Residual; 3],
            roots,
            &mut completion,
        )
        .unwrap();
    let models = pass
        .finish(vec![
            BatchVerdict::NoProperSubset,
            BatchVerdict::Refuted,
            BatchVerdict::Refuted,
        ])
        .unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].atoms().next().is_none());
    let measured = completion.last_regional_observations().unwrap();
    let slots = measured.slots().unwrap();
    assert_eq!(slots[0].unwrap().query.unwrap().deepest, Some(0));
    let declined = slots[1].unwrap().query.unwrap();
    assert!(declined.visited > 0);
    assert!(declined.deepest.is_none());
    assert_eq!(slots[2].unwrap().query.unwrap().deepest, Some(1));
    assert!(measured.width().is_none());
    assert!(measured.peak_bytes() <= 1024);
}
