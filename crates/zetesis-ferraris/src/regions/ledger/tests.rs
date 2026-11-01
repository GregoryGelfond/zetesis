//! Reusable explanations belong to the exact original theory, not the region,
//! inherited knowledge, a restriction-only visit or a frozen reduct query.

use super::{Deduction, LearningLimits, LearningOwner, Ledger, Literals};
use crate::regions::founded::explanations::Input;
use crate::regions::{SupportExplanationStatistics, Width, Work};
use crate::test_support::{SinglePermits, theory, world};
use crate::{
    EvaluationLimits, EvaluationWorkspace, FrozenSubject, Knowledge, Narrower, Narrowing,
    NarrowingScratch, NarrowingStatistics, Node, OriginalSubject, Producers, Region, RegionLimits,
    Theory,
};
use zetesis_algebra::{Cancellation, Stop};

mod carrier;
mod consolidation;
mod early;
mod eviction;
mod index;
mod partial;
mod sharing;
mod signed;

struct Program {
    theory: Theory,
    owner: LearningOwner,
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
        let program = theory(
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
        Self::of(program)
    }

    fn of(theory: Theory) -> Self {
        let index = Narrower::new(&theory);
        let producers =
            crate::producers(&theory, RegionLimits::default(), &Cancellation::default())
                .unwrap()
                .producers
                .unwrap();
        Self {
            owner: LearningOwner::theory(&theory, LearningLimits::default()),
            theory,
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
                OriginalSubject::carrier(&self.owner, Some(&self.producers)),
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
            scratch.explanations.rows[0]
                .row
                .conditions()
                .collect::<Vec<_>>(),
            [(2, false), (3, true)]
        );
        assert_eq!(scratch.explanations.absent_outputs(0), [0, 1]);
        (known, scratch)
    }
}

/// Signed conditions and signed outputs.
fn literals(conditions: &[(usize, bool)], outputs: &[(usize, bool)]) -> Literals {
    Literals {
        conditions: conditions.to_vec(),
        outputs: outputs.to_vec(),
    }
}

/// An empty tier of `theory`'s class under `limits`, whose founded allowance
/// does not bound direct retention.
fn owned(theory: &Theory, limits: LearningLimits) -> Ledger {
    let mut ledger = Ledger::default();
    ledger.prepare(&LearningOwner::theory(theory, limits));
    ledger.remaining = u64::MAX;
    ledger
}

/// Retain a row as the founded source would; true when it was added.
fn add(ledger: &mut Ledger, conditions: &[(usize, bool)], outputs: &[(usize, bool)]) -> bool {
    ledger
        .retain_founded(
            literals(conditions, outputs),
            &mut Work::new(u64::MAX),
            &mut 0,
        )
        .unwrap()
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
        theory: &program.theory,
        index: &program.index,
        producers: &program.producers,
        completed: &scratch.founded,
        atom_sure,
        atom_never,
    }
}

#[test]
fn small_allowances_keep_the_default_limits() {
    assert_eq!(LearningLimits::for_allowance(0), LearningLimits::default());
    assert_eq!(
        LearningLimits::for_allowance(64 << 20),
        LearningLimits::default()
    );
}

#[test]
fn tier_rows_grow_with_the_allowance_up_to_their_ceiling() {
    let limits = LearningLimits::for_allowance(512 << 20);
    assert_eq!((limits.tier_rows, limits.store_rows), (512, 2048));
    assert_eq!(LearningLimits::for_allowance(usize::MAX).tier_rows, 4096);
}

#[test]
fn the_founded_output_limit_declines_a_wider_row() {
    let mut program = Program::new(false);
    program.owner = LearningOwner::theory(
        &program.theory,
        LearningLimits {
            founded_outputs: 1,
            ..LearningLimits::default()
        },
    );
    let mut scratch = NarrowingScratch::default();
    let (result, counts) = program.close(
        &mut region(&[(0, true), (2, false), (3, true), (4, false)]),
        &mut program.index.knowledge(),
        &mut scratch,
    );
    assert_eq!(result, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.attempts, 1);
    assert_eq!(counts.support_explanations.declined, 1);
    assert_eq!(scratch.explanations.len(), 0);
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
                &program.theory,
                &world(&program.theory, mask),
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
                    Deduction::ForceOutputs
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
    let candidate = world(&program.theory, 0b01011);
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
            FrozenSubject::new(&program.theory, evaluated.truth()),
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
            OriginalSubject::new(&program.theory, None),
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
    assert!(!original.theory.same_instance(&foreign.theory));
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
            .same_class(&foreign.theory)
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
    assert!(measured.spent() > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent() {
            let mut ledger = Ledger::default();
            ledger.prepare(&program.owner);
            let mut counts = SupportExplanationStatistics::default();
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                ledger.capture(input(&program, &known, &scratch), 0, &mut work, &mut counts),
                Err(failure)
            );
            assert_eq!(work.spent(), prefix);
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
        Deduction::ForceOutputs
    );
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent() {
            let mut permits = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut permits);
            assert_eq!(
                scratch.explanations.deduction(0, &held, &cut, &mut work),
                Err(failure)
            );
            assert_eq!(work.spent(), prefix);
            assert_eq!(scratch.explanations.len(), 1);
        }
    }
}
