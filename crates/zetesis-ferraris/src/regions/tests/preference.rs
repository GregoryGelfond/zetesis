//! Propagation can be composed without spending or replacing split preferences.

use crate::{
    AdmissionLimits, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingQuota,
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
    let preference =
        known.preferred_atom_reserved(&region, &Cancellation::default(), &mut allowance);
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
        let selected =
            known.preferred_atom_reserved(&region, &Cancellation::default(), &mut allowance);
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
    let selected = known.preferred_atom_reserved(&region, &cancellation, &mut allowance);
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
    let selected =
        known.preferred_atom_reserved(&region, &Cancellation::default(), &mut Allowance::new(1000));
    assert_eq!(selected.result, Ok(Some(64)));
    assert_eq!(selected.work, 2);
}
