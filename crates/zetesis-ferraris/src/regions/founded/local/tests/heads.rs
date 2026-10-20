//! A disposable head prefix retains every discovered vertex and failure boundary.

use super::*;

fn scratch(vertices: Vec<usize>, atoms: usize) -> Scratch {
    let mut actual = Scratch {
        queue: vertices,
        ..Scratch::default()
    };
    actual.local.atoms = atoms;
    actual.local.marks.resize(16, 0);
    for &vertex in &actual.queue {
        actual.local.marks[vertex] = FORWARD;
        actual.local.touched.push(vertex);
    }
    actual
}

#[test]
fn partition_preserves_every_discovered_vertex() {
    for pattern in 0_u16..256 {
        let vertices: Vec<_> = (0..8)
            .map(|position| {
                if pattern & (1 << position) == 0 {
                    position + 8
                } else {
                    position
                }
            })
            .collect();
        let heads: Vec<_> = vertices
            .iter()
            .copied()
            .filter(|vertex| *vertex < 8)
            .collect();
        let mut actual = scratch(vertices.clone(), 8);
        let marks = actual.local.marks.clone();
        let touched = actual.local.touched.clone();
        actual
            .partition_heads(heads.len(), &mut Work::new(u64::MAX))
            .unwrap();
        assert_eq!(&actual.queue[..heads.len()], &heads);
        assert!(
            actual.queue[heads.len()..]
                .iter()
                .all(|vertex| *vertex >= 8)
        );
        let mut expected = vertices;
        expected.sort_unstable();
        let mut observed = actual.queue.clone();
        observed.sort_unstable();
        assert_eq!(observed, expected);
        assert_eq!(actual.local.marks, marks);
        assert_eq!(actual.local.touched, touched);
    }
}

#[test]
fn partition_stops_after_the_last_head() {
    let mut actual = scratch(vec![8, 0, 9, 10, 11, 12], 8);
    let mut work = Work::new(u64::MAX);
    actual.partition_heads(1, &mut work).unwrap();
    assert_eq!(work.spent, 3); // Two visited entries and one swap.
    assert_eq!(actual.queue, vec![0, 8, 9, 10, 11, 12]);
    let mut empty = scratch(vec![8, 9, 10], 8);
    empty.partition_heads(0, &mut Work::new(0)).unwrap();
}

#[test]
fn stopped_partitions_keep_membership_clearable() {
    let vertices = vec![8, 0, 9, 1, 10, 2, 11, 3];
    let mut measured = scratch(vertices.clone(), 8);
    let mut complete = Work::new(u64::MAX);
    measured.partition_heads(4, &mut complete).unwrap();
    for failure in [
        Stop::WorkLimit,
        Stop::Cancelled,
        Stop::Deadline,
        Stop::Allocation,
    ] {
        for ceiling in 0..complete.spent {
            let mut actual = scratch(vertices.clone(), 8);
            let marks = actual.local.marks.clone();
            let touched = actual.local.touched.clone();
            let mut allowance = Allowance {
                remaining: ceiling,
                failure,
            };
            let mut work = Work::reserved(&mut allowance);
            assert_eq!(actual.partition_heads(4, &mut work), Err(failure));
            work.refund();
            let mut expected = vertices.clone();
            expected.sort_unstable();
            actual.queue.sort_unstable();
            assert_eq!(actual.queue, expected);
            assert_eq!(actual.local.marks, marks);
            assert_eq!(actual.local.touched, touched);
            actual.local.clear(&mut Work::new(u64::MAX)).unwrap();
            assert!(actual.local.marks.iter().all(|mark| *mark == 0));
            assert!(actual.local.touched.is_empty());
        }
    }
}
