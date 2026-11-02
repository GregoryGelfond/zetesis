use super::*;
use crate::{EvaluationLimits, EvaluationWorkspace, test_support::world};

fn supported_constraint() -> Theory {
    constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        let fact = push(nodes, NodeView::Implies(verum, 0));
        let rule = push(nodes, NodeView::Implies(0, 1));
        let negation = push(nodes, NodeView::Implies(2, falsum));
        let pair = push(nodes, NodeView::And(&[0, negation]));
        let body = push(nodes, NodeView::And(&[1, pair]));
        vec![fact, rule, push(nodes, NodeView::Implies(body, falsum))]
    })
}

#[test]
fn checked_positive_facts_leave_only_unknown_coordinates() {
    let owner = supported_constraint();
    let facts = checked_facts(&owner);
    let cancellation = Cancellation::default();
    assert!(facts.contains(0, &mut budget(&cancellation)).unwrap());
    assert!(facts.contains(1, &mut budget(&cancellation)).unwrap());
    assert!(!facts.contains(2, &mut budget(&cancellation)).unwrap());
    let raw = source::clause(
        &owner,
        2,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(raw.support.len, 3);
    let conditioned = source::conditioned_clause(
        &owner,
        &facts,
        2,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(conditioned.root, raw.root);
    assert_eq!(conditioned.support.len, 1);
    assert_eq!(conditioned.support.atoms, [2, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(conditioned.forbidden, 0);
    let mut evaluator = EvaluationWorkspace::default();
    for assignment in [3, 7] {
        let interpretation = world(&owner, assignment);
        let evaluated =
            evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
        let original = evaluated
            .result
            .unwrap()
            .node_truth(owner.roots()[2])
            .unwrap();
        assert_eq!(original, assignment & 4 != 0);
        assert_eq!(
            original,
            usize::from(conditioned.forbidden) != (assignment >> 2)
        );
    }
    assert!(
        source::check_clause_equation(
            &owner,
            &facts,
            &[conditioned],
            conditioned.support,
            (&[4, 0], true),
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
    );
    assert!(
        !source::check_clause_equation(
            &owner,
            &facts,
            &[conditioned],
            conditioned.support,
            (&[4, 0], false),
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
    );
}

#[test]
fn a_negated_checked_fact_makes_the_clause_vacuous() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negative = push(nodes, NodeView::Implies(0, falsum));
        let body = push(nodes, NodeView::And(&[1, negative]));
        vec![0, push(nodes, NodeView::Implies(body, falsum))]
    });
    let facts = checked_facts(&owner);
    let cancellation = Cancellation::default();
    assert!(
        source::clause(
            &owner,
            1,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_some()
    );
    assert!(
        source::conditioned_clause(
            &owner,
            &facts,
            1,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_none()
    );
    for assignment in [1, 3] {
        let interpretation = world(&owner, assignment);
        let mut workspace = EvaluationWorkspace::default();
        let evaluated =
            workspace.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
        assert!(
            evaluated
                .result
                .unwrap()
                .node_truth(owner.roots()[1])
                .unwrap()
        );
    }
}

#[test]
fn conditioning_preserves_raw_duplicate_and_support_limits() {
    for (atoms, repeated) in [(1, true), (9, false)] {
        let owner = constructed_theory(atoms, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let leaves: Vec<_> = if repeated {
                vec![0, 0]
            } else {
                (0..atoms).collect()
            };
            let body = push(nodes, NodeView::And(&leaves));
            let mut roots: Vec<_> = (0..atoms).collect();
            roots.push(push(nodes, NodeView::Implies(body, falsum)));
            roots
        });
        let facts = checked_facts(&owner);
        let cancellation = Cancellation::default();
        for atom in 0..atoms {
            assert!(facts.contains(atom, &mut budget(&cancellation)).unwrap());
        }
        // Conditioning first would erase every literal and incorrectly admit
        // a source rejected by the unchanged raw grammar.
        assert!(
            source::clause(
                &owner,
                atoms,
                &mut Storage::default(),
                &mut budget(&cancellation)
            )
            .unwrap()
            .is_none()
        );
        assert!(
            source::conditioned_clause(
                &owner,
                &facts,
                atoms,
                &mut Storage::default(),
                &mut budget(&cancellation)
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn eight_known_atoms_retain_an_empty_refuting_clause() {
    let owner = constructed_theory(8, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let body = push(nodes, NodeView::And(&[0, 1, 2, 3, 4, 5, 6, 7]));
        let mut roots: Vec<_> = (0..8).collect();
        roots.push(push(nodes, NodeView::Implies(body, falsum)));
        roots
    });
    let facts = checked_facts(&owner);
    let cancellation = Cancellation::default();
    let raw = source::clause(
        &owner,
        8,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(raw.support.len, 8);
    let conditioned = source::conditioned_clause(
        &owner,
        &facts,
        8,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(conditioned.support.len, 0);
    assert_eq!(conditioned.support.atoms, [0; 8]);
    assert_eq!(conditioned.forbidden, 0);
    assert!(
        source::check_clause_equation(
            &owner,
            &facts,
            &[conditioned],
            conditioned.support,
            (&[0, 0], true),
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
    );
}

#[test]
fn conditioning_rejects_equal_foreign_fact_owners() {
    let owner = supported_constraint();
    let foreign = supported_constraint();
    let facts = checked_facts(&owner);
    let foreign_facts = checked_facts(&foreign);
    let cancellation = Cancellation::default();
    let record = source::conditioned_clause(
        &owner.clone(),
        &facts,
        2,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    assert!(
        source::conditioned_clause(
            &owner,
            &foreign_facts,
            2,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_none()
    );
    assert!(
        !source::check_clause_equation(
            &owner,
            &foreign_facts,
            &[record],
            record.support,
            (&[4, 0], true),
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
    );
}

#[test]
fn conditioned_source_and_replay_preserve_refusal_prefixes() {
    let owner = supported_constraint();
    let facts = checked_facts(&owner);
    let cancellation = Cancellation::default();
    let record = source::conditioned_clause(
        &owner,
        &facts,
        2,
        &mut Storage::default(),
        &mut budget(&cancellation),
    )
    .unwrap()
    .unwrap();
    for replay in [false, true] {
        let run = |meter: &mut Budget<'_, '_>| {
            meter.admit_bytes(facts.retained_bytes())?;
            let mut storage = Storage::with_live_bytes(facts.retained_bytes());
            if replay {
                source::check_clause_equation(
                    &owner,
                    &facts,
                    &[record],
                    record.support,
                    (&[4, 0], true),
                    &mut storage,
                    meter,
                )
            } else {
                source::conditioned_clause(&owner, &facts, 2, &mut storage, meter)
                    .map(|value| value.is_some())
            }
        };
        let mut full = budget(&cancellation);
        assert_eq!(run(&mut full), Ok(true));
        for maximum in 0..full.work() {
            let mut limited = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
            assert_eq!(run(&mut limited), Err(BudgetError::WorkDeclined));
            assert!(limited.work() <= maximum);
        }
        for maximum in 0..usize::try_from(full.peak_bytes()).unwrap() {
            let mut limited = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
            assert!(
                matches!(run(&mut limited),Err(BudgetError::StorageDeclined {required,limit}) if limit==maximum && required>maximum as u128)
            );
        }
    }
}
