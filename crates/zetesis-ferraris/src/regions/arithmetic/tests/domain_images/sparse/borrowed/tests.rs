use super::super::super::{contains, fixture};
use super::*;
use crate::{EvaluationLimits, EvaluationWorkspace, PreparationLimits, test_support::world};

fn prepared(owner: &Theory) -> RootPreparation {
    let mut meter = Meter::local(100_000_000);
    let attempt = RootPreparation::prepare_with_image_policy(
        owner,
        None,
        PreparationLimits {
            work: PreparationLimits::DEFAULT_WORK,
            bytes: 16 * 1024 * 1024,
        },
        &Cancellation::default(),
        &mut meter,
        crate::regions::arithmetic::images::Policy {
            enabled: false,
            retain_on_decline: false,
        },
    );
    let mut prepared = attempt.result.unwrap().unwrap();
    drop(prepared.take_seeded_root());
    prepared
}

fn finish(owner: &Theory, prepared: &RootPreparation, input: &Region) -> Attempt {
    run(
        owner,
        prepared,
        input,
        Limits::default(),
        &Cancellation::default(),
    )
}

fn exact(attempt: &Attempt) {
    assert_eq!(
        attempt.work,
        attempt.preparation + attempt.copying + attempt.closure
    );
}

#[test]
fn borrowed_bounds_preserve_each_small_original_model() {
    let groups = [vec![0, 1], vec![2, 3]];
    let possibilities = [(0, 2), (0, 3), (1, 2), (1, 3)];
    let mut checked = 0;
    for relation in 0..16 {
        let pairs: Vec<_> = possibilities
            .iter()
            .enumerate()
            .filter_map(|(index, &pair)| (relation & (1 << index) != 0).then_some(pair))
            .collect();
        let owner = fixture(4, &groups, &pairs);
        let prepared = prepared(&owner);
        if prepared.root_refuted() {
            continue;
        }
        let mut workspace = EvaluationWorkspace::default();
        let models: Vec<_> = (0..16)
            .filter(|&model| {
                workspace
                    .evaluate(
                        &world(&owner, model),
                        EvaluationLimits::default(),
                        &Cancellation::default(),
                    )
                    .result
                    .unwrap()
                    .is_model()
            })
            .collect();
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
            let actual = finish(&owner, &prepared, &input);
            exact(&actual);
            let old = super::super::run(
                &owner,
                &owner,
                &input,
                Limits::default(),
                &Cancellation::default(),
            );
            let (actual, result) = actual.result.unwrap().unwrap();
            let (old, old_result) = old.result.unwrap().unwrap();
            assert_eq!(result, old_result);
            if result != super::super::Narrowing::Refuted {
                assert_eq!(actual, old);
            }
            for &model in &models {
                if contains(&input, model) {
                    assert_ne!(result, super::super::Narrowing::Refuted);
                    assert!(contains(&actual, model));
                }
            }
            assert_eq!(input, before);
            checked += 1;
        }
    }
    assert!(checked > 0);
    println!("borrowed_bound_small_closures\tchecked={checked}");
}

#[test]
fn borrowed_bounds_preserve_every_optional_prefix_and_live_owner() {
    let owner = fixture(3, &[vec![0, 1]], &[(0, 2), (1, 2)]);
    let prepared = prepared(&owner);
    let input = Region::all_open(3);
    let before = input.clone();
    let complete = finish(&owner, &prepared, &input);
    exact(&complete);
    assert!(complete.result.as_ref().unwrap().is_some());
    assert!(complete.peak >= retained(&prepared) + input.retained_bytes());
    for work in 0..complete.work {
        let attempt = run(
            &owner,
            &prepared,
            &input,
            Limits {
                work,
                ..Limits::default()
            },
            &Cancellation::default(),
        );
        exact(&attempt);
        assert!(matches!(attempt.result, Err(BudgetError::WorkDeclined)));
        assert!(attempt.work <= work);
        assert_eq!(input, before);
        assert!(prepared.belongs_to(&owner));
    }
    let exact_limit = Limits {
        work: complete.work,
        bytes: usize::try_from(complete.peak).unwrap(),
        ..Limits::default()
    };
    let admitted = run(
        &owner,
        &prepared,
        &input,
        exact_limit,
        &Cancellation::default(),
    );
    assert_eq!(admitted.result, complete.result);
    assert_eq!(
        (admitted.work, admitted.peak),
        (complete.work, complete.peak)
    );
    let refused = run(
        &owner,
        &prepared,
        &input,
        Limits {
            bytes: exact_limit.bytes - 1,
            ..exact_limit
        },
        &Cancellation::default(),
    );
    assert!(matches!(
        refused.result,
        Err(BudgetError::StorageDeclined { .. })
    ));
    exact(&refused);
    assert_eq!(input, before);
}

#[test]
fn borrowed_bounds_decline_absence_and_reject_unowned_state() {
    let owner = fixture(3, &[], &[]);
    let prepared = prepared(&owner);
    let input = Region::all_open(3);
    let absent = finish(&owner, &prepared, &input);
    exact(&absent);
    assert!(absent.result.unwrap().is_none());
    assert_eq!((absent.copying, absent.closure), (0, 0));
    let foreign = fixture(3, &[], &[]);
    let wrong = finish(&foreign, &prepared, &input);
    assert_eq!(
        wrong.result,
        Err(BudgetError::Stopped(Stop::InvalidProgram))
    );
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let stopped = run(
        &owner,
        &prepared,
        &input,
        Limits {
            global_work: 0,
            work: 0,
            bytes: 0,
        },
        &cancellation,
    );
    assert_eq!(stopped.result, Err(BudgetError::Stopped(Stop::Cancelled)));
    assert_eq!((stopped.work, stopped.peak), (0, 0));
}

#[test]
fn borrowed_bounds_do_not_promote_guards_or_upper_rows_to_mandatory() {
    use super::super::super::{NodeView, constructed_theory, pair_constraint, push, threshold};
    for guarded in [false, true] {
        let owner = constructed_theory(4, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let row = threshold(nodes, &[0, 1], if guarded { 1 } else { 2 });
            let negative = push(nodes, NodeView::Implies(row, falsum));
            let root = if guarded {
                let body = push(nodes, NodeView::And(&[3, negative]));
                push(nodes, NodeView::Implies(body, falsum))
            } else {
                negative
            };
            vec![
                root,
                pair_constraint(nodes, falsum, (0, 2)),
                pair_constraint(nodes, falsum, (1, 2)),
            ]
        });
        let prepared = prepared(&owner);
        let mut input = Region::all_open(4);
        assert!(input.cut(0) && input.cut(1) && input.cut(3) && input.hold(2));
        let attempt = finish(&owner, &prepared, &input);
        exact(&attempt);
        assert!(attempt.result.unwrap().is_none());
        assert_eq!((attempt.copying, attempt.closure), (0, 0));
        let mut workspace = EvaluationWorkspace::default();
        assert!(
            workspace
                .evaluate(
                    &world(&owner, 1 << 2),
                    EvaluationLimits::default(),
                    &Cancellation::default()
                )
                .result
                .unwrap()
                .is_model()
        );
    }
}
