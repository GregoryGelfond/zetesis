use zetesis_algebra::{
    Cancellation, Failure, Meter, Stop,
    region::{Region, SubjectStep},
};

use super::super::super::{
    ParitySystem,
    budget::Budget,
    parity, root_facts,
    rows::{Counts, Equation, Recognized, Unit},
    storage::Storage,
};
use crate::{regions::NarrowingScratch, test_support::constructed_theory};

/// Runtime tests start with explicit mathematical equations and the real
/// echelon/index preparation. Source-recognition soundness has separate tests;
/// this fixture makes no claim that empty source roots entail these equations.
fn system(atoms: usize, specifications: &[(Vec<usize>, bool)]) -> ParitySystem {
    let theory = constructed_theory(atoms, |_| Vec::new());
    let cancellation = Cancellation::default();
    let mut work = Meter::local(u64::MAX);
    let mut budget = Budget::new(&mut work, u64::MAX, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let facts = root_facts::build(&theory, &mut storage, &mut budget).unwrap();
    let mut equations: Vec<_> = specifications
        .iter()
        .map(|(support, constant)| {
            let mut words = vec![0; atoms.div_ceil(128) * 2];
            for &atom in support {
                words[atom / 64] |= 1 << (atom % 64);
            }
            Equation {
                words: words.into(),
                constant: *constant,
            }
        })
        .collect();
    let prepared = parity::prepare(&mut equations, atoms, &mut storage, &mut budget).unwrap();
    ParitySystem {
        recognized: Recognized {
            facts,
            clauses: Vec::new(),
            equations,
            counts: Counts::default(),
        },
        prepared,
    }
}

/// As [`system`], after the direct units `units`, each `(atom, value)`.
fn system_after(
    atoms: usize,
    units: &[(usize, bool)],
    specifications: &[(Vec<usize>, bool)],
) -> ParitySystem {
    let theory = constructed_theory(atoms, |_| Vec::new());
    let cancellation = Cancellation::default();
    let mut work = Meter::local(u64::MAX);
    let mut budget = Budget::new(&mut work, u64::MAX, usize::MAX, &cancellation);
    let mut storage = Storage::default();
    let facts = root_facts::build(&theory, &mut storage, &mut budget).unwrap();
    let mut direct = storage.reserve(0, &mut budget).unwrap();
    for &(atom, value) in units {
        storage
            .push(&mut direct, Unit { atom, value }, 2, &mut budget)
            .unwrap();
    }
    let mut equations: Vec<_> = specifications
        .iter()
        .map(|(support, constant)| {
            let mut words = vec![0; atoms.div_ceil(128) * 2];
            for &atom in support {
                words[atom / 64] |= 1 << (atom % 64);
            }
            Equation {
                words: words.into(),
                constant: *constant,
            }
        })
        .collect();
    let prepared =
        parity::prepare_after_units(direct, &mut equations, atoms, &mut storage, &mut budget)
            .unwrap();
    ParitySystem {
        recognized: Recognized {
            facts,
            clauses: Vec::new(),
            equations,
            counts: Counts::default(),
        },
        prepared,
    }
}

/// Whether `system` is consistent in `region`, under an unbounded meter.
fn consistent(system: &ParitySystem, region: &Region) -> bool {
    system
        .consistent_in_reserved(region, &mut Meter::local(u64::MAX))
        .unwrap()
}

#[test]
fn consistency_reads_every_direct_unit_against_the_region() {
    // Units `a0 = 1` and `a65 = 0`, and the row `a1 + a2 = 1`, in every
    // region that decides at most atoms 0, 1, 2 and 65.
    let units = [(0, true), (65, false)];
    let rows = [(vec![1, 2], true)];
    let system = system_after(66, &units, &rows);
    let decided = [0, 1, 2, 65];
    for decisions in 0..3usize.pow(4) {
        let mut region = Region::all_open(66);
        let mut values = [None; 4];
        let mut rest = decisions;
        for (position, &atom) in decided.iter().enumerate() {
            match rest % 3 {
                1 => {
                    assert!(region.hold(atom));
                    values[position] = Some(true);
                }
                2 => {
                    assert!(region.cut(atom));
                    values[position] = Some(false);
                }
                _ => {}
            }
            rest /= 3;
        }
        let expected = values[0] != Some(false)
            && values[3] != Some(true)
            && !(values[1].is_some() && values[1] == values[2]);
        assert_eq!(
            consistent(&system, &region),
            expected,
            "decisions {values:?}"
        );
    }
}

#[test]
fn consistency_leaves_atoms_beyond_a_narrower_region_open() {
    // The system is over 66 atoms and its unit over atom 65; a region of ten
    // atoms has one mask word and decides nothing about that atom.
    let system = system_after(66, &[(65, false)], &[(vec![1, 2], true)]);
    let mut region = Region::all_open(10);
    assert!(consistent(&system, &region));
    assert!(region.hold(1) && region.hold(2));
    assert!(!consistent(&system, &region), "the row is still read");
}

#[test]
fn an_inspection_reads_no_direct_unit() {
    // The seeded root holds the direct units; a region without them is
    // outside this system's use, and an inspection decides nothing in it.
    let system = system_after(2, &[(0, true), (1, false)], &[]);
    let mut region = Region::all_open(2);
    let mut known = system.knowledge();
    let attempt = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(SubjectStep::Unchanged));
    assert!(region.is_open(0) && region.is_open(1));
}

fn chain(atoms: usize) -> ParitySystem {
    system(
        atoms,
        &(0..atoms.saturating_sub(1))
            .map(|atom| (vec![atom, atom + 1], false))
            .collect::<Vec<_>>(),
    )
}

fn inside(region: &Region, assignment: usize) -> bool {
    (0..region.len()).all(|atom| {
        region
            .decision(atom)
            .is_none_or(|value| value == (assignment & (1 << atom) != 0))
    })
}

fn satisfies(assignment: usize, specifications: &[(Vec<usize>, bool)]) -> bool {
    specifications.iter().all(|(support, constant)| {
        support
            .iter()
            .filter(|&&atom| assignment & (1 << atom) != 0)
            .count()
            % 2
            == usize::from(*constant)
    })
}

#[test]
fn every_small_region_keeps_all_equation_models() {
    for atoms in 0..=3usize {
        let possibilities = 1 << (atoms + 1);
        for first in 0..possibilities {
            for second in 0..possibilities {
                let specifications: Vec<_> = [first, second]
                    .into_iter()
                    .map(|row| {
                        (
                            (0..atoms)
                                .filter(|atom| (row / 2) & (1 << atom) != 0)
                                .collect(),
                            row % 2 != 0,
                        )
                    })
                    .collect();
                let system = system(atoms, &specifications);
                for encoded in 0..3usize.pow(u32::try_from(atoms).unwrap()) {
                    let mut region = Region::all_open(atoms);
                    let mut choices = encoded;
                    for atom in 0..atoms {
                        match choices % 3 {
                            1 => assert!(region.hold(atom)),
                            2 => assert!(region.cut(atom)),
                            _ => (),
                        }
                        choices /= 3;
                    }
                    let expected: Vec<_> = (0..1usize << atoms)
                        .filter(|&assignment| {
                            inside(&region, assignment) && satisfies(assignment, &specifications)
                        })
                        .collect();
                    let before = region.clone();
                    let mut work = Meter::local(u64::MAX);
                    let result = system.inspect_reserved(
                        &mut region,
                        &mut system.knowledge(),
                        &mut NarrowingScratch::default(),
                        &Cancellation::default(),
                        &mut work,
                    );
                    assert_eq!(result.statistics.work, work.charged());
                    match result.result.unwrap() {
                        SubjectStep::Refuted => assert!(expected.is_empty()),
                        step => {
                            assert!(
                                expected
                                    .iter()
                                    .all(|&assignment| inside(&region, assignment))
                            );
                            let changed = (0..atoms)
                                .any(|atom| region.decision(atom) != before.decision(atom));
                            assert_eq!(step == SubjectStep::Changed, changed);
                            assert_eq!(
                                result.statistics.propagations,
                                result.statistics.held + result.statistics.cut
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_root_unit_is_seen_without_a_new_decision() {
    for atoms in [1, 63, 64, 65, 127, 128, 129] {
        let system = system(atoms, &[(vec![atoms - 1], true)]);
        let mut region = Region::all_open(atoms);
        let result = system.inspect_reserved(
            &mut region,
            &mut system.knowledge(),
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut Meter::local(u64::MAX),
        );
        assert_eq!(result.result, Ok(SubjectStep::Changed));
        assert_eq!(region.decision(atoms - 1), Some(true));
        assert_eq!(
            (
                result.statistics.propagations,
                result.statistics.held,
                result.statistics.cut
            ),
            (1, 1, 0)
        );
    }
}

#[test]
fn basis_ordinals_resolve_their_original_row_ids() {
    let system = system(
        3,
        &[
            (vec![], false),
            (vec![0, 1], false),
            (vec![], false),
            (vec![1, 2], false),
        ],
    );
    assert_eq!(system.prepared.basis, [1, 3]);
    let mut region = Region::all_open(3);
    assert!(region.hold(1));
    let result = system.inspect_reserved(
        &mut region,
        &mut system.knowledge(),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Changed));
    assert!(region.is_held(0) && region.is_held(2));
}

#[test]
fn pre_batch_snapshots_preserve_the_next_row_trigger() {
    let system = chain(4);
    let mut known = system.knowledge();
    let mut region = Region::all_open(4);
    let mut scratch = NarrowingScratch::default();
    let mut work = Meter::local(u64::MAX);
    let token = Cancellation::default();
    assert_eq!(
        system
            .inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Unchanged)
    );
    assert_eq!(system.prepared.index.as_ref().unwrap().row_words, 2);
    assert!(region.hold(1));
    let first = system.inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work);
    assert_eq!(first.result, Ok(SubjectStep::Changed));
    assert_eq!(first.statistics.held, 2);
    assert!(region.is_held(0) && region.is_held(2) && region.is_open(3));
    assert_eq!(known.snapshot.old_masks().0, [2]);
    let next = system.inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work);
    assert_eq!(next.result, Ok(SubjectStep::Changed));
    assert_eq!(next.statistics.held, 1);
    assert!(region.is_held(3));
}

#[test]
fn odd_multiword_index_stride_reaches_the_last_row() {
    let system = chain(130);
    let index = system.prepared.index.as_ref().unwrap();
    assert_eq!((index.rows, index.row_words), (129, 4));
    let mut known = system.knowledge();
    let mut region = Region::all_open(130);
    let mut scratch = NarrowingScratch::default();
    let mut work = Meter::local(u64::MAX);
    let token = Cancellation::default();
    assert_eq!(
        system
            .inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Unchanged)
    );
    assert!(region.hold(129));
    let result = system.inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work);
    assert_eq!(result.result, Ok(SubjectStep::Changed));
    assert_eq!(result.statistics.held, 1);
    assert!(region.is_held(128) && region.is_open(127));
}

#[test]
fn equal_consequences_count_only_one_fresh_decision() {
    let system = system(3, &[(vec![0, 2], false), (vec![1, 2], false)]);
    let mut region = Region::all_open(3);
    assert!(region.hold(0) && region.hold(1));
    let result = system.inspect_reserved(
        &mut region,
        &mut system.knowledge(),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Changed));
    assert_eq!(
        (result.statistics.propagations, result.statistics.held),
        (1, 1)
    );
}

#[test]
fn opposite_consequences_retain_the_applied_prefix() {
    let system = system(3, &[(vec![0, 2], false), (vec![1, 2], false)]);
    let mut region = Region::all_open(3);
    assert!(region.hold(0) && region.cut(1));
    let result = system.inspect_reserved(
        &mut region,
        &mut system.knowledge(),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Refuted));
    assert!(region.is_held(2));
    assert_eq!(
        (
            result.statistics.propagations,
            result.statistics.held,
            result.statistics.cut
        ),
        (1, 1, 0)
    );
}

#[test]
fn empty_systems_and_zero_row_refutations_are_distinct() {
    for (specifications, expected) in [
        (vec![], SubjectStep::Unchanged),
        (vec![(vec![], true)], SubjectStep::Refuted),
    ] {
        let system = system(0, &specifications);
        let result = system.inspect_reserved(
            &mut Region::all_open(0),
            &mut system.knowledge(),
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut Meter::local(u64::MAX),
        );
        assert_eq!(result.result, Ok(expected));
        assert_eq!(result.statistics.propagations, 0);
    }
}

#[test]
fn an_empty_basis_needs_no_region_snapshot_allocation() {
    let system = system(129, &[]);
    let mut known = system.knowledge();
    let before = known.retained_bytes();
    let mut scratch = NarrowingScratch::default();
    let result = system.inspect_reserved(
        &mut Region::all_open(129),
        &mut known,
        &mut scratch,
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Unchanged));
    assert_eq!(known.retained_bytes(), before);
    assert_eq!(scratch.arithmetic.allocated_bytes(), 0);
}

#[test]
fn cancellation_precedes_invalid_state_and_work_refusal() {
    let system = chain(4);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut scratch = NarrowingScratch::default();
    scratch.arithmetic.consequences.push((0, true));
    let result = system.inspect_reserved(
        &mut Region::all_open(3),
        &mut system.knowledge(),
        &mut scratch,
        &cancellation,
        &mut Meter::local(0),
    );
    assert_eq!(result.result, Err(Failure::Stopped(Stop::Cancelled)));
    assert_eq!(result.statistics.work, 0);
    assert_eq!(scratch.arithmetic.consequences, [(0, true)]);
}

#[test]
fn detected_non_descendants_are_not_refutations() {
    let system = chain(4);
    let mut known = system.knowledge();
    let mut region = Region::all_open(4);
    assert!(region.hold(1));
    let mut scratch = NarrowingScratch::default();
    let token = Cancellation::default();
    let mut work = Meter::local(u64::MAX);
    assert_eq!(
        system
            .inspect_reserved(&mut region, &mut known, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Changed)
    );
    let result = system.inspect_reserved(
        &mut Region::all_open(4),
        &mut known,
        &mut scratch,
        &token,
        &mut work,
    );
    assert_eq!(result.result, Err(Failure::Stopped(Stop::InvalidProgram)));
    assert_eq!(result.statistics.propagations, 0);
}

#[test]
fn wrong_width_is_an_invariant_failure_before_publication() {
    let system = chain(4);
    let mut known = system.knowledge();
    let retained = known.retained_bytes();
    let result = system.inspect_reserved(
        &mut Region::all_open(3),
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Err(Failure::Stopped(Stop::InvalidProgram)));
    assert_eq!(result.statistics.propagations, 0);
    assert_eq!(known.retained_bytes(), retained);
}

#[test]
fn inspection_refutation_publishes_no_collected_decision() {
    let system = system(3, &[(vec![0, 1], false), (vec![2], false)]);
    let mut known = system.knowledge();
    let mut region = Region::all_open(3);
    assert!(region.hold(0) && region.hold(2));
    let result = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Refuted));
    assert!(region.is_open(1));
    assert_eq!(result.statistics.propagations, 0);
    assert!(!known.snapshot.initialized());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Ticket(u8),
    Stopped(Stop),
}

impl zetesis_algebra::Refusal for Refusal {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Ticket(_) => true,
            Self::Stopped(stop) => <Stop as zetesis_algebra::Refusal>::is_resource_refusal(stop),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

#[test]
fn caller_tickets_and_control_stops_remain_distinct() {
    let stopped = <Refusal as zetesis_algebra::Refusal>::from_stop(Stop::Cancelled);
    assert_eq!(stopped, Refusal::Stopped(Stop::Cancelled));
    assert!(!zetesis_algebra::Refusal::is_resource_refusal(&stopped));
    assert!(zetesis_algebra::Refusal::is_resource_refusal(
        &Refusal::Ticket(73)
    ));
}

struct Limited {
    available: u64,
    reserved: u64,
    refunded: u64,
}

impl zetesis_algebra::Quota for Limited {
    type Error = Refusal;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Refusal> {
        let grant = wanted.min(self.available);
        if grant == 0 {
            return Err(Refusal::Ticket(73));
        }
        self.available -= grant;
        self.reserved += grant;
        Ok(grant)
    }
    fn refund(&mut self, unused: u64) {
        self.refunded += unused;
    }
}

#[test]
fn every_work_cutoff_retains_typed_failure_and_receipt() {
    let system = chain(4);
    let mut original = Region::all_open(4);
    assert!(original.hold(1));
    let complete = system.inspect_reserved(
        &mut original.clone(),
        &mut system.knowledge(),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(complete.result, Ok(SubjectStep::Changed));
    let mut observed_applied_prefix = false;
    for limit in 0..complete.statistics.work {
        let mut quota = Limited {
            available: limit,
            reserved: 0,
            refunded: 0,
        };
        let mut region = original.clone();
        let result = system.inspect_reserved(
            &mut region,
            &mut system.knowledge(),
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut Meter::reserved(&mut quota),
        );
        assert_eq!(
            result.result,
            Err(Failure::Refused(Refusal::Ticket(73))),
            "limit {limit}"
        );
        assert_eq!(
            result.statistics.work,
            quota.reserved - quota.refunded,
            "limit {limit}"
        );
        assert!(result.statistics.work <= limit);
        let fresh = (0..4)
            .filter(|&atom| original.is_open(atom) && !region.is_open(atom))
            .count();
        assert_eq!(
            result.statistics.propagations,
            u64::try_from(fresh).unwrap()
        );
        observed_applied_prefix |= fresh != 0;
        assert!(region.is_held(1));
    }
    assert!(
        observed_applied_prefix,
        "some refusals occur after a sound decision is published"
    );
}

#[test]
fn metered_copy_overwrites_stale_polarities_independently() {
    let system = chain(4);
    let mut source = system.knowledge();
    let mut old_target = system.knowledge();
    let mut scratch = NarrowingScratch::default();
    let token = Cancellation::default();
    let mut work = Meter::local(u64::MAX);
    let mut held = Region::all_open(4);
    assert!(held.hold(1));
    assert_eq!(
        system
            .inspect_reserved(&mut held, &mut source, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Changed)
    );
    let mut cut = Region::all_open(4);
    assert!(cut.cut(1));
    assert_eq!(
        system
            .inspect_reserved(&mut cut, &mut old_target, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Changed)
    );
    let retained = old_target.retained_bytes();
    let before = (
        source.snapshot.old_masks().0.to_vec(),
        source.snapshot.old_masks().1.to_vec(),
    );
    let mut destination = Some(old_target);
    source
        .copy_reserved(&mut destination, &token, &mut work)
        .unwrap();
    let copied = destination.as_mut().unwrap();
    assert_eq!(copied.snapshot.old_masks(), source.snapshot.old_masks());
    assert_ne!(
        copied.snapshot.old_masks().0.as_ptr(),
        source.snapshot.old_masks().0.as_ptr()
    );
    assert_eq!(copied.retained_bytes(), retained);
    assert!(held.hold(3));
    assert_eq!(
        system
            .inspect_reserved(&mut held, copied, &mut scratch, &token, &mut work)
            .result,
        Ok(SubjectStep::Unchanged)
    );
    assert_eq!(source.snapshot.old_masks(), (&before.0[..], &before.1[..]));
}

#[test]
fn cancelled_copy_invalidates_only_the_destination() {
    let system = chain(4);
    let source = system.knowledge();
    let retained = source.retained_bytes();
    let mut destination = Some(system.knowledge());
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut work = Meter::local(0);
    assert_eq!(
        source.copy_reserved(&mut destination, &cancellation, &mut work),
        Err(Failure::Stopped(Stop::Cancelled))
    );
    assert!(destination.is_none());
    assert_eq!(work.charged(), 0);
    assert_eq!(source.retained_bytes(), retained);
}

#[test]
fn refused_copies_never_publish_a_partial_destination() {
    let system = chain(4);
    let mut source = system.knowledge();
    let mut region = Region::all_open(4);
    assert!(region.hold(1));
    let token = Cancellation::default();
    assert_eq!(
        system
            .inspect_reserved(
                &mut region,
                &mut source,
                &mut NarrowingScratch::default(),
                &token,
                &mut Meter::local(u64::MAX),
            )
            .result,
        Ok(SubjectStep::Changed)
    );
    let before = (
        source.snapshot.old_masks().0.to_vec(),
        source.snapshot.old_masks().1.to_vec(),
    );
    let mut complete_work = Meter::local(u64::MAX);
    source
        .copy_reserved(&mut None, &token, &mut complete_work)
        .unwrap();
    for limit in 0..complete_work.charged() {
        let mut quota = Limited {
            available: limit,
            reserved: 0,
            refunded: 0,
        };
        let mut work = Meter::reserved(&mut quota);
        let mut destination = None;
        let result = source.copy_reserved(&mut destination, &token, &mut work);
        assert_eq!(
            result,
            Err(Failure::Refused(Refusal::Ticket(73))),
            "limit {limit}"
        );
        assert!(destination.is_none(), "limit {limit}");
        work.refund();
        let charged = work.charged();
        assert_eq!(charged, quota.reserved - quota.refunded, "limit {limit}");
        assert_eq!(source.snapshot.old_masks(), (&before.0[..], &before.1[..]));
    }
}

fn inspected_open_65() -> (
    ParitySystem,
    Region,
    super::ParityKnowledge,
    NarrowingScratch,
) {
    let system = system(65, &[(vec![0, 1], false)]);
    let mut region = Region::all_open(65);
    let mut known = system.knowledge();
    let mut scratch = NarrowingScratch::default();
    let attempt = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut scratch,
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(attempt.result, Ok(SubjectStep::Unchanged));
    assert_eq!(system.prepared.basis.len(), 1);
    (system, region, known, scratch)
}

#[test]
fn empty_selection_keeps_fresh_decision_scratch() {
    let (system, mut region, mut known, mut scratch) = inspected_open_65();
    // The first inspection made no deductions but selected a real row and
    // built its open mask. No deductions is not the skip criterion.
    assert_eq!(&*scratch.arithmetic.touched, &[1]);
    assert_eq!(&*scratch.arithmetic.open, &[u64::MAX, 1]);
    let before_capacity = scratch.arithmetic.open.capacity();
    let result = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut scratch,
        &Cancellation::default(),
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Unchanged));
    assert_eq!(result.statistics.propagations, 0);
    assert_eq!(&*scratch.arithmetic.touched, &[0]);
    assert_eq!(&*scratch.arithmetic.open, &[0, 0]);
    assert_eq!(scratch.arithmetic.open.capacity(), before_capacity);
    assert_eq!(known.snapshot.old_masks(), region.decision_words());
    assert_eq!(region.open().count(), 65);
}

#[test]
fn unrelated_decisions_are_published_before_later_row_triggers() {
    let (system, mut region, mut known, mut scratch) = inspected_open_65();
    let token = Cancellation::default();
    assert!(region.hold(64)); // no coefficient in the prepared parity row
    let result = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut scratch,
        &token,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Unchanged));
    assert_eq!(&*scratch.arithmetic.touched, &[0]);
    assert_eq!(&*scratch.arithmetic.open, &[0, 1]);
    assert_eq!(known.snapshot.old_masks(), region.decision_words());

    assert!(region.hold(0));
    let result = system.inspect_reserved(
        &mut region,
        &mut known,
        &mut scratch,
        &token,
        &mut Meter::local(u64::MAX),
    );
    assert_eq!(result.result, Ok(SubjectStep::Changed));
    assert_eq!(result.statistics.held, 1);
    assert!(region.is_held(1));
    assert_eq!(known.snapshot.old_masks().0, &[1, 1]);
}

#[test]
fn empty_selection_cutoffs_retain_the_exact_prefix() {
    // Isolate the post-validation body so this receipt remains independent of
    // the separate ancestry implementation. W=2, T=1: entry4 + selection21
    // + empty detection2 + snapshot5 + final result1 = 33 units.
    for limit in 0..=33 {
        let (system, mut region, mut known, mut scratch) = inspected_open_65();
        known
            .snapshot
            .validate(&region, 65, &mut Meter::local(u64::MAX))
            .unwrap();
        let mut work = Meter::local(limit);
        let mut statistics = crate::regions::NarrowingStatistics::default();
        let result = super::inspect(
            &system,
            &mut region,
            &mut known.snapshot,
            (&mut scratch.arithmetic, None),
            &mut work,
            &mut statistics,
        );
        assert_eq!(
            result,
            if limit == 33 {
                Ok(SubjectStep::Unchanged)
            } else {
                Err(Failure::Stopped(Stop::WorkLimit))
            },
            "limit {limit}"
        );
        // Snapshot preparation's two-unit admission follows four entry units;
        // all other operations on this established empty-selection route are one.
        assert_eq!(work.charged(), if limit == 5 { 4 } else { limit });
        assert_eq!(statistics.propagations, 0);
        assert_eq!(region.open().count(), 65);
    }
}
