use super::{NodeId, Projections};
use crate::search::cursor::tests::budget;
use crate::{Assignment, Control, Incomplete, ProjectionLimits, ProjectionResource};

fn limited(resource: ProjectionResource, required: u128, limit: u128) -> Incomplete {
    Incomplete::ProjectionLimit { resource, required, limit }
}

#[test]
fn empty_history_admits_its_header() {
    let bytes = size_of::<Projections>();
    for max_bytes in [bytes - 1, bytes] {
        let result = Projections::new(0, ProjectionLimits { max_bytes, ..ProjectionLimits::default() });
        if max_bytes < bytes {
            assert_eq!(result.unwrap_err(), limited(ProjectionResource::Bytes, bytes as u128, max_bytes as u128));
        } else {
            let history = result.unwrap().statistics();
            assert_eq!(history.retained_bytes, bytes as u128);
            assert_eq!(history.peak_bytes, bytes as u128);
            assert_eq!(history.entries, 0);
            assert_eq!(history.nodes, 0);
        }
    }
}

#[test]
fn zero_width_key_uses_one_entry_without_nodes() {
    let control = Control::default();
    let mut charged = budget(&control);
    let mut index = Projections::new(0, ProjectionLimits {
        max_entries: 1, max_nodes: 0, max_bytes: size_of::<Projections>(),
    }).unwrap();
    assert!(index.permits(&Assignment(vec![]), &mut charged).unwrap());
    index.insert(0, |_| panic!("empty key"), &mut charged).unwrap();
    index.insert(0, |_| panic!("duplicate empty key"), &mut charged).unwrap();
    assert!(!index.permits(&Assignment(vec![]), &mut charged).unwrap());
    let history = index.statistics();
    assert_eq!(history.entries, 1);
    assert_eq!(history.nodes, 0);
    assert_eq!(history.retained_bytes, size_of::<Projections>() as u128);
    // Two lookup entries and two insert entries; no bit or terminal visits.
    assert_eq!(history.work, 4);
    assert_eq!(history.work, charged.statistics.work);
}

#[test]
fn full_entry_limit_still_admits_duplicate_keys() {
    let control = Control::default();
    let mut index = Projections::new(2, ProjectionLimits {
        max_entries: 1, ..ProjectionLimits::default()
    }).unwrap();
    let mut charged = budget(&control);
    index.insert(2, |_| false, &mut charged).unwrap();
    let before = index.statistics();
    let nodes = index.nodes.clone();
    index.insert(2, |_| false, &mut charged).unwrap();
    assert_eq!(index.insert(2, |_| true, &mut charged), Err(limited(ProjectionResource::Entries, 2, 1)));
    assert_eq!(index.nodes, nodes);
    assert_eq!(index.statistics().entries, 1);
    assert_eq!(index.statistics().retained_bytes, before.retained_bytes);
    assert_eq!(index.statistics().peak_bytes, before.peak_bytes);
    assert!(!index.permits(&Assignment(vec![false; 2]), &mut charged).unwrap());
    assert!(index.permits(&Assignment(vec![true; 2]), &mut charged).unwrap());
}

#[test]
fn node_limit_counts_only_the_missing_suffix() {
    let control = Control::default();
    let mut index = Projections::new(4, ProjectionLimits {
        max_nodes: 6, ..ProjectionLimits::default()
    }).unwrap();
    let mut charged = budget(&control);
    index.insert(4, |_| false, &mut charged).unwrap();
    // Four bits plus terminal use five nodes. Changing only the last bit
    // shares the prefix and needs exactly one more terminal, not a full key.
    assert_eq!(index.statistics().nodes, 5);
    index.insert(4, |bit| bit == 3, &mut charged).unwrap();
    assert_eq!(index.statistics().nodes, 6);
    let nodes = index.nodes.clone();
    assert_eq!(index.insert(4, |bit| bit == 0, &mut charged), Err(limited(ProjectionResource::Nodes, 10, 6)));
    assert_eq!(index.nodes, nodes);
    assert_eq!(index.statistics().entries, 2);
    assert!(index.permits(&Assignment(vec![true, false, false, false]), &mut charged).unwrap());
}

#[test]
fn byte_refusal_preserves_the_previous_key_set() {
    let control = Control::default();
    let header = size_of::<Projections>();
    let mut index = Projections::new(2, ProjectionLimits {
        max_bytes: header, ..ProjectionLimits::default()
    }).unwrap();
    let mut charged = budget(&control);
    let required = header + 3 * size_of::<[Option<NodeId>; 2]>();
    for _ in 0..2 {
        assert_eq!(index.insert(2, |_| panic!("capacity precedes key reads"), &mut charged),
            Err(limited(ProjectionResource::Bytes, required as u128, header as u128)));
        assert_eq!(index.statistics().entries, 0);
        assert_eq!(index.statistics().nodes, 0);
        assert_eq!(index.statistics().retained_bytes, header as u128);
        assert_eq!(index.statistics().peak_bytes, header as u128);
    }
    assert_eq!(index.statistics().work, 2);
    for values in [[false, false], [false, true], [true, false], [true, true]] {
        assert!(index.permits(&Assignment(values.to_vec()), &mut charged).unwrap());
    }
}

#[test]
fn growth_peak_includes_both_vector_capacities() {
    let control = Control::default();
    let mut index = Projections::new(4, ProjectionLimits::default()).unwrap();
    let header = size_of::<Projections>() as u128;
    let node_bytes = size_of::<[Option<NodeId>; 2]>() as u128;
    let mut peak = header;
    let mut charged = budget(&control);
    for key in 0..16 {
        let old = index.nodes.capacity();
        index.insert(4, |bit| key & (1 << bit) != 0, &mut charged).unwrap();
        let current = index.nodes.capacity();
        if current > old {
            peak = peak.max(header + (old as u128 + current as u128) * node_bytes);
        }
        assert_eq!(index.statistics().retained_bytes, header + current as u128 * node_bytes);
        assert_eq!(index.statistics().peak_bytes, peak);
    }
    // Complete binary tree for all sixteen four-bit keys, independently counted.
    assert_eq!(index.statistics().entries, 16);
    assert_eq!(index.statistics().nodes, 1 + 2 + 4 + 8 + 16);
    assert!(peak > index.statistics().retained_bytes);
}

#[test]
fn stopped_suffix_receipts_count_only_committed_keys() {
    let control = Control::default();
    let mut index = Projections::new(4, ProjectionLimits::default()).unwrap();
    for attempt in 1..=2 {
        let mut charged = budget(&control);
        charged.limits.max_work = 3;
        assert_eq!(index.insert(4, |_| false, &mut charged), Err(Incomplete::WorkLimit));
        assert_eq!(index.statistics().work, attempt * 3);
        assert_eq!(index.statistics().entries, 0);
        assert_eq!(index.statistics().nodes, 0);
        assert_eq!(index.statistics().peak_bytes, index.statistics().retained_bytes);
    }
    index.insert(4, |_| false, &mut budget(&control)).unwrap();
    assert_eq!(index.statistics().entries, 1);
    assert_eq!(index.statistics().nodes, 5);
    assert_eq!(index.statistics().work, 12);
}
