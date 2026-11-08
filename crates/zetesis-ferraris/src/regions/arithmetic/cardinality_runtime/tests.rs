use zetesis_algebra::Stop;

use super::*;
use crate::regions::arithmetic::{
    budget::Budget,
    cardinality,
    cardinality_source::{Bound, Kind, Source},
    source::Literal,
    storage::Storage,
    uniform::BoundSide,
};
use crate::test_support::theory;

fn bound(atoms: usize, support: &[usize], side: BoundSide, value: u64, guard: &[Literal]) -> Bound {
    let mut words = Words::<u64>::new(vec![0; atoms.div_ceil(128) * 2]);
    for &atom in support {
        words.insert(atom);
    }
    Bound {
        support: words,
        guard: guard.to_vec(),
        side,
        value,
        cardinality: u64::try_from(support.len()).unwrap(),
        source: Source {
            kind: Kind::Asserted,
            root: 0,
            node: 0,
        },
    }
}

fn system(atoms: usize, bounds: Vec<Bound>) -> CardinalitySystem {
    let cancel = Cancellation::default();
    let mut meter = Meter::local(u64::MAX);
    let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancel);
    let mut storage = Storage::default();
    let prepared = cardinality::prepare(&bounds, atoms, &mut storage, &mut budget).unwrap();
    CardinalitySystem {
        owner: theory(atoms, vec![], vec![]),
        bounds,
        prepared,
    }
}

fn guarded() -> CardinalitySystem {
    system(
        3,
        vec![
            bound(3, &[0, 1], BoundSide::Lower, 1, &[]),
            bound(
                3,
                &[0, 1],
                BoundSide::Upper,
                1,
                &[Literal {
                    atom: 2,
                    positive: true,
                }],
            ),
        ],
    )
}

fn families(system: &CardinalitySystem) -> &Families {
    let Prepared::Families(families) = &system.prepared else {
        panic!("fixture retains its families")
    };
    families
}

fn run(
    system: &CardinalitySystem,
    region: &mut Region,
    known: &mut CardinalityKnowledge,
) -> ArithmeticAttempt<Failure<Stop>> {
    system.inspect_reserved(
        region,
        known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    )
}

fn region(atoms: usize, held: usize, cut: usize) -> Region {
    assert_eq!(held & cut, 0);
    let mut region = Region::all_open(atoms);
    for atom in 0..atoms {
        if held & (1 << atom) != 0 {
            assert!(region.hold(atom));
        }
        if cut & (1 << atom) != 0 {
            assert!(region.cut(atom));
        }
    }
    region
}

fn value(bound: &Bound, region: &Region) -> u64 {
    let active = bound
        .guard
        .iter()
        .all(|literal| region.decision(literal.atom) == Some(literal.positive));
    let count: u64 = (0..region.len())
        .filter(|&atom| {
            bound.support.contains(atom)
                && match bound.side {
                    BoundSide::Lower => region.decision(atom) == Some(true),
                    BoundSide::Upper => region.decision(atom) != Some(false),
                }
        })
        .count()
        .try_into()
        .unwrap();
    match (bound.side, active) {
        (BoundSide::Lower, true) => count.max(bound.value),
        (BoundSide::Upper, true) => count.min(bound.value),
        (_, false) => count,
    }
}

fn fresh(system: &CardinalitySystem, region: &Region) -> Vec<PairSums> {
    let families = families(system);
    families
        .pairs
        .iter()
        .map(|pair| PairSums {
            lower: pair
                .lower
                .clone()
                .map(|position| value(&system.bounds[families.occurrences[position].row], region))
                .sum(),
            upper: pair
                .upper
                .clone()
                .map(|position| value(&system.bounds[families.occurrences[position].row], region))
                .sum(),
        })
        .collect()
}

#[test]
fn every_descendant_matches_fresh_family_sums() {
    let system = guarded();
    for old_held in 0..8 {
        for old_cut in 0..8 {
            if old_held & old_cut != 0 {
                continue;
            }
            let mut prior = region(3, old_held, old_cut);
            let mut parent = system.knowledge();
            if run(&system, &mut prior, &mut parent).result.unwrap() == SubjectStep::Refuted {
                continue;
            }
            assert_eq!(parent.sums, fresh(&system, &prior));
            for held in 0..8 {
                for cut in 0..8 {
                    if held & cut != 0 || old_held & !held != 0 || old_cut & !cut != 0 {
                        continue;
                    }
                    let mut current = region(3, held, cut);
                    let before = current.clone();
                    let mut child = parent.clone();
                    let expected = fresh(&system, &current);
                    let refuted = expected.iter().any(|pair| pair.lower > pair.upper);
                    let attempt = run(&system, &mut current, &mut child);
                    assert_eq!(
                        attempt.result.unwrap(),
                        if refuted {
                            SubjectStep::Refuted
                        } else {
                            SubjectStep::Unchanged
                        }
                    );
                    assert_eq!(current, before);
                    assert_eq!(
                        (
                            attempt.statistics.held,
                            attempt.statistics.cut,
                            attempt.statistics.propagations
                        ),
                        (0, 0, 0)
                    );
                    if refuted {
                        assert!(
                            [1usize, 2, 3, 5, 6]
                                .into_iter()
                                .all(|model| model & held != held || model & cut != 0)
                        );
                    } else {
                        assert_eq!(child.sums, expected);
                        assert_eq!(child.snapshot.old_masks(), current.decision_words());
                    }
                    assert_eq!(parent.sums, fresh(&system, &prior));
                    assert_eq!(parent.snapshot.old_masks(), prior.decision_words());
                }
            }
        }
    }
}

#[test]
fn a_guard_only_decision_updates_each_pair_occurrence() {
    let system = guarded();
    let stored: Vec<_> = families(&system)
        .occurrences
        .iter()
        .map(|occurrence| occurrence.value)
        .collect();
    let mut input = Region::all_open(3);
    let mut known = system.knowledge();
    assert_eq!(
        run(&system, &mut input, &mut known).result.unwrap(),
        SubjectStep::Unchanged
    );
    assert_eq!(known.sums, vec![PairSums { lower: 1, upper: 2 }; 4]);
    input.hold(2);
    assert_eq!(
        run(&system, &mut input, &mut known).result.unwrap(),
        SubjectStep::Unchanged
    );
    assert_eq!(known.sums, vec![PairSums { lower: 1, upper: 1 }; 4]);
    assert_eq!(
        families(&system)
            .occurrences
            .iter()
            .map(|occurrence| occurrence.value)
            .collect::<Vec<_>>(),
        stored
    );
}

#[test]
fn multiple_new_atoms_do_not_duplicate_an_occurrence() {
    let system = system(
        3,
        vec![
            bound(3, &[0, 1], BoundSide::Lower, 1, &[]),
            bound(3, &[0, 1], BoundSide::Upper, 2, &[]),
        ],
    );
    let mut input = Region::all_open(3);
    let mut known = system.knowledge();
    assert_eq!(
        run(&system, &mut input, &mut known).result.unwrap(),
        SubjectStep::Unchanged
    );
    input.hold(0);
    input.hold(1);
    assert_eq!(
        run(&system, &mut input, &mut known).result.unwrap(),
        SubjectStep::Unchanged
    );
    assert_eq!(known.sums, fresh(&system, &input));
    assert!(
        known
            .sums
            .iter()
            .all(|sum| *sum == PairSums { lower: 2, upper: 2 })
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Ticket(u64),
    Control(Stop),
}

impl zetesis_algebra::Refusal for Refusal {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Ticket(_) => true,
            Self::Control(stop) => zetesis_algebra::Refusal::is_resource_refusal(stop),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Control(stop)
    }
}

struct Account {
    remaining: u64,
    calls: usize,
    refunds: usize,
}

impl Quota for Account {
    type Error = Refusal;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        self.calls += 1;
        if self.remaining == 0 {
            return Err(Refusal::Ticket(41));
        }
        let granted = wanted.min(self.remaining);
        self.remaining -= granted;
        Ok(granted)
    }
    fn refund(&mut self, amount: u64) {
        self.refunds += 1;
        self.remaining += amount;
    }
}

#[test]
fn every_refused_prefix_preserves_the_caller_error() {
    let system = guarded();
    let mut root = region(3, 4, 0);
    let total = run(&system, &mut root, &mut system.knowledge())
        .statistics
        .work;
    for allowance in 0..=total {
        let mut input = region(3, 4, 0);
        let before = input.clone();
        let mut known = system.knowledge();
        let mut account = Account {
            remaining: allowance,
            calls: 0,
            refunds: 0,
        };
        let mut meter = Meter::reserved(&mut account);
        let attempt = system.inspect_reserved(
            &mut input,
            &mut known,
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut meter,
        );
        assert_eq!(attempt.statistics.work, meter.charged());
        assert_eq!(account.remaining + attempt.statistics.work, allowance);
        assert_eq!(account.refunds, 1);
        assert_eq!(input, before);
        if allowance < total {
            assert_eq!(attempt.result, Err(Failure::Refused(Refusal::Ticket(41))));
            assert!(!known.snapshot.initialized());
        } else {
            assert_eq!(attempt.result, Ok(SubjectStep::Unchanged));
            assert_eq!(known.sums, fresh(&system, &input));
        }
    }
}

#[test]
fn entry_cancellation_precedes_any_quota_request() {
    let system = guarded();
    let cancel = Cancellation::default();
    cancel.cancel();
    let mut account = Account {
        remaining: u64::MAX,
        calls: 0,
        refunds: 0,
    };
    let mut meter = Meter::reserved(&mut account);
    let mut known = system.knowledge();
    let mut input = Region::all_open(3);
    let attempt = system.inspect_reserved(
        &mut input,
        &mut known,
        &mut NarrowingScratch::default(),
        &cancel,
        &mut meter,
    );
    assert_eq!(attempt.result, Err(Failure::Stopped(Stop::Cancelled)));
    assert_eq!(attempt.statistics.work, 0);
    assert_eq!(account.calls, 0);
    assert!(!known.snapshot.initialized());
    assert!(known.sums.is_empty());
}

#[test]
fn detected_lineage_misuse_is_not_a_semantic_verdict() {
    let system = guarded();
    let mut prior = region(3, 1, 0);
    let mut known = system.knowledge();
    assert_eq!(
        run(&system, &mut prior, &mut known).result,
        Ok(SubjectStep::Unchanged)
    );
    for mut input in [Region::all_open(3), region(3, 0, 1), Region::all_open(4)] {
        let mut attempted = known.clone();
        assert_eq!(
            run(&system, &mut input, &mut attempted).result,
            Err(Failure::Stopped(Stop::InvalidProgram))
        );
        assert_eq!(attempted.snapshot.old_masks(), known.snapshot.old_masks());
        assert_eq!(attempted.sums, known.sums);
    }
}

#[test]
fn retained_bytes_count_the_actual_owned_capacities() {
    let system = guarded();
    let mut known = system.knowledge();
    assert_eq!(
        known.retained_bytes(),
        size_of::<CardinalityKnowledge>() as u128
    );
    run(&system, &mut Region::all_open(3), &mut known)
        .result
        .unwrap();
    assert_eq!(size_of::<PairSums>(), 2 * size_of::<u64>());
    assert_eq!(
        known.retained_bytes(),
        size_of::<CardinalityKnowledge>() as u128
            + known.snapshot.allocated_bytes()
            + known.sums.capacity() as u128 * size_of::<PairSums>() as u128
    );
    let copied = known.clone();
    assert_eq!(copied.sums, known.sums);
    assert_ne!(copied.sums.as_ptr(), known.sums.as_ptr());
    assert_ne!(
        copied.snapshot.old_masks().0.as_ptr(),
        known.snapshot.old_masks().0.as_ptr()
    );
}

#[test]
fn no_families_still_publishes_a_complete_snapshot() {
    let system = system(3, vec![]);
    let mut input = region(3, 1, 2);
    let mut known = system.knowledge();
    assert_eq!(
        run(&system, &mut input, &mut known).result,
        Ok(SubjectStep::Unchanged)
    );
    assert!(known.sums.is_empty());
    assert_eq!(known.snapshot.old_masks(), input.decision_words());
}

mod incidence;
mod copies;

#[test]
fn prepared_refutation_does_not_initialize_lineage_state() {
    let system = system(
        2,
        vec![
            bound(2, &[0, 1], BoundSide::Lower, 2, &[]),
            bound(2, &[0, 1], BoundSide::Upper, 1, &[]),
        ],
    );
    assert!(system.root_refuted());
    let mut known = system.knowledge();
    let before = known.retained_bytes();
    assert_eq!(
        run(&system, &mut Region::all_open(2), &mut known).result,
        Ok(SubjectStep::Refuted)
    );
    assert_eq!(known.retained_bytes(), before);
    assert!(!known.snapshot.initialized());
    assert!(known.sums.is_empty());
}
