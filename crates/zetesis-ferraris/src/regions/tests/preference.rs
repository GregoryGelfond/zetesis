//! Propagation can be composed without spending or replacing split preferences.

use crate::{
    AdmissionLimits, ChoiceHeads, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingScratch,
    Node, OriginalSubject, Region, RegionLimits, Theory,
};
use zetesis_algebra::{Cancellation, Stop, region::Entry};

#[derive(Default)]
struct Allowance {
    remaining: u64,
    granted: u64,
    refunded: u64,
}

impl Allowance {
    fn new(remaining: u64) -> Self {
        Self {
            remaining,
            ..Self::default()
        }
    }
}

impl zetesis_algebra::Quota for Allowance {
    type Error = Stop;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let grant = wanted.min(self.remaining);
        if grant == 0 {
            return Err(Stop::WorkLimit);
        }
        self.remaining -= grant;
        self.granted += grant;
        Ok(grant)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
        self.refunded += unspent;
    }
}

/// `(b → c) → a` leaves every atom open. Its known root removes a's
/// occurrence from the ranking; b and c still share one unknown parent.
fn theory() -> Theory {
    Theory::new(
        3,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::implies(1, 2),
                Node::implies(3, 0),
            ],
            vec![],
        )
        .unwrap(),
        vec![4],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The theory's index, its open region closed, and the closure's knowledge.
fn closed(theory: &Theory) -> (Narrower, Region, Knowledge) {
    let narrower = Narrower::new(theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut known = narrower.knowledge();
    narrower
        .narrow_known(
            OriginalSubject::new(theory, None),
            &mut region,
            &mut known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    (narrower, region, known)
}

#[test]
fn deferred_preference_preserves_existing_split_choice() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let mut known = narrower.knowledge();
    let mut entry = Entry::new(Region::all_open(3), ());
    entry.prefer(2);
    let attempt = crate::test_support::stop_narrowing(&narrower.propagate_known_reserved(
        OriginalSubject::new(&theory, None),
        &mut entry.region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(attempt.preferred, None);
    assert_eq!(entry.split_atom(), Some(2));
}

#[test]
fn separate_preference_has_the_original_narrowing_receipt() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let mut ordinary = Region::all_open(3);
    let attempt = narrower.narrow_known(
        OriginalSubject::new(&theory, None),
        &mut ordinary,
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    );
    attempt.result.unwrap();
    let expected = attempt.statistics;
    let mut region = Region::all_open(3);
    let mut known = narrower.knowledge();
    let mut allowance = Allowance::new(1000);
    let propagated = crate::test_support::stop_narrowing(&narrower.propagate_known_reserved(
        OriginalSubject::new(&theory, None),
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut allowance),
    ));
    propagated.result.unwrap();
    let preference = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut allowance),
    ));
    assert_eq!(preference.result.unwrap(), attempt.preferred);
    assert_eq!(region, ordinary);
    assert_eq!(preference.work, 3);
    assert_eq!(propagated.statistics.work + preference.work, expected.work);
    assert_eq!(allowance.granted - allowance.refunded, expected.work);
}

#[test]
fn preference_refusal_retains_exact_read_prefix() {
    let theory = theory();
    let (narrower, region, known) = closed(&theory);
    for limit in 0..=3 {
        let mut allowance = Allowance::new(limit);
        let selected = crate::test_support::stop_preference(&known.preferred_atom_reserved(
            &narrower,
            &region,
            &ChoiceHeads::default(),
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut allowance),
        ));
        assert_eq!(selected.work, limit);
        assert_eq!(allowance.granted - allowance.refunded, limit);
        assert_eq!(
            selected.result,
            if limit == 3 {
                Ok(Some(1))
            } else {
                Err(Stop::WorkLimit)
            }
        );
    }
}

#[test]
fn cancelled_preference_reads_nothing() {
    let theory = theory();
    let (narrower, region, known) = closed(&theory);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut allowance = Allowance::new(1000);
    let selected = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &ChoiceHeads::default(),
        &cancellation,
        &mut zetesis_algebra::Meter::reserved(&mut allowance),
    ));
    assert_eq!(selected.result, Err(Stop::Cancelled));
    assert_eq!(selected.work, 0);
    assert_eq!(allowance.granted, 0);
}

#[test]
fn zero_parent_counts_prefer_the_lowest_open_atom() {
    let theory = Theory::new(
        130,
        FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let (narrower, mut region, known) = closed(&theory);
    for atom in 0..130 {
        if atom != 64 && atom != 129 {
            assert!(region.cut(atom));
        }
    }
    let selected = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(selected.result, Ok(Some(64)));
    assert_eq!(selected.work, 2);
}

/// `{a0}` is an atomic choice; `a1 :- not a2`, `a2 :- not a1` form an even
/// loop, a constraint rejects both, and `a3` and `a4` each require both
/// through their own unresolved conjunction, so the loop atoms keep more
/// unknown parents than the choice head. Every atom stays open.
fn choice_beside_a_loop() -> Theory {
    choice_beside_a_loop_and(vec![], vec![])
}

/// [`choice_beside_a_loop`] with `nodes` appended after its eighteen and
/// `roots` appended to its six.
fn choice_beside_a_loop_and(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    let mut all = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::atom(3),
        Node::atom(4),
        Node::falsum(),
        Node::implies(0, 5),
        Node::or_pair([0, 6]),
        Node::implies(2, 5),
        Node::implies(8, 1),
        Node::implies(1, 5),
        Node::implies(10, 2),
        Node::and_pair([1, 2]),
        Node::implies(12, 5),
        Node::and_pair([1, 2]),
        Node::implies(14, 3),
        Node::and_pair([1, 2]),
        Node::implies(16, 4),
    ];
    all.extend(nodes);
    let mut asserted = vec![7, 9, 11, 13, 15, 17];
    asserted.extend(roots);
    Theory::new(
        5,
        FormulaParts::new(all, vec![]).unwrap(),
        asserted,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn unknown(known: &Knowledge, atom: usize) -> usize {
    match &known.width {
        crate::regions::Width::Compact16(known) => known.unknown.get(atom),
        crate::regions::Width::Compact32(known) => known.unknown.get(atom),
        crate::regions::Width::Native(known) => known.unknown.get(atom),
    }
}

#[test]
fn choice_heads_rank_before_more_constrained_atoms() {
    let theory = choice_beside_a_loop();
    let extraction =
        crate::producers(&theory, RegionLimits::default(), &Cancellation::default()).unwrap();
    let producers = extraction
        .producers
        .expect("every root is in the producer fragment");
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut known = narrower.knowledge();
    narrower
        .narrow_known(
            OriginalSubject::new(&theory, Some(&producers)),
            &mut region,
            &mut known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert_eq!(region.open().collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
    let ordinary = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    let busiest = ordinary.result.unwrap().unwrap();
    // The premise: the ordinary ranking prefers an atom with more unknown
    // parents than the choice head.
    assert!(unknown(&known, busiest) > unknown(&known, 0));
    let choice = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &extraction.choice_heads,
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(choice.result, Ok(Some(0)));
    // Only the open choice can win; the four unmarked degrees are unread.
    assert_eq!(choice.work, 1);
    assert_eq!(ordinary.work, 5);
}

#[test]
fn choice_heads_are_marked_outside_the_producer_fragment() {
    // A bare conjunction root, a3 ∧ a4, lies outside the producer fragment.
    let theory = choice_beside_a_loop_and(vec![Node::and_pair([3, 4])], vec![18]);
    let extraction =
        crate::producers(&theory, RegionLimits::default(), &Cancellation::default()).unwrap();
    assert!(extraction.producers.is_none());
    let marked: Vec<_> = (0..theory.atom_count())
        .filter(|&atom| extraction.choice_heads.contains(atom))
        .collect();
    assert_eq!(marked, [0]);
}

#[test]
fn a_subject_with_choices_splits_on_a_choice_head() {
    let theory = choice_beside_a_loop();
    let extraction =
        crate::producers(&theory, RegionLimits::default(), &Cancellation::default()).unwrap();
    let narrower = Narrower::new(&theory);
    let split = |subject: OriginalSubject<'_>| {
        let mut entry = Entry::new(Region::all_open(theory.atom_count()), ());
        let attempt = narrower.narrow_known(
            subject,
            &mut entry.region,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        );
        attempt.result.unwrap();
        if let Some(atom) = attempt.preferred {
            entry.prefer(atom);
        }
        entry.split_atom()
    };
    let producers = extraction.producers.as_ref();
    // The premise: without choices the subject splits on a busier atom.
    assert_ne!(split(OriginalSubject::new(&theory, producers)), Some(0));
    assert_eq!(
        split(OriginalSubject::new(&theory, producers).with_choices(&extraction.choice_heads)),
        Some(0)
    );
}

#[test]
fn closed_and_outside_choice_bits_do_not_hide_open_atoms() {
    let theory = Theory::new(
        130,
        FormulaParts::new(vec![], vec![]).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let (narrower, mut region, known) = closed(&theory);
    assert!(region.hold(0));
    assert!(region.cut(64));
    assert!(region.cut(129));
    let choices = ChoiceHeads {
        words: vec![1, 1, !3, u64::MAX].into(),
    };
    let attempt = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &choices,
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(attempt.result, Ok(Some(1)));
    assert_eq!(attempt.work, 127);
    let choices = ChoiceHeads {
        words: vec![1, 1 | (1 << 63), !1, u64::MAX].into(),
    };
    let attempt = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &choices,
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(attempt.result, Ok(Some(127)));
    assert_eq!(attempt.work, 1);
}

#[test]
fn a_stopped_narrowing_publishes_no_partial_preference() {
    let owner = theory();
    let narrower = Narrower::new(&owner);
    let run = |max_work| {
        narrower.narrow_known(
            OriginalSubject::new(&owner, None),
            &mut Region::all_open(owner.atom_count()),
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits { max_work },
            &Cancellation::default(),
        )
    };
    let complete = run(u64::MAX);
    assert_eq!(
        complete.result.unwrap(),
        Narrowing::Fixed { changed: false }
    );
    assert_eq!(complete.preferred, Some(1));
    for limit in 0..complete.statistics.work {
        let stopped = run(limit);
        assert_eq!(stopped.result, Err(Stop::WorkLimit), "limit {limit}");
        assert_eq!(stopped.preferred, None, "limit {limit}");
        assert_eq!(stopped.statistics.work, limit, "limit {limit}");
    }
}

#[test]
fn a_refuted_narrowing_publishes_no_preference() {
    let owner = crate::test_support::theory(3, vec![Node::falsum()], vec![0]);
    let narrower = Narrower::new(&owner);
    let attempt = narrower.narrow_known(
        OriginalSubject::new(&owner, None),
        &mut Region::all_open(owner.atom_count()),
        &mut narrower.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Refuted);
    assert_eq!(attempt.preferred, None);
}

#[test]
fn choice_storage_preserves_the_vector_layout() {
    use std::mem::{align_of, size_of};
    assert_eq!(size_of::<ChoiceHeads>(), size_of::<Vec<u64>>());
    assert_eq!(align_of::<ChoiceHeads>(), align_of::<Vec<u64>>());
    let words = vec![1_u64, 2, 0];
    let pointer = words.as_ptr();
    let capacity = words.capacity();
    let choices = ChoiceHeads {
        words: words.into(),
    };
    assert_eq!(choices.words.as_ptr(), pointer);
    assert_eq!(choices.words.capacity(), capacity);
    assert!(choices.contains(0));
    assert!(choices.contains(65));
    assert!(!choices.contains(64));
    assert!(!choices.contains(192));
    assert!(!choices.contains(usize::MAX));
}

/// Frozen propagation over an authenticated all-true candidate; the outer
/// implication leaves every atom open, so a ranking has exactly three reads.
fn frozen_probe_narrowing(
    choose: bool,
    allowance: u64,
    cancellation: &Cancellation,
) -> (crate::NarrowingAttempt, Region, u64) {
    let owner = theory();
    let narrower = Narrower::new(&owner);
    // The interpretation holding all three atoms satisfies every node.
    let truth = [true; 5];
    let subject = crate::FrozenSubject::new(&owner, &truth);
    let mut region = Region::all_open(owner.atom_count());
    let mut known = narrower.frozen_knowledge();
    let mut quota = Allowance::new(allowance);
    let attempt = {
        let mut meter = zetesis_algebra::Meter::reserved(&mut quota);
        if choose {
            narrower.narrow_frozen_known_reserved(
                subject,
                &mut region,
                &mut known,
                &mut NarrowingScratch::default(),
                cancellation,
                &mut meter,
            )
        } else {
            narrower.propagate_frozen_known_reserved(
                subject,
                &mut region,
                &mut known,
                &mut NarrowingScratch::default(),
                cancellation,
                &mut meter,
            )
        }
    };
    (
        crate::test_support::stop_narrowing(&attempt),
        region,
        quota.granted - quota.refunded,
    )
}

#[test]
fn frozen_probe_propagation_omits_only_unused_ranking() {
    let cancellation = Cancellation::default();
    let (ranked, expected, ranked_work) = frozen_probe_narrowing(true, u64::MAX, &cancellation);
    let (deferred, actual, deferred_work) = frozen_probe_narrowing(false, u64::MAX, &cancellation);
    assert_eq!(deferred.result, ranked.result);
    assert_eq!(deferred.result, Ok(Narrowing::Fixed { changed: false }));
    assert_eq!(actual, expected);
    assert_eq!(ranked.preferred, Some(1));
    assert_eq!(deferred.preferred, None);
    assert_eq!(ranked_work, ranked.statistics.work);
    assert_eq!(deferred_work, deferred.statistics.work);
    assert_eq!(ranked_work - deferred_work, 3);
    let mut expected_statistics = ranked.statistics;
    expected_statistics.work -= 3;
    assert_eq!(deferred.statistics, expected_statistics);
}

#[test]
fn frozen_probe_propagation_preserves_every_refused_prefix() {
    let cancellation = Cancellation::default();
    let (complete, expected, total) = frozen_probe_narrowing(false, u64::MAX, &cancellation);
    assert!(complete.result.is_ok());
    for allowance in 0..total {
        let (ranked, ranked_region, ranked_work) =
            frozen_probe_narrowing(true, allowance, &cancellation);
        let (deferred, actual, work) = frozen_probe_narrowing(false, allowance, &cancellation);
        assert_eq!(deferred.result, Err(Stop::WorkLimit), "{allowance}");
        assert_eq!(deferred.result, ranked.result, "{allowance}");
        assert_eq!(deferred.statistics, ranked.statistics, "{allowance}");
        assert_eq!(actual, ranked_region, "{allowance}");
        assert_eq!(work, ranked_work, "{allowance}");
        assert_eq!(work, allowance, "{allowance}");
        assert_eq!(deferred.preferred, None, "{allowance}");
    }
    let (exact, actual, work) = frozen_probe_narrowing(false, total, &cancellation);
    assert_eq!(exact.result, complete.result);
    assert_eq!(actual, expected);
    assert_eq!(work, total);
    let (ranking_refused, _, work) = frozen_probe_narrowing(true, total, &cancellation);
    assert_eq!(ranking_refused.result, Err(Stop::WorkLimit));
    assert_eq!(work, total);
}

#[test]
fn frozen_probe_propagation_honors_preexisting_cancellation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    for choose in [false, true] {
        let (attempt, region, work) = frozen_probe_narrowing(choose, u64::MAX, &cancellation);
        assert_eq!(attempt.result, Err(Stop::Cancelled));
        assert_eq!(attempt.statistics.work, 0);
        assert_eq!(attempt.preferred, None);
        assert_eq!(region, Region::all_open(3));
        assert_eq!(work, 0);
    }
}

#[test]
fn refuted_frozen_propagation_has_the_unchanged_receipt() {
    let owner = theory();
    let narrower = Narrower::new(&owner);
    let truth = [true; 5];
    let subject = crate::FrozenSubject::new(&owner, &truth);
    let run = |choose, allowance| {
        let mut region = Region::all_open(3);
        assert!(region.cut(0));
        assert!(region.hold(1));
        assert!(region.hold(2));
        let mut known = narrower.frozen_knowledge();
        let mut quota = Allowance::new(allowance);
        let attempt = {
            let mut meter = zetesis_algebra::Meter::reserved(&mut quota);
            if choose {
                narrower.narrow_frozen_known_reserved(
                    subject,
                    &mut region,
                    &mut known,
                    &mut NarrowingScratch::default(),
                    &Cancellation::default(),
                    &mut meter,
                )
            } else {
                narrower.propagate_frozen_known_reserved(
                    subject,
                    &mut region,
                    &mut known,
                    &mut NarrowingScratch::default(),
                    &Cancellation::default(),
                    &mut meter,
                )
            }
        };
        (attempt, region, quota.granted - quota.refunded)
    };
    let (complete, _, total) = run(false, u64::MAX);
    assert_eq!(complete.result, Ok(Narrowing::Refuted));
    for allowance in 0..=total {
        let (ranked, expected, ranked_work) = run(true, allowance);
        let (deferred, actual, work) = run(false, allowance);
        assert_eq!(deferred.result, ranked.result, "{allowance}");
        assert_eq!(deferred.statistics, ranked.statistics, "{allowance}");
        assert_eq!(actual, expected, "{allowance}");
        assert_eq!(work, ranked_work, "{allowance}");
        assert_eq!(work, allowance, "{allowance}");
        assert_eq!(deferred.preferred, None, "{allowance}");
        if allowance == total {
            assert_eq!(deferred.result, Ok(Narrowing::Refuted));
        } else {
            assert_eq!(
                deferred.result,
                Err(zetesis_algebra::Failure::Refused(Stop::WorkLimit))
            );
        }
    }
}

/// `0 ∨ 1 ∨ 2`, and beside it three implications from 3 that no root reads:
/// 3 has three unknown parents and each other atom one. With `busier`, a
/// fourth such implication gives 2 a second.
fn disjunction_beside_a_busier_atom(busier: bool) -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::atom(3),
        Node::or_pair([1, 2]),
        Node::or_pair([0, 4]),
        Node::implies(3, 0),
        Node::implies(3, 1),
        Node::implies(3, 2),
    ];
    if busier {
        nodes.push(Node::implies(2, 3));
    }
    Theory::new(
        4,
        FormulaParts::new(nodes, vec![]).unwrap(),
        vec![5],
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// The closed open region, and then the region with 0 cut, closed on the
/// first's knowledge: the preferred split of each.
fn splits_before_and_after_a_cut(theory: &Theory) -> (Option<usize>, Option<usize>) {
    let narrower = Narrower::new(theory);
    let mut region = Region::all_open(theory.atom_count());
    let mut known = narrower.knowledge();
    let close = |region: &mut Region, known: &mut Knowledge| {
        let attempt = narrower.narrow_known(
            OriginalSubject::new(theory, None),
            region,
            known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        );
        assert_ne!(attempt.result.unwrap(), Narrowing::Refuted);
        attempt.preferred
    };
    let before = close(&mut region, &mut known);
    assert!(region.cut(0));
    (before, close(&mut region, &mut known))
}

#[test]
fn a_shortened_chain_outranks_more_unknown_parents() {
    // Nothing is shortened at first, and 3 has the most unknown parents.
    // Cutting 0 leaves the disjunction two operands, 1 and 2, which then
    // precede 3; they have one unknown parent each, so the lower is taken.
    assert_eq!(
        splits_before_and_after_a_cut(&disjunction_beside_a_busier_atom(false)),
        (Some(3), Some(1))
    );
}

#[test]
fn equal_shortened_scores_fall_to_unknown_parents() {
    assert_eq!(
        splits_before_and_after_a_cut(&disjunction_beside_a_busier_atom(true)),
        (Some(3), Some(2))
    );
}

#[test]
fn a_countermodel_walk_ranks_by_shortened_chains() {
    // The candidate holding every atom satisfies every node, so its reduct
    // is the theory itself.
    let owner = disjunction_beside_a_busier_atom(false);
    let narrower = Narrower::new(&owner);
    let truth = [true; 9];
    let subject = crate::FrozenSubject::new(&owner, &truth);
    let mut region = Region::all_open(4);
    let mut known = narrower.frozen_knowledge();
    let mut close = |region: &mut Region| {
        let attempt = crate::test_support::stop_narrowing(&narrower.narrow_frozen_known_reserved(
            subject,
            region,
            &mut known,
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(u64::MAX)),
        ));
        assert_ne!(attempt.result.unwrap(), Narrowing::Refuted);
        attempt.preferred
    };
    assert_eq!(close(&mut region), Some(3));
    assert!(region.cut(0));
    assert_eq!(close(&mut region), Some(1));
}

#[test]
fn a_frozen_mask_shortens_a_disjunction_before_any_split() {
    // The candidate omits 0, so its reduct reads 0 as falsum: the disjunction
    // has two members at the query's root, and 1 precedes the busier 3.
    let owner = disjunction_beside_a_busier_atom(false);
    let narrower = Narrower::new(&owner);
    // Under {1, 2, 3}: 0 and `3 -> 0` fail, every other node holds.
    let truth = [false, true, true, true, true, true, false, true, true];
    let mut region = Region::all_open(4);
    let mut known = narrower.frozen_knowledge();
    let attempt = crate::test_support::stop_narrowing(&narrower.narrow_frozen_known_reserved(
        crate::FrozenSubject::new(&owner, &truth),
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(u64::MAX)),
    ));
    assert_ne!(attempt.result.unwrap(), Narrowing::Refuted);
    assert!(known.shortened());
    assert_eq!(attempt.preferred, Some(1));
}

#[test]
fn a_settled_chain_leaves_the_count_ranking() {
    // Cutting 0 shortens the disjunction and holding 1 then settles it. The
    // mark stays set while every score is zero, so the unknown-parent counts
    // decide as they did without it: of `3 -> 0`, `3 -> 1` and `3 -> 2` the
    // second is now known, which leaves 3 two unknown parents and 2 one.
    let theory = disjunction_beside_a_busier_atom(false);
    let narrower = Narrower::new(&theory);
    let mut region = Region::all_open(4);
    let mut known = narrower.knowledge();
    let close = |region: &mut Region, known: &mut Knowledge| {
        let attempt = narrower.narrow_known(
            OriginalSubject::new(&theory, None),
            region,
            known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        );
        assert_ne!(attempt.result.unwrap(), Narrowing::Refuted);
        attempt.preferred
    };
    close(&mut region, &mut known);
    assert!(region.cut(0));
    assert_eq!(close(&mut region, &mut known), Some(1));
    assert!(region.hold(1));
    let preferred = close(&mut region, &mut known);
    assert!(known.shortened());
    for atom in [2, 3] {
        let score = narrower
            .shortened_weight(&known, atom, &mut zetesis_algebra::Meter::local(u64::MAX))
            .map_err(zetesis_algebra::Failure::into_stop)
            .unwrap();
        assert_eq!(score, 0, "atom {atom}");
    }
    assert_eq!(preferred, Some(3));
}

#[test]
fn a_frozen_region_without_the_mark_scores_nothing() {
    // Every model of the fixture as the frozen candidate, and every region
    // of its countermodel walk reached by at most two decisions over the
    // candidate's atoms. The frozen mask can shorten a chain at the root, so
    // both marked and unmarked regions occur.
    let owner = disjunction_beside_a_busier_atom(false);
    let narrower = Narrower::new(&owner);
    let (mut unmarked, mut marked) = (0, 0);
    for candidate in 0..16_usize {
        let holds = |atom: usize| candidate & (1 << atom) != 0;
        // Each node's truth under the candidate, in the fixture's order.
        let truth = [
            holds(0),
            holds(1),
            holds(2),
            holds(3),
            holds(1) || holds(2),
            holds(0) || holds(1) || holds(2),
            !holds(3) || holds(0),
            !holds(3) || holds(1),
            !holds(3) || holds(2),
        ];
        if !truth[5] {
            continue;
        }
        let atoms: Vec<usize> = (0..4).filter(|&atom| holds(atom)).collect();
        let steps: Vec<(usize, bool)> = atoms
            .iter()
            .flat_map(|&atom| [(atom, false), (atom, true)])
            .collect();
        let mut paths: Vec<Vec<(usize, bool)>> = vec![Vec::new()];
        paths.extend(steps.iter().map(|&step| vec![step]));
        for &first in &steps {
            paths.extend(steps.iter().map(|&second| vec![first, second]));
        }
        for path in paths {
            let mut region = Region::all_open(4);
            let mut known = narrower.frozen_knowledge();
            let mut close = |region: &mut Region| {
                let attempt =
                    crate::test_support::stop_narrowing(&narrower.narrow_frozen_known_reserved(
                        crate::FrozenSubject::new(&owner, &truth),
                        region,
                        &mut known,
                        &mut NarrowingScratch::default(),
                        &Cancellation::default(),
                        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(u64::MAX)),
                    ));
                attempt.result.unwrap() != Narrowing::Refuted
            };
            let mut reached = close(&mut region);
            for &(atom, value) in &path {
                if !reached || !region.is_open(atom) {
                    reached = false;
                    break;
                }
                assert!(if value {
                    region.hold(atom)
                } else {
                    region.cut(atom)
                });
                reached = close(&mut region);
            }
            if !reached {
                continue;
            }
            if known.shortened() {
                marked += 1;
                continue;
            }
            unmarked += 1;
            for &atom in atoms.iter().filter(|&&atom| region.is_open(atom)) {
                let score = narrower
                    .shortened_weight(&known, atom, &mut zetesis_algebra::Meter::local(u64::MAX))
                    .map_err(zetesis_algebra::Failure::into_stop)
                    .unwrap();
                assert_eq!(
                    score, 0,
                    "candidate {candidate:04b}, path {path:?}, atom {atom}"
                );
            }
        }
    }
    assert!(
        unmarked > 0 && marked > 0,
        "{unmarked} unmarked, {marked} marked"
    );
}

/// The closed region of `disjunction_beside_a_busier_atom(false)` with 0 cut.
fn shortened_disjunction() -> (Narrower, Region, Knowledge) {
    let theory = disjunction_beside_a_busier_atom(false);
    let (narrower, mut region, mut known) = closed(&theory);
    assert!(region.cut(0));
    narrower
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut region,
            &mut known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    (narrower, region, known)
}

/// The reads of ranking `shortened_disjunction`: the open atoms 1, 2 and 3
/// once each; the disjunction and one implication above each of 1 and 2; and
/// the three implications above 3, which no node reads in turn.
const SHORTENED_RANKING_WORK: u64 = 3 + 2 + 2 + 3;

#[test]
fn a_shortened_ranking_charges_each_parent_read() {
    let (narrower, region, known) = shortened_disjunction();
    let scores: Vec<u64> = [1, 2, 3]
        .into_iter()
        .map(|atom| {
            narrower
                .shortened_weight(&known, atom, &mut crate::regions::Meter::local(u64::MAX))
                .map_err(zetesis_algebra::Failure::into_stop)
                .unwrap()
        })
        .collect();
    assert_eq!(
        scores,
        [super::shortening(3, 2), super::shortening(3, 2), 0]
    );
    let selected = crate::test_support::stop_preference(&known.preferred_atom_reserved(
        &narrower,
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut zetesis_algebra::Meter::reserved(&mut Allowance::new(1000)),
    ));
    assert_eq!(selected.result, Ok(Some(1)));
    assert_eq!(selected.work, SHORTENED_RANKING_WORK);
}

#[test]
fn a_refused_shortened_ranking_retains_its_exact_read_prefix() {
    let (narrower, region, known) = shortened_disjunction();
    for limit in 0..=SHORTENED_RANKING_WORK {
        let mut allowance = Allowance::new(limit);
        let selected = crate::test_support::stop_preference(&known.preferred_atom_reserved(
            &narrower,
            &region,
            &ChoiceHeads::default(),
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut allowance),
        ));
        assert_eq!(selected.work, limit, "limit {limit}");
        assert_eq!(allowance.granted - allowance.refunded, limit);
        assert_eq!(
            selected.result,
            if limit == SHORTENED_RANKING_WORK {
                Ok(Some(1))
            } else {
                Err(Stop::WorkLimit)
            },
            "limit {limit}"
        );
    }
}
