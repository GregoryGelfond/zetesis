use zetesis_algebra::Meter;

use super::*;

/// Each authored body selects exactly one local pattern. Repeated literals and
/// nested conjunctions exercise the occurrence reader, not a prepacked input.
fn pattern_definitions(atoms: usize, head: usize, patterns: &[u64]) -> Theory {
    constructed_theory(atoms, |nodes| {
        let falsum = push(nodes, NodeView::False);
        patterns
            .iter()
            .map(|pattern| {
                let literals: Vec<_> = (0..atoms)
                    .map(|atom| {
                        if pattern & (1 << atom) == 0 {
                            push(nodes, NodeView::Implies(atom, falsum))
                        } else {
                            atom
                        }
                    })
                    .collect();
                let repeated = push(nodes, NodeView::And(&[literals[0], literals[0]]));
                let body = push(nodes, NodeView::And(&literals));
                let nested = push(nodes, NodeView::And(&[repeated, body]));
                push(nodes, NodeView::Implies(nested, head))
            })
            .collect()
    })
}

#[test]
fn every_column_matches_the_original_body_truth() {
    let cancellation = Cancellation::default();
    for atoms in 1..=8 {
        let last = (1_u64 << atoms) - 1;
        for head in 0..atoms {
            for pattern in [0, last, 0x55 & last, 0xaa & last] {
                let owner = pattern_definitions(atoms, head, &[pattern]);
                let producers = inventory(&owner);
                let record = completion(
                    &owner,
                    &producers,
                    head,
                    0,
                    &mut budget(&mut Meter::local(u64::MAX), &cancellation),
                )
                .unwrap()
                .unwrap();
                assert_eq!(record.support.len, atoms);
                assert_eq!(
                    record.support.atoms[..atoms],
                    (0..atoms).collect::<Vec<_>>()
                );
                assert_matches_original_bodies(&owner, &producers, &record);
            }
        }
    }
}

#[test]
fn producer_union_matches_truth_across_word_boundaries() {
    let owner = pattern_definitions(8, 7, &[0, 63, 64, 127, 128, 191, 192, 255]);
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    assert_eq!(producers.by_head[7].len(), 8);
    let record = completion(
        &owner,
        &producers,
        7,
        0,
        &mut budget(&mut Meter::local(u64::MAX), &cancellation),
    )
    .unwrap()
    .unwrap();
    assert_eq!(record.support.len, 8);
    assert_matches_original_bodies(&owner, &producers, &record);
}

#[test]
fn full_width_tables_preserve_every_optional_refusal() {
    let owner = pattern_definitions(8, 7, &[0x55, 0xaa]);
    let producers = inventory(&owner);
    let cancellation = Cancellation::default();
    let live_bytes = 29;
    let mut complete_work = Meter::local(u64::MAX);
    let mut complete = budget(&mut complete_work, &cancellation);
    let expected = completion(&owner, &producers, 7, live_bytes, &mut complete)
        .unwrap()
        .unwrap();
    assert_matches_original_bodies(&owner, &producers, &expected);
    for maximum in 0..complete.work() {
        let mut limited_work = Meter::local(u64::MAX);
        let mut limited = Budget::new(&mut limited_work, maximum, usize::MAX, &cancellation);
        assert_eq!(
            completion(&owner, &producers, 7, live_bytes, &mut limited),
            Err(BudgetError::WorkDeclined),
            "work allowance {maximum}"
        );
        assert!(limited.work() <= maximum);
    }
    for maximum in 0..usize::try_from(complete.peak_bytes()).unwrap() {
        let mut limited_work = Meter::local(u64::MAX);
        let mut limited = Budget::new(&mut limited_work, u64::MAX, maximum, &cancellation);
        assert!(
            matches!(completion(&owner, &producers, 7, live_bytes, &mut limited),
                Err(BudgetError::StorageDeclined { required, limit })
                    if limit == maximum && required > maximum as u128),
            "byte allowance {maximum}"
        );
    }
    assert_eq!(
        completion(
            &owner,
            &producers,
            7,
            live_bytes,
            &mut budget(&mut Meter::local(u64::MAX), &cancellation)
        ),
        Ok(Some(expected))
    );
}
