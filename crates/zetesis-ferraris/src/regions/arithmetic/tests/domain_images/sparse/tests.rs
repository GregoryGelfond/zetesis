use super::super::{contains as original_contains, fixture, scalar};
use super::*;
use crate::{EvaluationLimits, EvaluationWorkspace, test_support::world};

fn finish(owner: &Theory, input: &Region) -> Attempt {
    run(
        owner,
        owner,
        input,
        Limits::default(),
        &Cancellation::default(),
    )
}

fn exact_account(attempt: &Attempt) {
    assert_eq!(
        attempt.work,
        attempt.preparation + attempt.copying + attempt.closure
    );
}

fn models(owner: &Theory) -> Vec<usize> {
    let mut evaluation = EvaluationWorkspace::default();
    (0..1 << owner.atom_count())
        .filter(|&mask| {
            evaluation
                .evaluate(
                    &world(owner, mask),
                    EvaluationLimits::default(),
                    &Cancellation::default(),
                )
                .result
                .unwrap()
                .is_model()
        })
        .collect()
}

#[test]
fn sparse_pivots_match_every_small_original_image_closure() {
    let groups = [vec![0, 1], vec![2, 3]];
    let possible = [(0, 2), (0, 3), (1, 2), (1, 3)];
    for relation in 0..16 {
        let pairs: Vec<_> = possible
            .iter()
            .enumerate()
            .filter_map(|(index, &pair)| (relation & (1 << index) != 0).then_some(pair))
            .collect();
        let owner = fixture(4, &groups, &pairs);
        let models = models(&owner);
        for mut code in 0..81 {
            let mut input = Region::all_open(4);
            for atom in 0..4 {
                match code % 3 {
                    1 => assert!(input.hold(atom)),
                    2 => assert!(input.cut(atom)),
                    _ => (),
                }
                code /= 3;
            }
            let before = input.clone();
            let attempt = finish(&owner, &input);
            exact_account(&attempt);
            assert_eq!(
                attempt.copying,
                11 + 6 * input.mask_words() as u64 + 17 * input.decided().count() as u64
            );
            let (actual, outcome) = attempt.result.unwrap().unwrap();
            let mut reference = input.clone();
            let expected = scalar(&groups, &pairs, &mut reference);
            assert_eq!(outcome, expected);
            if outcome != Narrowing::Refuted {
                assert_eq!(actual, reference);
            }
            assert_eq!(input, before, "the caller's Region is never mutated");
            for &model in &models {
                if original_contains(&input, model) {
                    assert_ne!(outcome, Narrowing::Refuted);
                    assert!(original_contains(&actual, model));
                }
            }
        }
    }
}

#[test]
fn sparse_images_keep_held_pivots_duplicates_and_padding_sound() {
    let groups = [vec![0, 64, 129]];
    let pairs = [(0, 128), (64, 128), (129, 128), (0, 128)];
    let owner = fixture(130, &groups, &pairs);
    let mut input = Region::all_open(130);
    assert!(input.hold(0));
    let attempt = finish(&owner, &input);
    exact_account(&attempt);
    let (actual, outcome) = attempt.result.unwrap().unwrap();
    assert_eq!(outcome, Narrowing::Fixed { changed: true });
    assert!(actual.is_held(0) && actual.is_cut(128));
    assert_eq!(actual.open().count(), 128);
    let (_, cut) = actual.decision_words();
    assert_eq!(cut[2] & !3, 0);
    let without_duplicate = fixture(130, &groups, &pairs[..3]);
    let clean = finish(&without_duplicate, &input);
    assert!(
        attempt.work > clean.work,
        "duplicate occurrences remain visited and charged"
    );
    assert_eq!(actual, clean.result.unwrap().unwrap().0);
}

#[test]
fn sparse_images_handle_overlaps_zero_degree_and_missing_sources() {
    let groups = [vec![0, 1], vec![1, 2]];
    let pairs = [(0, 3), (1, 3)];
    let owner = fixture(4, &groups, &pairs);
    let input = Region::all_open(4);
    let (actual, _) = finish(&owner, &input).result.unwrap().unwrap();
    assert!(actual.is_cut(3));
    assert!(
        models(&owner)
            .iter()
            .all(|&model| original_contains(&actual, model))
    );
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2)]);
    let mut input = Region::all_open(3);
    assert!(input.cut(0) && input.hold(1));
    let (actual, outcome) = finish(&owner, &input).result.unwrap().unwrap();
    assert_eq!(outcome, Narrowing::Fixed { changed: false });
    assert_eq!(actual, input);
    let owner = fixture(2, &[vec![0, 1]], &[]);
    let mut input = Region::all_open(2);
    assert!(input.cut(0) && input.cut(1));
    assert_eq!(
        finish(&owner, &input).result.unwrap().unwrap().1,
        Narrowing::Refuted
    );
}

#[test]
fn sparse_refusals_publish_no_root_and_conserve_each_work_prefix() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let input = Region::all_open(3);
    let before = input.clone();
    let complete = finish(&owner, &input);
    exact_account(&complete);
    assert!(complete.copying > 0 && complete.closure > 0);
    for limit in 0..complete.work {
        let attempt = run(
            &owner,
            &owner,
            &input,
            Limits {
                work: limit,
                ..Limits::default()
            },
            &Cancellation::default(),
        );
        exact_account(&attempt);
        assert!(matches!(attempt.result, Err(BudgetError::WorkDeclined)));
        assert!(attempt.work <= limit);
        assert_eq!(input, before);
    }
    let exact = run(
        &owner,
        &owner,
        &input,
        Limits {
            work: complete.work,
            bytes: usize::try_from(complete.peak).unwrap(),
            ..Limits::default()
        },
        &Cancellation::default(),
    );
    exact_account(&exact);
    assert_eq!(exact.result, complete.result);
    assert_eq!(exact.work, complete.work);
    assert_eq!(exact.peak, complete.peak);
    let global = run(
        &owner,
        &owner,
        &input,
        Limits {
            global_work: 0,
            ..Limits::default()
        },
        &Cancellation::default(),
    );
    assert!(matches!(
        global.result,
        Err(BudgetError::Stopped(Stop::WorkLimit))
    ));
    assert_eq!(global.work, 0);
}

#[test]
fn sparse_storage_decline_and_cancellation_keep_exact_boundaries() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let input = Region::all_open(3);
    let complete = finish(&owner, &input);
    for bytes in [0, input.retained_bytes() - 1, complete.peak - 1] {
        let attempt = run(
            &owner,
            &owner,
            &input,
            Limits {
                bytes: usize::try_from(bytes).unwrap(),
                ..Limits::default()
            },
            &Cancellation::default(),
        );
        exact_account(&attempt);
        assert!(matches!(
            attempt.result,
            Err(BudgetError::StorageDeclined { .. })
        ));
        assert!(attempt.peak > bytes);
    }
    let control = Cancellation::default();
    control.cancel();
    let stopped = run(
        &owner,
        &owner,
        &input,
        Limits {
            global_work: 0,
            work: 0,
            bytes: 0,
        },
        &control,
    );
    assert_eq!(stopped.result, Err(BudgetError::Stopped(Stop::Cancelled)));
    assert_eq!((stopped.work, stopped.peak), (0, 0));
}

#[test]
fn sparse_images_decline_absent_groups_and_reject_foreign_owners() {
    for atoms in [0, 3] {
        let owner = fixture(atoms, &[], &[]);
        let result = finish(&owner, &Region::all_open(atoms));
        exact_account(&result);
        assert!(result.result.unwrap().is_none());
        assert_eq!((result.copying, result.closure), (0, 0));
    }
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let other = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let foreign = run(
        &owner,
        &other,
        &Region::all_open(3),
        Limits::default(),
        &Cancellation::default(),
    );
    assert_eq!(
        foreign.result,
        Err(BudgetError::Stopped(Stop::InvalidProgram))
    );
}

#[test]
fn sparse_images_close_a_mandatory_chain_and_refute_a_short_domain() {
    for values in [2, 3] {
        let groups: Vec<Vec<usize>> = (0..3)
            .map(|position| (position * values..(position + 1) * values).collect())
            .collect();
        let mut pairs = Vec::new();
        for position in 0..2 {
            for left in 0..values {
                for right in 0..=left {
                    pairs.push((position * values + left, (position + 1) * values + right));
                }
            }
        }
        let owner = fixture(3 * values, &groups, &pairs);
        let admitted_models = models(&owner);
        let input = Region::all_open(3 * values);
        let attempt = finish(&owner, &input);
        exact_account(&attempt);
        let (actual, outcome) = attempt.result.unwrap().unwrap();
        let mut reference = input.clone();
        assert_eq!(outcome, scalar(&groups, &pairs, &mut reference));
        if values == 2 {
            assert!(admitted_models.is_empty());
            assert_eq!(outcome, Narrowing::Refuted);
        } else {
            let unique = (1 << 0) | (1 << 4) | (1 << 8);
            assert_eq!(admitted_models, [unique]);
            assert_eq!(outcome, Narrowing::Fixed { changed: true });
            assert_eq!(actual, reference);
            assert!(original_contains(&actual, unique));
            for (position, group) in groups.iter().enumerate() {
                let possible: Vec<_> = group
                    .iter()
                    .copied()
                    .filter(|&atom| !actual.is_cut(atom))
                    .collect();
                assert_eq!(possible, [position * values + position]);
            }
            // Image exclusion leaves the unique possible value in each group;
            // the ordinary Boolean subject remains responsible for holding it.
            assert_eq!(actual.open().collect::<Vec<_>>(), [0, 4, 8]);
        }
    }
}
