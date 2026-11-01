//! Class markers constrain the actual inputs, without owning regional learning.

use std::mem::{align_of, size_of, size_of_val};

use zetesis_algebra::{AnswerSetsSought, Cancellation, Countermodels};

use super::super::{Context, LearningAccess, ReadingClass, Width};
use crate::test_support::theory;
use crate::{
    FrozenSubject, Knowledge, Narrower, NarrowingScratch, Node, OriginalSubject, Region,
    RegionLimits,
};

/// Choices for both atoms, a disjunctive fact, and two observed guards on
/// separate numeric rows. Each guard also occurs in its choice tautology.
fn numeric_choice_theory() -> crate::Theory {
    use crate::{ExtremumCutDescriptor, ExtremumCutRule, NumericDomain, NumericRow};
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::falsum(),
        Node::implies(0, 2),
        Node::implies(1, 2),
        Node::or_pair([0, 3]),
        Node::or_pair([1, 4]),
        Node::or_pair([0, 1]),
        Node::threshold(0),
        Node::extremum_cut(0),
        Node::implies(8, 2),
        Node::implies(9, 2),
        Node::or_pair([8, 10]),
        Node::or_pair([9, 11]),
    ];
    let parts = crate::FormulaParts::from_numeric_raw(
        nodes,
        vec![],
        vec![0, 0, 1, 0, 1],
        vec![1, 1, 2, 0, 3],
        vec![
            NumericRow {
                start: 0,
                length: 3,
                domain: NumericDomain::Weights,
            },
            NumericRow {
                start: 3,
                length: 2,
                domain: NumericDomain::Ordinal { rank_count: 4 },
            },
        ],
        vec![crate::ThresholdDescriptor { row: 0, bound: 3 }],
        vec![ExtremumCutDescriptor {
            row: 1,
            rule: ExtremumCutRule::Suffix(2),
        }],
    )
    .unwrap();
    crate::Theory::new(
        2,
        parts,
        vec![5, 6, 7, 12, 13],
        crate::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn known_node_signs_hold_in_every_stable_completion() {
    let theory = numeric_choice_theory();
    let index = Narrower::new(&theory);
    let cancel = Cancellation::default();
    let mut answers = Vec::new();
    let mut evaluation = crate::EvaluationWorkspace::default();
    for mask in 0..4 {
        let world = crate::test_support::world(&theory, mask);
        if crate::check(&theory, &world, crate::Limits::default(), &cancel)
            .unwrap()
            .accepted()
        {
            let read = evaluation
                .evaluate(&world, crate::EvaluationLimits::default(), &cancel)
                .result
                .unwrap();
            let truth = read.truth().to_vec();
            answers.push((world, truth));
        }
    }
    assert_eq!(answers.len(), 3);

    let mut observed = [[false; 2]; 2];
    for pattern in 0..9 {
        let mut region = Region::all_open(2);
        for (atom, decision) in [pattern % 3, pattern / 3].into_iter().enumerate() {
            match decision {
                1 => assert!(region.hold(atom)),
                2 => assert!(region.cut(atom)),
                _ => {}
            }
        }
        let before = region.clone();
        let mut knowledge = index.knowledge();
        let (step, _) = crate::test_support::close(
            &theory,
            &index,
            &mut region,
            &mut knowledge,
            &mut NarrowingScratch::default(),
        );
        let masks = super::compact(&knowledge).masks.slices();
        for (world, truth) in &answers {
            if before
                .decided()
                .any(|(a, value)| world.contains(a) != value)
            {
                continue;
            }
            assert_ne!(step, zetesis_algebra::region::Narrowing::Refuted);
            assert!(
                region
                    .decided()
                    .all(|(a, value)| world.contains(a) == value)
            );
            for (id, &actual) in truth.iter().enumerate() {
                let holds = super::super::bit(masks[0], id);
                let fails = super::super::bit(masks[1], id);
                assert!(!holds || actual, "held node {id} in region {pattern}");
                assert!(!fails || !actual, "failed node {id} in region {pattern}");
                if (8..=9).contains(&id) && (holds || fails) {
                    observed[id - 8][usize::from(actual)] = true;
                }
            }
        }
    }
    // A missing all-unknown numeric mask must not make the comparisons vacuous.
    assert_eq!(observed, [[true; 2]; 2]);
}

/// h follows from a threshold on the choice b, or from unsupported d.
/// The classical interpretation {h} satisfies all roots. Only {b,h} is
/// stable in the region holding h, so the threshold body is forced there.
#[test]
fn sole_numeric_body_is_known_only_for_stable_completions() {
    let parts = crate::FormulaParts::from_numeric_raw(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(1, 3),
            Node::or_pair([1, 4]),
            Node::threshold(0),
            Node::implies(6, 0),
            Node::implies(2, 0),
            Node::implies(2, 2),
        ],
        vec![],
        vec![1],
        vec![1],
        vec![crate::NumericRow {
            start: 0,
            length: 1,
            domain: crate::NumericDomain::Weights,
        }],
        vec![crate::ThresholdDescriptor { row: 0, bound: 1 }],
        vec![],
    )
    .unwrap();
    let theory = crate::Theory::new(
        3,
        parts,
        vec![5, 7, 8, 9],
        crate::AdmissionLimits::default(),
    )
    .unwrap();
    let cancel = Cancellation::default();
    let h = crate::test_support::world(&theory, 1);
    let mut oracle = crate::EvaluationWorkspace::default();
    let reading = oracle
        .evaluate(&h, crate::EvaluationLimits::default(), &cancel)
        .result
        .unwrap();
    assert!(theory.roots().iter().all(|&root| reading.truth()[root]));
    assert!(!reading.truth()[6]);
    let stable = (0..8)
        .filter(|&bits| {
            let m = crate::test_support::world(&theory, bits);
            crate::check(&theory, &m, crate::Limits::default(), &cancel)
                .unwrap()
                .accepted()
        })
        .collect::<Vec<_>>();
    assert_eq!(stable, [0, 3]);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(3);
    assert!(region.hold(0));
    let mut ordinary = index.knowledge();
    let (ordinary_step, _) = crate::test_support::close(
        &theory,
        &index,
        &mut region.clone(),
        &mut ordinary,
        &mut NarrowingScratch::default(),
    );
    assert_ne!(ordinary_step, zetesis_algebra::region::Narrowing::Refuted);
    assert!(!super::super::bit(
        super::compact(&ordinary).masks.slices()[0],
        6
    ));
    let producers = crate::regions::producers(&theory, RegionLimits::default(), &cancel)
        .unwrap()
        .producers
        .expect("complete ordinary and choice producers");
    let mut known = index.knowledge();
    let step = index
        .narrow_known(
            OriginalSubject::new(&theory, Some(&producers)),
            &mut region,
            &mut known,
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &cancel,
        )
        .result
        .unwrap();
    assert_ne!(step, zetesis_algebra::region::Narrowing::Refuted);
    let masks = super::compact(&known).masks.slices();
    assert!(
        super::super::bit(masks[0], 6),
        "the only possible producer's threshold must hold"
    );
    assert!(
        super::super::bit(masks[1], 2),
        "the unfounded alternative cannot support h"
    );
    assert_eq!(
        region.decided().collect::<Vec<_>>(),
        [(0, true), (1, true), (2, false)]
    );
}

#[test]
fn class_markers_do_not_enlarge_regional_knowledge() {
    assert_eq!(size_of::<Knowledge<AnswerSetsSought>>(), size_of::<Width>());
    assert_eq!(size_of::<Knowledge<Countermodels>>(), size_of::<Width>());
    assert_eq!(
        align_of::<Knowledge<AnswerSetsSought>>(),
        align_of::<Width>()
    );
    assert_eq!(align_of::<Knowledge<Countermodels>>(), align_of::<Width>());
    assert_eq!(
        size_of::<Context<'_, AnswerSetsSought>>(),
        size_of::<OriginalSubject<'_>>()
    );
    assert_eq!(
        size_of::<Context<'_, Countermodels>>(),
        size_of::<FrozenSubject<'_>>()
    );
}

#[test]
fn frozen_execution_has_no_learning_borrow() {
    let mut scratch = NarrowingScratch::default();
    let mut frozen: () = Countermodels::borrow_learning(&mut scratch.learning);
    assert_eq!(size_of_val(&frozen), 0);
    assert!(frozen.state().is_none());
    assert!(frozen.state_mut().is_none());
    let mut original = AnswerSetsSought::borrow_learning(&mut scratch.learning);
    assert!(original.state().is_some());
    assert!(original.state_mut().is_some());
}

#[test]
fn both_concrete_factories_execute_the_same_unmasked_readings() {
    let theory = theory(1, vec![Node::atom(0)], vec![0]);
    let index = Narrower::new(&theory);
    let mut original = index.knowledge();
    let mut frozen = index.frozen_knowledge();
    assert_eq!(original.retained_bytes(), frozen.retained_bytes());
    let mut original_region = Region::all_open(1);
    let mut frozen_region = original_region.clone();
    let mut scratch = NarrowingScratch::default();
    let cancellation = Cancellation::default();
    let a = index.narrow_known(
        OriginalSubject::new(&theory, None),
        &mut original_region,
        &mut original,
        &mut scratch,
        RegionLimits::default(),
        &cancellation,
    );
    let b = index.narrow_frozen_known(
        FrozenSubject::new(&theory, &[true]),
        &mut frozen_region,
        &mut frozen,
        &mut scratch,
        RegionLimits::default(),
        &cancellation,
    );
    assert_eq!(a.result, b.result);
    assert_eq!(a.statistics, b.statistics);
    assert_eq!(a.preferred, b.preferred);
    assert_eq!(original_region, frozen_region);
    assert_eq!(
        super::compact(&original).masks.slices(),
        super::compact(&frozen).masks.slices()
    );
    let mut copied = index.frozen_knowledge();
    copied.clone_from(&frozen);
    super::counters::same_knowledge(&copied, &frozen);
    assert_eq!(scratch.learned_rows(), 0);
}
