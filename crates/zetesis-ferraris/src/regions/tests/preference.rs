//! Propagation can be composed without spending or replacing split preferences.

use crate::{
    AdmissionLimits, ChoiceHeads, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingQuota,
    NarrowingScratch, Node, OriginalSubject, Region, RegionLimits, Theory,
};
use zetesis_cpu::{Cancellation, Stop};

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

impl NarrowingQuota for Allowance {
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

fn closed(theory: &Theory) -> (Region, Knowledge) {
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
        .unwrap();
    (region, known)
}

#[test]
fn deferred_preference_preserves_existing_split_choice() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let mut known = narrower.knowledge();
    let mut region = Region::all_open(3);
    region.prefer(2);
    let attempt = narrower.propagate_known_reserved(
        OriginalSubject::new(&theory, None),
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(2));
}

#[test]
fn separate_preference_has_the_original_narrowing_receipt() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let mut ordinary = Region::all_open(3);
    let (_, expected) = narrower
        .narrow_known(
            OriginalSubject::new(&theory, None),
            &mut ordinary,
            &mut narrower.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let mut region = Region::all_open(3);
    let mut known = narrower.knowledge();
    let mut allowance = Allowance::new(1000);
    let propagated = narrower.propagate_known_reserved(
        OriginalSubject::new(&theory, None),
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut allowance,
    );
    propagated.result.unwrap();
    let preference = known.preferred_atom_reserved(
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut allowance,
    );
    region.prefer(preference.result.unwrap().unwrap());
    assert_eq!(region, ordinary);
    assert_eq!(preference.work, 3);
    assert_eq!(propagated.statistics.work + preference.work, expected.work);
    assert_eq!(allowance.granted - allowance.refunded, expected.work);
}

#[test]
fn preference_refusal_retains_exact_read_prefix() {
    let theory = theory();
    let (region, known) = closed(&theory);
    for limit in 0..=3 {
        let mut allowance = Allowance::new(limit);
        let selected = known.preferred_atom_reserved(
            &region,
            &ChoiceHeads::default(),
            &Cancellation::default(),
            &mut allowance,
        );
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
    let (region, known) = closed(&theory);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut allowance = Allowance::new(1000);
    let selected = known.preferred_atom_reserved(
        &region,
        &ChoiceHeads::default(),
        &cancellation,
        &mut allowance,
    );
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
    let (mut region, known) = closed(&theory);
    for atom in 0..130 {
        if atom != 64 && atom != 129 {
            assert!(region.cut(atom));
        }
    }
    let selected = known.preferred_atom_reserved(
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
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
        .unwrap();
    assert_eq!(region.open().collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
    let ordinary = known.preferred_atom_reserved(
        &region,
        &ChoiceHeads::default(),
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
    let busiest = ordinary.result.unwrap().unwrap();
    // The premise: the ordinary ranking prefers an atom with more unknown
    // parents than the choice head.
    assert!(unknown(&known, busiest) > unknown(&known, 0));
    let choice = known.preferred_atom_reserved(
        &region,
        &extraction.choice_heads,
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
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
        let mut region = Region::all_open(theory.atom_count());
        narrower
            .narrow_known(
                subject,
                &mut region,
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        region.split_atom()
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
    let (mut region, known) = closed(&theory);
    assert!(region.hold(0));
    assert!(region.cut(64));
    assert!(region.cut(129));
    let choices = ChoiceHeads {
        words: vec![1, 1, !3, u64::MAX],
    };
    let attempt = known.preferred_atom_reserved(
        &region,
        &choices,
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
    assert_eq!(attempt.result, Ok(Some(1)));
    assert_eq!(attempt.work, 127);
    let choices = ChoiceHeads {
        words: vec![1, 1 | (1 << 63), !1, u64::MAX],
    };
    let attempt = known.preferred_atom_reserved(
        &region,
        &choices,
        &Cancellation::default(),
        &mut Allowance::new(1000),
    );
    assert_eq!(attempt.result, Ok(Some(127)));
    assert_eq!(attempt.work, 1);
}
