//! A secondary subject breaks degree ties without changing split coverage.

use std::collections::BTreeSet;

use crate::{
    AdmissionLimits, ChoiceHeads, FormulaParts, Knowledge, Narrower, Narrowing, NarrowingQuota,
    NarrowingScratch, Node, OriginalSubject, Region, RegionLimits, Theory,
};
use zetesis_cpu::{
    Cancellation, Stop,
    regions::{Counting, Traversal, Visit},
};

#[derive(Default)]
struct Allowance {
    remaining: u64,
    spent: u64,
}

impl Allowance {
    fn new(remaining: u64) -> Self {
        Self {
            remaining,
            spent: 0,
        }
    }
}

impl NarrowingQuota for Allowance {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let grant = self.remaining.min(wanted);
        if grant == 0 {
            return Err(Stop::WorkLimit);
        }
        self.remaining -= grant;
        self.spent += grant;
        Ok(grant)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
        self.spent -= unspent;
    }
}

fn theory(atoms: usize, tail: Vec<Node>, roots: Vec<usize>) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    nodes.extend(tail);
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn close(theory: &Theory, index: &Narrower, region: &mut Region, known: &mut Knowledge) {
    let attempt = index.propagate_known_reserved(
        OriginalSubject::new(theory, None),
        region,
        known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_ne!(attempt.result.unwrap(), Narrowing::Refuted);
}

fn closed(theory: &Theory) -> (Narrower, Region, Knowledge) {
    let index = Narrower::new(theory);
    let mut known = index.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    close(theory, &index, &mut region, &mut known);
    (index, region, known)
}

#[test]
fn secondary_degree_only_breaks_primary_ties() {
    let primary = theory(3, vec![], vec![]);
    let secondary = theory(3, vec![Node::implies(1, 2)], vec![]);
    let (_, region, primary) = closed(&primary);
    let (_, _, secondary) = closed(&secondary);
    let attempt = primary.preferred_atom_with_secondary_reserved(
        &region,
        &ChoiceHeads::default(),
        &secondary,
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(Some(1)));
    assert_eq!(attempt.work, 6);
}

#[test]
fn secondary_degree_cannot_overrule_primary_degree() {
    let primary = theory(3, vec![Node::implies(0, 2)], vec![]);
    let secondary = theory(3, vec![Node::implies(1, 1)], vec![]);
    let (_, region, primary) = closed(&primary);
    let (_, _, secondary) = closed(&secondary);
    let attempt = primary.preferred_atom_with_secondary_reserved(
        &region,
        &ChoiceHeads::default(),
        &secondary,
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(Some(0)));
}

#[test]
fn secondary_degree_preserves_exact_quota_prefixes() {
    let source = theory(3, vec![Node::implies(1, 2)], vec![]);
    let (_, region, known) = closed(&source);
    for limit in 0..=6 {
        let mut quota = Allowance::new(limit);
        let attempt = known.preferred_atom_with_secondary_reserved(
            &region,
            &ChoiceHeads::default(),
            &known,
            &Cancellation::default(),
            &mut quota,
        );
        assert_eq!((attempt.work, quota.spent), (limit, limit));
        assert_eq!(
            attempt.result,
            if limit == 6 {
                Ok(Some(1))
            } else {
                Err(Stop::WorkLimit)
            }
        );
    }
}

#[test]
fn secondary_degree_accepts_different_counter_widths() {
    let source = theory(3, vec![Node::implies(1, 2)], vec![]);
    let (index, region, compact) = closed(&source);
    let mut native = Knowledge {
        width: super::super::Width::Native(index.root_known::<usize>()),
    };
    close(&source, &index, &mut region.clone(), &mut native);
    let attempt = compact.preferred_atom_with_secondary_reserved(
        &region,
        &ChoiceHeads::default(),
        &native,
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(Some(1)));
}

#[test]
fn cancelled_secondary_preference_never_reserves_work() {
    let source = theory(3, vec![], vec![]);
    let (_, region, known) = closed(&source);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut quota = Allowance::new(u64::MAX);
    let attempt = known.preferred_atom_with_secondary_reserved(
        &region,
        &ChoiceHeads::default(),
        &known,
        &cancellation,
        &mut quota,
    );
    assert_eq!(attempt.result, Err(Stop::Cancelled));
    assert_eq!((attempt.work, quota.spent), (0, 0));
}

#[test]
fn choice_priority_precedes_both_degrees() {
    let primary = theory(3, vec![Node::implies(0, 1)], vec![]);
    let secondary = theory(3, vec![Node::implies(1, 1)], vec![]);
    let original = theory(
        3,
        vec![Node::falsum(), Node::implies(2, 3), Node::or_pair([2, 4])],
        vec![5],
    );
    let extraction =
        crate::producers(&original, RegionLimits::default(), &Cancellation::default()).unwrap();
    let (_, region, primary) = closed(&primary);
    let (_, _, secondary) = closed(&secondary);
    let attempt = primary.preferred_atom_with_secondary_reserved(
        &region,
        &extraction.choice_heads,
        &secondary,
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(Some(2)));
}

#[test]
fn equal_secondary_degrees_prefer_the_lowest_open_atom() {
    let source = theory(3, vec![], vec![]);
    let (index, mut region, mut known) = closed(&source);
    assert!(region.cut(0));
    close(&source, &index, &mut region, &mut known);
    let attempt = known.preferred_atom_with_secondary_reserved(
        &region,
        &ChoiceHeads::default(),
        &known,
        &Cancellation::default(),
        &mut Allowance::new(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(Some(1)));
    assert_eq!(attempt.work, 4);
}

#[test]
fn secondary_preference_keeps_every_satisfying_assignment_once() {
    let theory = theory(
        4,
        vec![Node::or_pair([1, 2]), Node::implies(0, 3)],
        vec![4, 5],
    );
    let index = Narrower::new(&theory);
    let mut walk = Traversal::with_state(Region::all_open(4), Counting::Never, index.knowledge());
    let mut actual = BTreeSet::new();
    let mut scratch = NarrowingScratch::default();
    while let Some(visit) = walk
        .next(|region, known| {
            let attempt = index.propagate_known_reserved(
                OriginalSubject::new(&theory, None),
                region,
                known,
                &mut scratch,
                &Cancellation::default(),
                &mut Allowance::new(u64::MAX),
            );
            let result = attempt.result?;
            if result != Narrowing::Refuted {
                let selected = known.preferred_atom_with_secondary_reserved(
                    region,
                    &ChoiceHeads::default(),
                    known,
                    &Cancellation::default(),
                    &mut Allowance::new(u64::MAX),
                );
                if let Some(atom) = selected.result? {
                    region.prefer(atom);
                }
            }
            Ok::<_, Stop>(result)
        })
        .unwrap()
    {
        let Visit::Leaf(region, _) = visit else {
            panic!("complete assignments are enumerated without subtree counting");
        };
        let mask = region.held().fold(0_u32, |mask, atom| mask | (1 << atom));
        assert!(actual.insert(mask), "duplicate assignment {mask}");
    }
    let expected = (0_u32..16)
        .filter(|mask| mask & 6 != 0 && (mask & 1 == 0 || mask & 8 != 0))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}
