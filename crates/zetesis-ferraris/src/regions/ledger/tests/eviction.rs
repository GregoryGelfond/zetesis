//! A full tier gives up the row with the fewest decayed hits, the oldest
//! among equals, and never a founded row its floor protects.

use super::*;

fn limits(tier_rows: usize, founded_floor: usize) -> LearningLimits {
    LearningLimits {
        tier_rows,
        founded_floor,
        ..LearningLimits::default()
    }
}

/// Three rows with distinct conditions and one shared output.
fn full(limits: LearningLimits) -> Ledger {
    let mut ledger = owned(&Program::new(false).theory, limits);
    for atom in 0..3 {
        assert!(add(&mut ledger, &[(atom, true)], &[(4, false)]));
    }
    ledger
}

fn conditions(ledger: &Ledger) -> Vec<Vec<(usize, bool)>> {
    ledger
        .rows
        .iter()
        .map(|entry| entry.row.conditions().collect())
        .collect()
}

#[test]
fn a_full_tier_evicts_the_row_with_fewest_hits() {
    let mut ledger = full(limits(3, 0));
    ledger.hit(0);
    ledger.hit(2);
    assert!(add(&mut ledger, &[(3, true)], &[(4, false)]));
    assert_eq!(ledger.evicted(), 1);
    assert_eq!(
        conditions(&ledger),
        [vec![(0, true)], vec![(3, true)], vec![(2, true)]]
    );
}

#[test]
fn eviction_takes_the_oldest_of_equal_hits() {
    let mut ledger = full(limits(3, 0));
    assert!(add(&mut ledger, &[(3, true)], &[(4, false)]));
    assert_eq!(
        conditions(&ledger),
        [vec![(3, true)], vec![(1, true)], vec![(2, true)]]
    );
}

#[test]
fn the_founded_floor_protects_every_founded_row() {
    let mut ledger = full(limits(3, 3));
    assert!(!ledger.can_capture());
    assert!(!add(&mut ledger, &[(3, true)], &[(4, false)]));
    assert_eq!(ledger.evicted(), 0);
    assert_eq!(ledger.len(), 3);
}

#[test]
fn founded_rows_above_the_floor_are_evictable() {
    let mut ledger = full(limits(3, 2));
    assert!(ledger.can_capture());
    assert!(add(&mut ledger, &[(3, true)], &[(4, false)]));
    assert_eq!(ledger.evicted(), 1);
}

#[test]
fn hits_halve_after_a_tier_of_retentions() {
    let mut ledger = owned(&Program::new(false).theory, limits(2, 0));
    assert!(add(&mut ledger, &[(0, true)], &[(4, false)]));
    for _ in 0..4 {
        ledger.hit(0);
    }
    assert_eq!(ledger.rows[0].hits, 4);
    assert!(add(&mut ledger, &[(1, true)], &[(4, false)]));
    assert_eq!(ledger.rows[0].hits, 2);
}
