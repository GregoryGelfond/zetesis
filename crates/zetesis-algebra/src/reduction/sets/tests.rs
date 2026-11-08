use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::{intersection, ordered_union, union};

#[test]
fn occurrence_refinements_match_independent_finite_set_operations() {
    for first in 0..16_u8 {
        let left: BTreeSet<_> = (0..4).filter(|bit| first & (1 << bit) != 0).collect();
        for second in 0..16_u8 {
            let right: BTreeSet<_> = (0..4).filter(|bit| second & (1 << bit) != 0).collect();
            let reads = left
                .iter()
                .copied()
                .map(|item| (item, right.contains(&item)));
            assert_eq!(
                intersection(reads).collect::<Vec<_>>(),
                left.intersection(&right).copied().collect::<Vec<_>>()
            );
            assert_eq!(
                ordered_union(left.iter().copied(), right.iter().copied()).collect::<Vec<_>>(),
                left.union(&right).copied().collect::<Vec<_>>()
            );
            assert_eq!(
                union(left.iter().copied(), right.iter().copied()).collect::<BTreeSet<_>>(),
                left.union(&right).copied().collect::<BTreeSet<_>>()
            );
        }
    }
}

#[test]
fn occurrence_union_preserves_duplicate_admission_order() {
    let left = [3, 1, 3];
    let right = [3, 2, 2];
    assert_eq!(
        union(left.iter().copied(), right.iter().copied()).collect::<Vec<_>>(),
        [3, 1, 3, 3, 2, 2]
    );
}

#[test]
fn intersection_reads_only_through_the_next_selected_occurrence() {
    let reads = RefCell::new(Vec::new());
    let records = [(0, false), (1, true), (1, true), (2, false), (3, true)]
        .into_iter()
        .inspect(|&(item, _)| reads.borrow_mut().push(item));
    let mut selected = intersection(records);
    assert!(reads.borrow().is_empty());
    assert_eq!(selected.next(), Some(1));
    assert_eq!(*reads.borrow(), [0, 1]);
    assert_eq!(selected.next(), Some(1));
    assert_eq!(*reads.borrow(), [0, 1, 1]);
    assert_eq!(selected.next(), Some(3));
    assert_eq!(*reads.borrow(), [0, 1, 1, 2, 3]);
    assert_eq!(selected.next(), None);
}

// Observational instrumentation is confined to this test representation.
// Native representation reads and order must satisfy the documented purity.
#[derive(Clone, Copy, Debug)]
struct Item<'a> {
    key: u8,
    origin: char,
    trace: &'a RefCell<Vec<String>>,
}
impl PartialEq for Item<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for Item<'_> {}
impl PartialOrd for Item<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Item<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.trace
            .borrow_mut()
            .push(format!("cmp {} {}", self.key, other.key));
        self.key.cmp(&other.key)
    }
}

#[test]
fn ordered_union_keeps_the_left_equal_representative_and_read_order() {
    let trace = RefCell::new(Vec::new());
    let left = [1, 2].into_iter().map(|key| {
        trace.borrow_mut().push(format!("read L {key}"));
        Item {
            key,
            origin: 'L',
            trace: &trace,
        }
    });
    let right = [1, 3].into_iter().map(|key| {
        trace.borrow_mut().push(format!("read R {key}"));
        Item {
            key,
            origin: 'R',
            trace: &trace,
        }
    });
    let mut merged = ordered_union(left, right);
    assert!(trace.borrow().is_empty());
    let first = merged.next().unwrap();
    assert_eq!((first.key, first.origin), (1, 'L'));
    assert_eq!(*trace.borrow(), ["read L 1", "read R 1", "cmp 1 1"]);
    let second = merged.next().unwrap();
    assert_eq!((second.key, second.origin), (2, 'L'));
    assert_eq!(
        *trace.borrow(),
        [
            "read L 1", "read R 1", "cmp 1 1", "read L 2", "read R 3", "cmp 2 3"
        ]
    );
    let third = merged.next().unwrap();
    assert_eq!((third.key, third.origin), (3, 'R'));
    assert_eq!(merged.next(), None);
}

#[test]
fn refused_publication_does_not_read_beyond_its_selected_prefix() {
    let trace = RefCell::new(Vec::new());
    let left = [1, 2].into_iter().map(|key| {
        trace.borrow_mut().push(format!("read L {key}"));
        Item {
            key,
            origin: 'L',
            trace: &trace,
        }
    });
    let right = [1, 3].into_iter().map(|key| {
        trace.borrow_mut().push(format!("read R {key}"));
        Item {
            key,
            origin: 'R',
            trace: &trace,
        }
    });
    let refused: Result<(), &'static str> = ordered_union(left, right).try_for_each(|item| {
        assert_eq!((item.key, item.origin), (1, 'L'));
        Err("publication refused")
    });
    assert_eq!(refused, Err("publication refused"));
    assert_eq!(*trace.borrow(), ["read L 1", "read R 1", "cmp 1 1"]);
}
