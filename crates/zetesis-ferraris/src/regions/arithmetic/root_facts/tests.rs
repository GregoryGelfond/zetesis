mod refusals;

use zetesis_algebra::Cancellation;

use crate::{
    AdmissionError, Limits, NodeView, Theory,
    regions::Work,
    test_support::{constructed_theory, push, world},
};

use super::super::{
    budget::{Budget, BudgetError},
    storage::Storage,
};
use super::{CheckedRootFacts, Step, build, replay};

fn budget(cancellation: &Cancellation) -> Budget<'_, 'static> {
    Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, cancellation)
}

fn chained() -> Theory {
    constructed_theory(5, |nodes| {
        let falsehood = push(nodes, NodeView::False);
        let truth = push(nodes, NodeView::Implies(falsehood, falsehood));
        let inner = push(nodes, NodeView::And(&[0, 1]));
        let body = push(nodes, NodeView::And(&[0, truth, inner]));
        let last = push(nodes, NodeView::Implies(2, 3));
        let middle = push(nodes, NodeView::Implies(body, 2));
        let first = push(nodes, NodeView::Implies(truth, 1));
        let negative = push(nodes, NodeView::Implies(4, falsehood));
        let choice = push(nodes, NodeView::Or(&[4, negative]));
        vec![last, middle, first, 0, choice]
    })
}

fn checked(theory: &Theory) -> CheckedRootFacts {
    build(
        theory,
        &mut Storage::default(),
        &mut budget(&Cancellation::default()),
    )
    .unwrap()
}

#[test]
fn nested_repeated_premises_reach_ordered_original_witnesses() {
    let owner = chained();
    let facts = checked(&owner);
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    assert_eq!(facts.words, [15]);
    assert_eq!(facts.steps.len(), 4);
    let mut prior = 0_u64;
    for step in &facts.steps {
        assert_eq!(prior & (1 << step.atom), 0);
        if step.atom == 2 {
            assert_eq!(prior & 3, 3);
            assert_eq!(step.root, 1);
        } else if step.atom == 3 {
            assert_ne!(prior & 4, 0);
            assert_eq!(step.root, 0);
        }
        prior |= 1 << step.atom;
    }
    for atom in 0..5 {
        assert_eq!(facts.contains(atom, &mut meter).unwrap(), atom < 4);
    }
    assert_eq!(
        facts.contains(5, &mut meter),
        Err(BudgetError::Source(AdmissionError::Atom))
    );
}

#[test]
fn facts_are_consequences_of_original_models() {
    let owner = chained();
    let facts = checked(&owner);
    let cancellation = Cancellation::default();
    let mut models = 0;
    for mask in 0..32 {
        let interpretation = world(&owner, mask);
        if crate::models(&owner, &interpretation, Limits::default(), &cancellation).unwrap() {
            models += 1;
            assert!(facts.steps.iter().all(|step| mask & (1 << step.atom) != 0));
        }
    }
    assert_eq!(models, 2);
}

#[test]
fn cycles_and_unsupported_sources_do_not_supply_facts() {
    let owner = constructed_theory(3, |nodes| {
        let falsehood = push(nodes, NodeView::False);
        let left = push(nodes, NodeView::Implies(0, 1));
        let right = push(nodes, NodeView::Implies(1, 0));
        let negative = push(nodes, NodeView::Implies(2, falsehood));
        let choice = push(nodes, NodeView::Or(&[2, negative]));
        let negative_body = push(nodes, NodeView::Implies(negative, 0));
        let disjunctive_body = push(nodes, NodeView::Implies(choice, 1));
        vec![left, right, choice, negative_body, disjunctive_body]
    });
    let facts = checked(&owner);
    assert!(facts.steps.is_empty());
    assert_eq!(facts.words, [0]);
}

#[test]
fn two_producers_publish_one_witness_for_the_same_head() {
    let owner = constructed_theory(2, |nodes| {
        let first = push(nodes, NodeView::Implies(0, 1));
        let repeated = push(nodes, NodeView::And(&[0, 0]));
        let second = push(nodes, NodeView::Implies(repeated, 1));
        vec![first, second, 0]
    });
    let facts = checked(&owner);
    assert_eq!(facts.words, [3]);
    assert_eq!(facts.steps.iter().filter(|step| step.atom == 1).count(), 1);
}

#[test]
fn checked_owner_accepts_clones_but_not_equal_foreign_theories() {
    let owner = chained();
    let facts = checked(&owner);
    let clone = owner.clone();
    let foreign = chained();
    assert!(facts.belongs_to(&clone));
    assert!(facts.owner().same_instance(&owner));
    assert!(!facts.belongs_to(&foreign));
    let cancellation = Cancellation::default();
    assert!(
        replay(
            &clone,
            &owner,
            &facts.steps,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_some()
    );
    assert!(
        replay(
            &foreign,
            &owner,
            &facts.steps,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn independent_replay_rejects_wrong_repeated_and_reordered_witnesses() {
    let owner = chained();
    let valid = [
        Step { root: 3, atom: 0 },
        Step { root: 2, atom: 1 },
        Step { root: 1, atom: 2 },
        Step { root: 0, atom: 3 },
    ];
    let cancellation = Cancellation::default();
    assert_eq!(
        replay(
            &owner,
            &owner,
            &valid,
            &mut Storage::default(),
            &mut budget(&cancellation)
        )
        .unwrap()
        .unwrap(),
        [15]
    );
    let proposals = [
        vec![Step { root: 3, atom: 1 }],
        vec![valid[0], valid[0]],
        vec![valid[2], valid[0], valid[1]],
        vec![Step { root: 4, atom: 4 }],
    ];
    for proposal in proposals {
        assert!(
            replay(
                &owner,
                &owner,
                &proposal,
                &mut Storage::default(),
                &mut budget(&cancellation)
            )
            .unwrap()
            .is_none(),
            "{proposal:?}"
        );
    }
    assert_eq!(
        replay(
            &owner,
            &owner,
            &[Step { root: 5, atom: 0 }],
            &mut Storage::default(),
            &mut budget(&cancellation)
        ),
        Err(BudgetError::Source(AdmissionError::Root))
    );
}

#[test]
fn final_mask_cannot_justify_an_unseeded_cycle() {
    let owner = constructed_theory(2, |nodes| {
        vec![
            push(nodes, NodeView::Implies(0, 1)),
            push(nodes, NodeView::Implies(1, 0)),
        ]
    });
    let cancellation = Cancellation::default();
    for proposal in [
        [Step { root: 0, atom: 1 }, Step { root: 1, atom: 0 }],
        [Step { root: 1, atom: 0 }, Step { root: 0, atom: 1 }],
    ] {
        assert!(
            replay(
                &owner,
                &owner,
                &proposal,
                &mut Storage::default(),
                &mut budget(&cancellation)
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn successful_storage_contains_only_the_checked_owner() {
    let owner = chained();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let facts = build(&owner, &mut storage, &mut meter).unwrap();
    assert_eq!(storage.bytes(), facts.retained_bytes());
    assert!(meter.peak_bytes() > storage.bytes());
    assert_eq!(meter.phases().total(), meter.work());
    assert!(meter.phases().recognition > 0);
    assert!(meter.phases().provenance > 0);
}
