//! Reusable explanations belong to the exact original theory, not the region,
//! inherited knowledge, a restriction-only visit or a frozen reduct query.

use super::{Deduction, Ledger};
use crate::regions::founded::explanations::Input;
use crate::regions::{SupportExplanationStatistics, Width, Work};
use crate::test_support::{SinglePermits, theory, world};
use crate::{
    EvaluationLimits, EvaluationWorkspace, FrozenSubject, Knowledge, Narrower, Narrowing,
    NarrowingScratch, NarrowingStatistics, Node, OriginalSubject, Producers, Region, RegionLimits,
    Theory,
};
use zetesis_cpu::{Cancellation, Stop};

mod partial;

struct Program {
    owner: Theory,
    index: Narrower,
    producers: Producers,
}

impl Program {
    /// a <-> b; a <- x; b <- not y; independent choices x,y,z.
    fn new(seeded: bool) -> Self {
        let mut roots = vec![9, 10, 11, 12, 13, 14, 15];
        if seeded {
            roots.push(0);
        }
        let owner = theory(
            5,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::atom(3),
                Node::atom(4),
                Node::falsum(),
                Node::implies(2, 5),
                Node::implies(3, 5),
                Node::implies(4, 5),
                Node::or_pair([2, 6]),
                Node::or_pair([3, 7]),
                Node::or_pair([4, 8]),
                Node::implies(1, 0),
                Node::implies(0, 1),
                Node::implies(2, 0),
                Node::implies(7, 1),
            ],
            roots,
        );
        let index = Narrower::new(&owner);
        let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .unwrap();
        Self {
            owner,
            index,
            producers,
        }
    }

    fn close(
        &self,
        region: &mut Region,
        known: &mut Knowledge,
        scratch: &mut NarrowingScratch,
    ) -> (Narrowing, NarrowingStatistics) {
        self.index
            .narrow_known(
                OriginalSubject::new(&self.owner, Some(&self.producers)),
                region,
                known,
                scratch,
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
    }

    fn captured(&self) -> (Knowledge, NarrowingScratch) {
        let mut region = region(&[(0, true), (2, false), (3, true), (4, false)]);
        let mut known = self.index.knowledge();
        let mut scratch = NarrowingScratch::default();
        let (result, counts) = self.close(&mut region, &mut known, &mut scratch);
        assert_eq!(result, Narrowing::Refuted);
        assert_eq!(counts.support_explanations.certified, 1);
        assert_eq!(counts.support_explanations.refuted, 0);
        assert!(scratch.founded.passes > 0);
        assert_eq!(scratch.explanations.len(), 1);
        assert_eq!(
            scratch.explanations.rows[0].conditions,
            [(2, false), (3, true)]
        );
        assert_eq!(scratch.explanations.outputs(0), [0, 1]);
        (known, scratch)
    }
}

fn region(decisions: &[(usize, bool)]) -> Region {
    let mut result = Region::all_open(5);
    for &(atom, value) in decisions {
        assert!(if value {
            result.hold(atom)
        } else {
            result.cut(atom)
        });
    }
    result
}

fn compatible(region: &Region, mask: usize) -> bool {
    (0..5).all(|atom| {
        (!region.is_held(atom) || mask & (1 << atom) != 0)
            && (!region.is_cut(atom) || mask & (1 << atom) == 0)
    })
}

fn masks(known: &Knowledge) -> (&[u64], &[u64]) {
    let slices = match &known.width {
        Width::Compact16(known) => known.masks.slices(),
        Width::Compact32(known) => known.masks.slices(),
        Width::Native(known) => known.masks.slices(),
    };
    (slices[2], slices[3])
}

fn input<'a>(
    program: &'a Program,
    known: &'a Knowledge,
    scratch: &'a NarrowingScratch,
) -> Input<'a> {
    let (atom_sure, atom_never) = masks(known);
    Input {
        theory: &program.owner,
        index: &program.index,
        producers: &program.producers,
        completed: &scratch.founded,
        atom_sure,
        atom_never,
    }
}

#[test]
fn fresh_sibling_knowledge_reuses_a_complete_explanation() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let before = scratch.founded.passes;
    let mut sibling = region(&[(0, true), (2, false), (3, true), (4, true)]);
    let (result, counts) =
        program.close(&mut sibling, &mut program.index.knowledge(), &mut scratch);
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.refuted, 1);
    assert_eq!(counts.support_explanations.attempts, 0);
    assert_eq!(scratch.founded.passes, before);
}

#[test]
fn matching_conditions_cut_open_unsupported_atoms() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let mut child = region(&[(2, false), (3, true)]);
    let (result, counts) = program.close(&mut child, &mut program.index.knowledge(), &mut scratch);
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert!(child.is_cut(0) && child.is_cut(1));
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.cut, 2);
    assert_eq!(counts.support_explanations.refuted, 0);
}

#[test]
fn all_partial_regions_preserve_every_original_answer() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let answers: Vec<_> = (0..32)
        .filter(|&mask| {
            crate::check(
                &program.owner,
                &world(&program.owner, mask),
                crate::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .collect();
    let mut matched = 0;
    let mut forced = 0;
    for mut code in 0..243 {
        let mut initial = Region::all_open(5);
        for atom in 0..5 {
            match code % 3 {
                1 => {
                    assert!(initial.cut(atom));
                }
                2 => {
                    assert!(initial.hold(atom));
                }
                _ => {}
            }
            code /= 3;
        }
        let mut narrowed = initial.clone();
        let (result, counts) =
            program.close(&mut narrowed, &mut program.index.knowledge(), &mut scratch);
        matched += counts.support_explanations.matched;
        forced += counts.support_explanations.forced;
        for &answer in &answers {
            if compatible(&initial, answer) {
                assert_ne!(result, Narrowing::Refuted);
                assert!(compatible(&narrowed, answer));
            }
        }
    }
    assert!(
        matched > 0,
        "the comparison must exercise retained explanations"
    );
    assert!(forced > 0, "the comparison must exercise partial matching");
}

#[test]
fn matching_requires_every_signed_condition() {
    let program = Program::new(false);
    let (_, scratch) = program.captured();
    for x in 0..3 {
        for y in 0..3 {
            let held = [u64::from(x == 2) << 2 | u64::from(y == 2) << 3];
            let cut = [u64::from(x == 1) << 2 | u64::from(y == 1) << 3];
            assert_eq!(
                scratch
                    .explanations
                    .deduction(0, &held, &cut, &mut Work::new(u64::MAX))
                    .unwrap(),
                if x == 1 && y == 2 {
                    Deduction::ExcludeOutputs
                } else {
                    Deduction::None
                }
            );
        }
    }
}

#[test]
fn frozen_and_inventory_free_visits_suspend_original_explanations() {
    let program = Program::new(false);
    let (_, mut scratch) = program.captured();
    let candidate = world(&program.owner, 0b01011);
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace
        .evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let decisions = [(0, true), (2, false), (3, true), (4, false)];
    let (frozen, frozen_counts) = program
        .index
        .narrow_frozen_known(
            FrozenSubject::new(&program.owner, evaluated.truth()),
            &mut region(&decisions),
            &mut program.index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(frozen, Narrowing::Fixed { .. }));
    assert_eq!(frozen_counts.support_explanations.work, 0);
    assert_eq!(scratch.explanations.len(), 1);
    let (without_inventory, inventory_counts) = program
        .index
        .narrow_known(
            OriginalSubject::new(&program.owner, None),
            &mut region(&decisions),
            &mut program.index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(without_inventory, Narrowing::Fixed { .. }));
    assert_eq!(inventory_counts.support_explanations.work, 0);
    assert_eq!(scratch.explanations.len(), 1);
    let (original, original_counts) = program.close(
        &mut region(&decisions),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(original, Narrowing::Refuted);
    assert_eq!(original_counts.support_explanations.refuted, 1);
}

#[test]
fn a_foreign_theory_discards_previous_explanations() {
    let original = Program::new(false);
    let foreign = Program::new(true);
    let (_, mut scratch) = original.captured();
    assert!(!original.owner.same_instance(&foreign.owner));
    let (result, counts) = foreign.close(
        &mut region(&[(0, true), (2, false), (3, true)]),
        &mut foreign.index.knowledge(),
        &mut scratch,
    );
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(counts.support_explanations.refuted, 0);
    assert_eq!(scratch.explanations.len(), 0);
    assert!(
        scratch
            .explanations
            .owner
            .as_ref()
            .unwrap()
            .same_instance(&foreign.owner)
    );
}

#[test]
fn interrupted_capture_never_publishes_partial_evidence() {
    let program = Program::new(false);
    // The first published atom is already held, so the completed pass is
    // followed by a contradiction without changing its input atom masks.
    let (known, scratch) = program.captured();
    let mut complete = Ledger::default();
    complete.prepare(&program.owner);
    let mut measured = Work::new(u64::MAX);
    complete
        .capture(
            input(&program, &known, &scratch),
            0,
            &mut measured,
            &mut SupportExplanationStatistics::default(),
        )
        .unwrap();
    assert_eq!(complete.len(), 1);
    assert!(measured.spent > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut ledger = Ledger::default();
            ledger.prepare(&program.owner);
            let mut counts = SupportExplanationStatistics::default();
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                ledger.capture(input(&program, &known, &scratch), 0, &mut work, &mut counts),
                Err(failure)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(ledger.len(), 0);
            assert_eq!(counts.certified, 0);
            ledger
                .capture(
                    input(&program, &known, &scratch),
                    0,
                    &mut Work::new(u64::MAX),
                    &mut counts,
                )
                .unwrap();
            assert_eq!(ledger.rows, complete.rows);
        }
    }
}

#[test]
fn interrupted_matching_preserves_the_typed_failure() {
    let program = Program::new(false);
    let (_, scratch) = program.captured();
    let held = [1 << 3];
    let cut = [1 << 2];
    let mut measured = Work::new(u64::MAX);
    assert_eq!(
        scratch
            .explanations
            .deduction(0, &held, &cut, &mut measured)
            .unwrap(),
        Deduction::ExcludeOutputs
    );
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                scratch.explanations.deduction(0, &held, &cut, &mut work),
                Err(failure)
            );
            assert_eq!(work.spent, prefix);
            assert_eq!(scratch.explanations.len(), 1);
        }
    }
}
