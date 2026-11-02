use super::*;
use crate::{EvaluationLimits, EvaluationWorkspace, test_support::world};

fn nested_clauses(patterns: &[usize], wrappers: usize) -> Theory {
    constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        patterns
            .iter()
            .map(|&pattern| {
                let mut terms = [0; 3];
                for (atom, term) in terms.iter_mut().enumerate() {
                    *term = if pattern & (1 << atom) == 0 {
                        push(nodes, NodeView::Implies(atom, falsum))
                    } else {
                        atom
                    };
                    for _ in 0..wrappers {
                        *term = push(nodes, NodeView::Implies(verum, *term));
                    }
                }
                // Deliberately visit atoms out of their canonical order.
                let pair = push(nodes, NodeView::And(&[terms[0], terms[1]]));
                let body = push(nodes, NodeView::And(&[terms[2], pair]));
                push(nodes, NodeView::Implies(body, falsum))
            })
            .collect()
    })
}

#[test]
fn nested_wrapped_clauses_preserve_every_interpretation() {
    let cancellation = Cancellation::default();
    for wrappers in [0, 1, 3, 16] {
        let owner = nested_clauses(&(0..8).collect::<Vec<_>>(), wrappers);
        let mut evaluator = EvaluationWorkspace::default();
        for ordinal in 0..8 {
            let mut storage = Storage::default();
            let record = source::clause(&owner, ordinal, &mut storage, &mut budget(&cancellation))
                .unwrap()
                .unwrap();
            assert_eq!(record.root, ordinal);
            assert_eq!(record.support.len, 3);
            assert_eq!(record.support.atoms, [0, 1, 2, 0, 0, 0, 0, 0]);
            assert_eq!(usize::from(record.forbidden), ordinal);
            assert_eq!(storage.bytes(), 0);
            for pattern in 0..8 {
                let interpretation = world(&owner, pattern);
                let evaluated =
                    evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
                let original = evaluated
                    .result
                    .unwrap()
                    .node_truth(owner.roots()[ordinal])
                    .unwrap();
                assert_eq!(original, pattern != usize::from(record.forbidden));
            }
        }
    }
}

#[test]
fn nested_source_replay_authenticates_owner_and_equations() {
    let owner = nested_clauses(&[1, 2, 4, 7], 3);
    let foreign = nested_clauses(&[1, 2, 4, 7], 3);
    let cancellation = Cancellation::default();
    let records: Vec<_> = (0..owner.roots().len())
        .map(|ordinal| {
            source::clause(
                &owner,
                ordinal,
                &mut Storage::default(),
                &mut budget(&cancellation),
            )
            .unwrap()
            .unwrap()
        })
        .collect();
    let mut evaluator = EvaluationWorkspace::default();
    let allowed: Vec<_> = (0..8)
        .filter(|&pattern| {
            let interpretation = world(&owner, pattern);
            let evaluated =
                evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
            let truth = evaluated.result.unwrap();
            owner
                .roots()
                .iter()
                .all(|&root| truth.node_truth(root).unwrap())
        })
        .collect();
    for coefficients in 0_u64..8 {
        for constant in [false, true] {
            let expected = allowed.iter().all(|&pattern| {
                ((u64::try_from(pattern).unwrap() & coefficients).count_ones() % 2 != 0) == constant
            });
            let actual = source::check_clause_equation(
                &owner,
                &checked_facts(&owner.clone()),
                &records,
                records[0].support,
                (&[coefficients, 0], constant),
                &mut Storage::default(),
                &mut budget(&cancellation),
            )
            .unwrap();
            assert_eq!(
                actual, expected,
                "coefficients={coefficients}, constant={constant}"
            );
        }
    }
    assert!(
        !source::check_clause_equation(
            &owner,
            &checked_facts(&foreign),
            &records,
            records[0].support,
            (&[7, 0], false),
            &mut Storage::default(),
            &mut budget(&cancellation),
        )
        .unwrap()
    );
}

#[test]
fn repeated_atoms_across_nested_wrappers_decline() {
    for negative in [false, true] {
        let owner = constructed_theory(2, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let verum = push(nodes, NodeView::Implies(falsum, falsum));
            let repeated = if negative {
                push(nodes, NodeView::Implies(0, falsum))
            } else {
                0
            };
            let wrapped = push(nodes, NodeView::Implies(verum, repeated));
            let nested = push(nodes, NodeView::And(&[1, wrapped]));
            let body = push(nodes, NodeView::And(&[0, nested]));
            vec![push(nodes, NodeView::Implies(body, falsum))]
        });
        let cancellation = Cancellation::default();
        let mut storage = Storage::default();
        assert!(
            source::clause(&owner, 0, &mut storage, &mut budget(&cancellation))
                .unwrap()
                .is_none()
        );
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn noncanonical_truth_and_hidden_conjunctions_decline() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        let tautology = push(nodes, NodeView::Implies(0, 0));
        let other_truth = push(nodes, NodeView::Implies(falsum, 0));
        let wrong_wrapper = push(nodes, NodeView::Implies(tautology, 1));
        let pair = push(nodes, NodeView::And(&[0, 1]));
        let hidden = push(nodes, NodeView::Implies(verum, pair));
        let nested_truth = push(nodes, NodeView::And(&[0, verum]));
        [tautology, other_truth, wrong_wrapper, hidden, nested_truth]
            .into_iter()
            .map(|body| push(nodes, NodeView::Implies(body, falsum)))
            .collect()
    });
    let cancellation = Cancellation::default();
    for ordinal in 0..owner.roots().len() {
        assert!(
            source::clause(
                &owner,
                ordinal,
                &mut Storage::default(),
                &mut budget(&cancellation)
            )
            .unwrap()
            .is_none(),
            "root {ordinal}"
        );
    }
}

#[test]
fn canonical_true_constraint_forbids_the_empty_pattern() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        vec![push(nodes, NodeView::Implies(verum, falsum))]
    });
    let cancellation = Cancellation::default();
    let record = source::clause(
        &owner,
        0,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(record.support.len, 0);
    assert_eq!(record.support.atoms, [0; 8]);
    assert_eq!(record.forbidden, 0);
    for pattern in 0..4 {
        let interpretation = world(&owner, pattern);
        let mut evaluator = EvaluationWorkspace::default();
        let evaluated =
            evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
        assert!(
            !evaluated
                .result
                .unwrap()
                .node_truth(owner.roots()[0])
                .unwrap()
        );
    }
    assert!(
        source::check_clause_equation(
            &owner,
            &checked_facts(&owner),
            &[record],
            record.support,
            (&[0, 0], true),
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
    );
}

#[test]
fn nested_clause_decoding_and_replay_refuse_every_incomplete_prefix() {
    let owner = nested_clauses(&[1, 2, 4, 7], 2);
    let cancellation = Cancellation::default();
    let records: Vec<_> = (0..owner.roots().len())
        .map(|ordinal| {
            source::clause(
                &owner,
                ordinal,
                &mut Storage::default(),
                &mut budget(&cancellation),
            )
            .unwrap()
            .unwrap()
        })
        .collect();
    for replay in [false, true] {
        let run = |meter: &mut Budget<'_, '_>| {
            let mut storage = Storage::default();
            if replay {
                source::check_clause_equation(
                    &owner,
                    &checked_facts(&owner),
                    &records,
                    records[0].support,
                    (&[7, 0], false),
                    &mut storage,
                    meter,
                )
            } else {
                source::clause(&owner, 0, &mut storage, meter).map(|record| record.is_some())
            }
        };
        let mut full = budget(&cancellation);
        assert_eq!(run(&mut full), Ok(true));
        for maximum in 0..full.work() {
            let mut limited = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
            assert_eq!(
                run(&mut limited),
                Err(BudgetError::WorkDeclined),
                "replay={replay}, work={maximum}"
            );
            assert!(limited.work() <= maximum);
        }
        for maximum in 0..usize::try_from(full.peak_bytes()).unwrap() {
            let mut limited = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
            assert!(
                matches!(run(&mut limited), Err(BudgetError::StorageDeclined {required,limit}) if limit == maximum && required > maximum as u128),
                "replay={replay}, bytes={maximum}"
            );
        }
    }
}
