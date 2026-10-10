//! Source-level semantic preservation of prefix narrowing.
//!
//! The oracle is the admitted original formula, not the region search result.
//! Frozen subjects use the actual complete original truth in every M. These
//! tests establish preservation, not that frozen prefix propagation is enabled
//! or disabled; that policy needs the internal same-index control test.

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    EvaluationLimits, EvaluationWorkspace, FrozenSubject, Interpretation, Limits, Narrower,
    Narrowing, NarrowingScratch, OriginalSubject, Region, RegionLimits, Theory, models,
    models_reduct,
};
use zetesis_reference_support::formula;

const CASES: &[(&str, &str)] = &[
    (
        "false extension, positive conditions",
        "{a;b;c;d;z}. \
         :- #count{1:a;2:b;3:c;4:d} >= 2. \
         :- z, not #count{1:a;2:b;3:c} >= 2.",
    ),
    (
        "true prefix, positive conditions",
        "{a;b;c;d;z}. \
         :- #count{1:a;2:b;3:c} < 2. \
         :- not z, #count{1:a;2:b;3:c;4:d} >= 2.",
    ),
    (
        "default negation in conditions",
        "{a;b;c;z}. \
         :- #count{1:not a;2:b;3:c;4:not b} >= 2. \
         :- z, not #count{1:not a;2:b;3:c} >= 2.",
    ),
    (
        "distinct tuple keys repeat a condition, written out of order",
        "{a;b;c;z}. \
         :- #count{4:c;2:b;3:a;1:a} >= 2. \
         :- z, not #count{3:a;1:a;2:b} >= 2.",
    ),
    (
        "conjunction and nested default negation conditions",
        "{a;b;c;z}. \
         :- #count{1:a,b;2:not a,c;3:not not b;4:c} >= 2. \
         :- z, not #count{1:a,b;2:not a,c;3:not not b} >= 2.",
    ),
];

fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|&atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

fn partial_region(count: usize, mut code: usize) -> Region {
    let mut region = Region::all_open(count);
    for atom in 0..count {
        match code % 3 {
            0 => {}
            1 => assert!(region.hold(atom)),
            2 => assert!(region.cut(atom)),
            _ => unreachable!(),
        }
        code /= 3;
    }
    assert_eq!(code, 0);
    region
}

fn compatible(region: &Region, count: usize, mask: usize) -> bool {
    (0..count).all(|atom| {
        region
            .decision(atom)
            .is_none_or(|value| value == (mask & (1 << atom) != 0))
    })
}

/// Compare full compatible model sets, including the empty set after refutation.
/// No assertion demands that an incomplete propagator decide every entailment.
fn preserves(
    count: usize,
    model_masks: &[usize],
    before: &Region,
    after: &Region,
    outcome: Narrowing,
    context: &str,
) {
    let before_models: Vec<_> = model_masks
        .iter()
        .copied()
        .filter(|&mask| compatible(before, count, mask))
        .collect();
    let after_models: Vec<_> = model_masks
        .iter()
        .copied()
        .filter(|&mask| outcome != Narrowing::Refuted && compatible(after, count, mask))
        .collect();
    assert_eq!(before_models, after_models, "{context}");
}

#[test]
fn source_prefix_closure_preserves_every_compatible_original_model() {
    let cancellation = Cancellation::default();
    for &(label, source) in CASES {
        let input = formula(source);
        let theory = input.theory();
        let count = theory.atom_count();
        assert!(
            count <= 5,
            "bounded full interpretations and partial regions"
        );
        assert_eq!(theory.parts().thresholds().len(), 2, "{label}");
        let original_models: Vec<_> = (0..1 << count)
            .filter(|&mask| {
                models(
                    theory,
                    &interpretation(theory, mask),
                    Limits::default(),
                    &cancellation,
                )
                .unwrap()
            })
            .collect();
        assert!(!original_models.is_empty(), "nonvacuous source: {label}");
        assert!(
            original_models.len() < 1 << count,
            "nontrivial source: {label}"
        );
        let narrower = Narrower::new(theory);
        for code in 0..3_usize.pow(u32::try_from(count).unwrap()) {
            let before = partial_region(count, code);
            let mut after = before.clone();
            let (outcome, _) = narrower
                .narrow_known(
                    OriginalSubject::new(theory, None),
                    &mut after,
                    &mut narrower.knowledge(),
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &cancellation,
                )
                .unwrap();
            preserves(
                count,
                &original_models,
                &before,
                &after,
                outcome,
                &format!("{label}: original partial region {code}"),
            );
        }
    }
}

#[test]
fn source_prefix_closure_preserves_arbitrary_frozen_models() {
    let cancellation = Cancellation::default();
    for &(label, source) in CASES {
        let input = formula(source);
        let theory = input.theory();
        let count = theory.atom_count();
        assert!(count <= 5, "bounded M, J, and partial regions");
        assert_eq!(theory.parts().thresholds().len(), 2, "{label}");
        let narrower = Narrower::new(theory);
        let mut non_subset_models = 0;
        let mut outers_with_no_frozen_models = 0;
        for outer_mask in 0..1 << count {
            let outer = interpretation(theory, outer_mask);
            let mut workspace = EvaluationWorkspace::default();
            let evaluated = workspace
                .evaluate(&outer, EvaluationLimits::default(), &cancellation)
                .result
                .unwrap();
            let truth: Vec<_> = (0..theory.view().len())
                .map(|node| evaluated.node_truth(node).unwrap())
                .collect();
            let frozen_models: Vec<_> = (0..1 << count)
                .filter(|&inner_mask| {
                    models_reduct(
                        theory,
                        &outer,
                        &interpretation(theory, inner_mask),
                        Limits::default(),
                        &cancellation,
                    )
                    .unwrap()
                })
                .collect();
            non_subset_models += frozen_models
                .iter()
                .filter(|&&inner_mask| inner_mask & !outer_mask != 0)
                .count();
            outers_with_no_frozen_models += usize::from(frozen_models.is_empty());
            for code in 0..3_usize.pow(u32::try_from(count).unwrap()) {
                let before = partial_region(count, code);
                let mut after = before.clone();
                let (outcome, _) = narrower
                    .narrow_frozen_known(
                        FrozenSubject::new(theory, &truth),
                        &mut after,
                        &mut narrower.knowledge(),
                        &mut NarrowingScratch::default(),
                        RegionLimits::default(),
                        &cancellation,
                    )
                    .unwrap();
                preserves(
                    count,
                    &frozen_models,
                    &before,
                    &after,
                    outcome,
                    &format!("{label}: M={outer_mask}, frozen partial region {code}"),
                );
            }
        }
        assert!(
            non_subset_models > 0,
            "must test actual J not subset M models: {label}"
        );
        assert!(
            outers_with_no_frozen_models > 0,
            "must also test false original root masks: {label}"
        );
    }
}
