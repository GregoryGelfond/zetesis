use super::{Outcome, Progress, Support, scan};
use crate::{Node, RegionLimits, test_support::theory};
use zetesis_cpu::{Cancellation, Stop};

use super::super::{NarrowingQuota, Producers, Work};

fn producers(bodies: usize) -> Producers {
    let mut nodes: Vec<_> = (0..=bodies).map(Node::atom).collect();
    let roots = (1..=bodies)
        .map(|body| {
            let root = nodes.len();
            nodes.push(Node::implies(body, 0));
            root
        })
        .collect();
    super::super::producers(
        &theory(bodies + 1, nodes, roots),
        RegionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .producers
    .unwrap()
}

fn expected(possible: usize, length: usize, held: bool) -> Outcome {
    let available: Vec<_> = (0..length)
        .filter(|position| possible & (1 << position) != 0)
        .collect();
    match available.as_slice() {
        [] => Outcome::None,
        [sole] if held => Outcome::Sole(*sole),
        _ => Outcome::Enough,
    }
}

fn checked(positions: &mut [usize; 2], possible: usize, length: usize, held: bool) {
    let result = scan(positions, length, held, |position| {
        Ok(possible & (1 << position) != 0)
    })
    .unwrap();
    assert_eq!(result, expected(possible, length, held));
}

#[test]
fn every_monotone_withdrawal_matches_complete_coverage() {
    const ROWS: usize = 5;
    for ancestor in 0..1 << ROWS {
        for held in [false, true] {
            let mut positions = [0; 2];
            checked(&mut positions, ancestor, ROWS, held);
            for descendant in 0..1 << ROWS {
                if descendant & ancestor != descendant {
                    continue;
                }
                // Each child gets the same completed parent, not another
                // child's eliminated prefix. An open head may become held.
                let mut child = positions;
                checked(&mut child, descendant, ROWS, held);
                checked(&mut child, descendant, ROWS, true);
            }
        }
    }
}

#[test]
fn repeated_withdrawals_read_only_witnesses_and_new_rows() {
    const ROWS: usize = 64;
    let mut available = [true; ROWS];
    let mut positions = [0_usize; 2];
    let mut visited = 0;
    let mut check = |available: &[bool; ROWS]| {
        scan(&mut positions, ROWS, true, |position| {
            visited += 1;
            Ok(available[position])
        })
        .unwrap()
    };
    assert_eq!(check(&available), Outcome::Enough);
    for withdrawn in 1..ROWS {
        available[withdrawn] = false;
        let outcome = check(&available);
        assert_eq!(
            outcome,
            if withdrawn + 1 == ROWS {
                Outcome::Sole(0)
            } else {
                Outcome::Enough
            }
        );
    }
    assert_eq!(visited, 3 * ROWS - 2);
    let restarted_reads = (2..=ROWS).sum::<usize>() + ROWS;
    assert!(visited * 10 < restarted_reads);
    assert_eq!(positions, [0, ROWS]);
    // The covered suffix still permits a zero-support conclusion after the
    // first witness fails, with only that witness read once more.
    let mut final_reads = 0;
    assert_eq!(
        scan(&mut positions, ROWS, true, |_| {
            final_reads += 1;
            Ok(false)
        })
        .unwrap(),
        Outcome::None
    );
    assert_eq!(final_reads, 1);
}

#[test]
fn newly_held_heads_require_a_second_witness() {
    let mut positions = [0_usize; 2];
    assert_eq!(
        scan(&mut positions, 5, false, |position| Ok(position == 2)).unwrap(),
        Outcome::Enough
    );
    assert_eq!(positions, [2, 2]);
    let mut visited = Vec::new();
    assert_eq!(
        scan(&mut positions, 5, true, |position| {
            visited.push(position);
            Ok(position == 2)
        })
        .unwrap(),
        Outcome::Sole(2)
    );
    assert_eq!(visited, [2, 3, 4]);
}

#[test]
fn preparation_is_private_until_every_row_is_charged() {
    let producers = producers(5);
    let needed = u64::try_from(producers.by_head.len()).unwrap() + 1;
    for limit in 0..needed {
        let mut progress = Progress::default();
        let mut work = Work::new(limit);
        assert_eq!(
            progress.prepare(&producers, &mut work),
            Err(Stop::WorkLimit)
        );
        assert_eq!(work.spent, limit);
        assert_eq!(progress, Progress::Vacant);
        assert_eq!(progress.allocated_bytes(), 0);
    }
    let mut progress = Progress::default();
    let mut work = Work::new(needed);
    progress.prepare(&producers, &mut work).unwrap();
    assert_eq!(work.spent, needed);
    let Progress::Small(rows) = &progress else {
        panic!("the complete producer count fits compact positions");
    };
    assert_eq!(rows, &vec![[0; 2]; producers.by_head.len()]);
    assert_eq!(
        progress.allocated_bytes(),
        rows.capacity() as u128 * size_of::<[u16; 2]>() as u128
    );
}

struct Quota {
    remaining: u64,
    failure: Stop,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, _: u64) -> Result<u64, Stop> {
        if self.remaining == 0 {
            Err(self.failure)
        } else {
            self.remaining -= 1;
            Ok(1)
        }
    }

    fn refund(&mut self, _: u64) {
        panic!("each one-unit reservation is consumed immediately");
    }
}

#[test]
fn interrupted_preparation_publishes_no_positions() {
    let producers = producers(5);
    for failure in [Stop::Cancelled, Stop::Deadline] {
        for permitted in 0..=producers.by_head.len() {
            let mut quota = Quota {
                remaining: u64::try_from(permitted).unwrap(),
                failure,
            };
            let mut work = Work::reserved(&mut quota);
            let mut progress = Progress::default();
            assert_eq!(progress.prepare(&producers, &mut work), Err(failure));
            assert_eq!(work.spent, u64::try_from(permitted).unwrap());
            assert_eq!(progress, Progress::Vacant);
        }
    }
}

#[test]
fn stopped_scans_never_manufacture_a_completed_outcome() {
    let producers = producers(5);
    for failure in [Stop::Cancelled, Stop::Deadline, Stop::WorkLimit] {
        for permitted in 0..5 {
            let mut progress = Progress::default();
            progress
                .prepare(&producers, &mut Work::new(u64::MAX))
                .unwrap();
            let mut quota = Quota {
                remaining: permitted,
                failure,
            };
            let mut work = Work::reserved(&mut quota);
            // Only the last producer remains possible. Every shorter scan
            // must refuse before either the sole or zero-support conclusion.
            let never = [(1_u64 << 5) - 2];
            assert!(matches!(
                progress.check(&producers, 0, true, &never, &[1], &mut work),
                Err(stop) if stop == failure
            ));
            assert_eq!(work.spent, permitted);
            let mut resumed = Work::new(5 - permitted);
            assert!(matches!(
                progress.check(&producers, 0, true, &never, &[1], &mut resumed),
                Ok(Support::Sole(Some(5)))
            ));
            assert_eq!(resumed.spent, 5 - permitted);
        }
    }
}

#[test]
fn short_rows_need_no_progress_allocation() {
    for length in 0..=2 {
        let producers = producers(length);
        let mut progress = Progress::default();
        let mut work = Work::new(u64::try_from(length).unwrap());
        let result = progress
            .check(&producers, 0, true, &[0], &[1], &mut work)
            .unwrap();
        assert!(matches!(
            (length, result),
            (0, Support::Unavailable) | (1, Support::Sole(Some(1))) | (2, Support::Possible)
        ));
        assert_eq!(progress, Progress::Vacant);
        assert_eq!(progress.allocated_bytes(), 0);
    }
}

#[test]
fn compact_and_native_positions_preserve_the_same_scan() {
    let producers = producers(5);
    let mut widths = [
        Progress::Small(vec![[0; 2]; 6]),
        Progress::Medium(vec![[0; 2]; 6]),
        Progress::Wide(vec![[0; 2]; 6]),
    ];
    for progress in &mut widths {
        let mut work = Work::new(5);
        assert!(matches!(
            progress.check(&producers, 0, true, &[0b01110], &[1], &mut work),
            Ok(Support::Possible)
        ));
        assert_eq!(work.spent, 5);
        let mut work = Work::new(2);
        assert!(matches!(
            progress.check(&producers, 0, true, &[0b11110], &[1], &mut work),
            Ok(Support::Sole(Some(5)))
        ));
        assert_eq!(work.spent, 2);
    }
}

#[test]
fn clones_own_positions_and_restore_the_parent_prefix() {
    let parent = Progress::Small(vec![[1, 4], [0, 0]]);
    let mut child = parent.clone();
    let Progress::Small(rows) = &mut child else {
        unreachable!();
    };
    let child_address = rows.as_ptr();
    rows[0] = [4, 5];
    assert_ne!(child, parent);
    child.clone_from(&parent);
    assert_eq!(child, parent);
    let (Progress::Small(source), Progress::Small(destination)) = (&parent, &child) else {
        unreachable!();
    };
    assert_eq!(destination.as_ptr(), child_address);
    assert_ne!(source.as_ptr(), destination.as_ptr());
    assert_eq!(
        child.allocated_bytes(),
        destination.capacity() as u128 * size_of::<[u16; 2]>() as u128
    );
    child.clone_from(&Progress::Vacant);
    assert_eq!(child, Progress::Vacant);
    assert_eq!(child.allocated_bytes(), 0);
}

#[test]
fn knowledge_receipts_count_owned_capacity_once() {
    use super::super::{Narrower, Width};

    let index = Narrower::new(&theory(1, vec![Node::atom(0)], vec![]));
    let mut knowledge = index.knowledge();
    let empty_bytes = knowledge.retained_bytes();
    let mut rows = Vec::with_capacity(12);
    rows.push([0_u16; 2]);
    let allocated = rows.capacity() as u128 * size_of::<[u16; 2]>() as u128;
    let Width::Compact16(known) = &mut knowledge.width else {
        panic!("one atom uses compact counters");
    };
    known.support_progress = Progress::Small(rows);
    assert_eq!(knowledge.retained_bytes(), empty_bytes + allocated);
    let mut cloned = knowledge.clone();
    let Width::Compact16(known) = &cloned.width else {
        unreachable!();
    };
    assert_eq!(
        cloned.retained_bytes(),
        empty_bytes + known.support_progress.allocated_bytes()
    );
    cloned.clone_from(&index.knowledge());
    assert_eq!(cloned.retained_bytes(), empty_bytes);
}

#[test]
fn every_interrupted_prefix_retains_only_proved_failures() {
    const ROWS: usize = 5;
    for possible in 0..1 << ROWS {
        for held in [false, true] {
            for permitted in 0..ROWS {
                let mut positions = [0_usize; 2];
                let mut visited = 0;
                let result = scan(&mut positions, ROWS, held, |position| {
                    if visited == permitted {
                        return Err(Stop::Cancelled);
                    }
                    visited += 1;
                    Ok(possible & (1 << position) != 0)
                });
                let [first, second] = positions;
                assert!(first <= second && second <= ROWS);
                for position in 0..ROWS {
                    if position < first || (first < position && position < second) {
                        assert_eq!(possible & (1 << position), 0);
                    }
                }
                if let Ok(outcome) = result {
                    assert_eq!(outcome, expected(possible, ROWS, held));
                } else {
                    assert_eq!(visited, permitted);
                    checked(&mut positions, possible, ROWS, held);
                }
            }
        }
    }
}
