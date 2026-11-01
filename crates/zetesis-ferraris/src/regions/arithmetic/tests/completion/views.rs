use super::*;

fn canonical_fact() -> Theory {
    constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        vec![push(nodes, NodeView::Implies(verum, 1))]
    })
}

#[test]
fn canonical_true_and_bare_facts_have_identical_completion() {
    let bare = theory(2, vec![Node::atom(0), Node::atom(1)], vec![1]);
    let lowered = canonical_fact();
    let cancellation = Cancellation::default();
    let mut records = Vec::new();
    for (owner, has_body) in [(&bare, false), (&lowered, true)] {
        let producers = inventory(owner);
        assert_eq!(producers.by_head[1].len(), 1);
        let producer = &producers.rules[producers.by_head[1][0]];
        assert_eq!(producer.body.is_some(), has_body);
        let record = completion(owner, &producers, 1, 0, &mut budget(&cancellation))
            .unwrap()
            .unwrap();
        // The fact body contributes no atom. Only its completion head remains.
        assert_eq!(record.support.len, 1);
        assert_eq!(record.support.atoms, [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&*record.forbidden, [1, 0, 0, 0]);
        assert_matches_original_bodies(owner, &producers, &record);
        records.push(record);
    }
    assert_eq!(records[0], records[1]);
}

#[test]
fn a_true_body_does_not_skip_later_normal_producers() {
    let owner = constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let verum = push(nodes, NodeView::Implies(falsum, falsum));
        let fact = push(nodes, NodeView::Implies(verum, 2));
        let not_first = push(nodes, NodeView::Implies(0, falsum));
        let body = push(nodes, NodeView::And(&[not_first, 1]));
        vec![fact, push(nodes, NodeView::Implies(body, 2))]
    });
    let producers = inventory(&owner);
    assert_eq!(producers.by_head[2].len(), 2);
    let cancellation = Cancellation::default();
    let record = completion(&owner, &producers, 2, 0, &mut budget(&cancellation))
        .unwrap()
        .unwrap();
    assert_eq!(record.support.len, 3);
    assert_eq!(record.support.atoms, [0, 1, 2, 0, 0, 0, 0, 0]);
    assert_eq!(&*record.forbidden, [0b1111, 0, 0, 0]);
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn a_true_body_does_not_hide_ineligible_later_producers() {
    for kind in 0..4 {
        let owner = constructed_theory(10, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let verum = push(nodes, NodeView::Implies(falsum, falsum));
            let fact = push(nodes, NodeView::Implies(verum, 9));
            let later = match kind {
                0 => {
                    let not_head = push(nodes, NodeView::Implies(9, falsum));
                    push(nodes, NodeView::Or(&[9, not_head]))
                }
                1 => push(nodes, NodeView::Or(&[9, 0])),
                2 => {
                    let body = push(nodes, NodeView::Or(&[0, 1]));
                    push(nodes, NodeView::Implies(body, 9))
                }
                _ => {
                    let body = push(nodes, NodeView::And(&[0, 1, 2, 3, 4, 5, 6, 7, 8]));
                    push(nodes, NodeView::Implies(body, 9))
                }
            };
            vec![fact, later]
        });
        let producers = inventory(&owner);
        assert_eq!(producers.by_head[9].len(), 2, "kind {kind}");
        let cancellation = Cancellation::default();
        assert!(
            completion(&owner, &producers, 9, 0, &mut budget(&cancellation))
                .unwrap()
                .is_none(),
            "kind {kind}"
        );
    }
}

#[test]
fn only_whole_body_canonical_truth_is_an_empty_conjunction() {
    for kind in 0..4 {
        let owner = constructed_theory(2, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let verum = push(nodes, NodeView::Implies(falsum, falsum));
            let body = match kind {
                0 => push(nodes, NodeView::Implies(0, 0)),
                1 => push(nodes, NodeView::Implies(falsum, 0)),
                2 => push(nodes, NodeView::And(&[0, verum])),
                _ => push(nodes, NodeView::Implies(verum, 0)),
            };
            vec![push(nodes, NodeView::Implies(body, 1))]
        });
        let producers = inventory(&owner);
        assert_eq!(producers.by_head[1].len(), 1);
        let cancellation = Cancellation::default();
        assert!(
            completion(&owner, &producers, 1, 0, &mut budget(&cancellation))
                .unwrap()
                .is_none(),
            "kind {kind}"
        );
    }
}

#[test]
fn canonical_fact_replay_requires_the_original_owner() {
    let owner = canonical_fact();
    let equal_foreign = canonical_fact();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert!(
        completion(&equal_foreign, &producers, 1, 0, &mut budget(&cancellation))
            .unwrap()
            .is_none()
    );
    let record = completion(&owner.clone(), &producers, 1, 0, &mut budget(&cancellation))
        .unwrap()
        .unwrap();
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn canonical_fact_decoding_preserves_every_refusal_prefix() {
    let owner = canonical_fact();
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let expected = completion(&owner, &producers, 1, 23, &mut full)
        .unwrap()
        .unwrap();
    for maximum in 0..full.work() {
        let mut limited = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        assert_eq!(
            completion(&owner, &producers, 1, 23, &mut limited),
            Err(BudgetError::WorkDeclined)
        );
        assert!(limited.work() <= maximum);
    }
    for maximum in 0..usize::try_from(full.peak_bytes()).unwrap() {
        let mut limited = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
        assert!(
            matches!(completion(&owner, &producers, 1, 23, &mut limited), Err(BudgetError::StorageDeclined {required,limit}) if limit == maximum && required > maximum as u128)
        );
    }
    for failure in [Stop::Cancelled, Stop::WorkLimit, Stop::Allocation] {
        for remaining in 0..full.work() {
            let mut allowance = Allowance { remaining, failure };
            let mut limited = Budget::new(
                Work::reserved(&mut allowance),
                u64::MAX,
                usize::MAX,
                &cancellation,
            );
            assert_eq!(
                completion(&owner, &producers, 1, 23, &mut limited),
                Err(BudgetError::Stopped(failure))
            );
            assert!(limited.work() <= remaining);
            limited.finish();
        }
    }
    assert_eq!(
        completion(&owner, &producers, 1, 23, &mut budget(&cancellation)),
        Ok(Some(expected))
    );
}
