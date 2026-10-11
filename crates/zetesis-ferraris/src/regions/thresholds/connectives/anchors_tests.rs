//! Sparse anchor discovery does not authenticate or retain relation pairs.

use std::mem::size_of;

use super::{Chain, Connectives, Row, Work, anchor_keys};
use crate::Threshold;
use zetesis_cpu::Stop;

fn row(conditions: &[usize], weights: &[u64]) -> Row {
    Row::new(Threshold {
        conditions,
        weights,
        bound: 1,
    })
    .unwrap()
}

fn chain(leaves: &[usize]) -> Chain {
    Chain {
        disjunction: false,
        root: 0,
        operands: leaves.to_vec(),
    }
}

#[test]
fn no_unit_rows_have_no_anchor_storage() {
    for rows in [vec![], vec![row(&[], &[])], vec![row(&[0, 1], &[2, 1])]] {
        let keys = anchor_keys(&rows).unwrap();
        assert!(keys.is_empty());
        assert_eq!(keys.capacity(), 0);
        // The prepare branch returns before calling chain_anchors.
        assert!(
            Connectives::prepare(&rows, &[chain(&[1, 0])])
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn compact_postings_record_each_owner_once() {
    let count = 128;
    let rows: Vec<_> = (0..count).map(|_| row(&[0, 1], &[1, 1])).collect();
    let chains: Vec<_> = (0..count).map(|_| chain(&[1, 0])).collect();
    let anchors = Connectives::prepare(&rows, &chains).unwrap().unwrap();
    assert_eq!(anchors.anchors, [0]);
    assert_eq!(anchors.rows.len(), 1);
    assert_eq!(anchors.chains.len(), 1);
    assert_eq!(anchors.rows[0], (0..count).collect::<Vec<_>>());
    assert_eq!(anchors.chains[0], (0..count).collect::<Vec<_>>());
    assert_eq!(
        anchors.by_chain,
        (0..count).map(|chain| (chain, 0)).collect::<Vec<_>>()
    );
    // Every pair really qualifies, yet only owner postings and a reverse
    // projection are retained: no count * count authenticated relation table.
    assert_eq!(
        anchors.rows.iter().map(<[usize]>::len).sum::<usize>(),
        count
    );
    assert_eq!(
        anchors.chains.iter().map(<[usize]>::len).sum::<usize>(),
        count
    );
    let payload = anchors.rows.retained_bytes()
        + anchors.chains.retained_bytes()
        + anchors.anchors.capacity() as u128 * size_of::<usize>() as u128
        + anchors.by_chain.capacity() as u128 * size_of::<(usize, usize)>() as u128;
    assert!(payload < (count * count * size_of::<usize>()) as u128);
}

#[test]
fn a_shared_parent_with_a_different_minimum_is_excluded() {
    let rows = [row(&[1, 2], &[1, 1])];
    let chains = [chain(&[0, 2, 1]), chain(&[2, 1])];
    let anchors = Connectives::prepare(&rows, &chains).unwrap().unwrap();
    let mut work = Work::new(u64::MAX);
    // Both chains contain condition 1, but only the second may be an exact
    // prefix or extension. Exclusion occurs before exact_prefix is called.
    assert_eq!(anchors.chains(1, &mut work).unwrap(), [1]);
    assert!(anchors.rows(0, &mut work).unwrap().is_empty());
    assert_eq!(anchors.rows(1, &mut work).unwrap(), [0]);
    assert_eq!(chains[0].operands, [0, 2, 1]);
    assert_eq!(chains[1].operands, [2, 1]);
}

#[test]
fn repeated_anchors_preserve_both_owner_orders() {
    let rows = [
        row(&[40, 42], &[1, 1]),
        row(&[3, 5], &[1, 1]),
        row(&[40, 44], &[1, 1]),
        row(&[3, 4], &[2, 1]),
    ];
    let chains = [
        chain(&[5, 3]),
        chain(&[1, 3]),
        chain(&[44, 40]),
        chain(&[42, 40]),
        chain(&[9]),
    ];
    let anchors = Connectives::prepare(&rows, &chains).unwrap().unwrap();
    let mut work = Work::new(u64::MAX);
    assert_eq!(anchors.anchors, [3, 40]);
    assert_eq!(anchors.rows(0, &mut work).unwrap(), [1]);
    assert_eq!(anchors.rows(2, &mut work).unwrap(), [0, 2]);
    assert_eq!(anchors.rows(3, &mut work).unwrap(), [0, 2]);
    assert_eq!(anchors.chains(3, &mut work).unwrap(), [0]);
    assert_eq!(anchors.chains(40, &mut work).unwrap(), [2, 3]);
    assert!(anchors.rows(1, &mut work).unwrap().is_empty());
    assert!(anchors.rows(4, &mut work).unwrap().is_empty());
    assert!(anchors.chains(9, &mut work).unwrap().is_empty());
}

#[test]
fn rebuilding_does_not_reuse_another_owners_chain_anchor() {
    let first = Connectives::prepare(&[row(&[3, 5], &[1, 1])], &[chain(&[5, 3])])
        .unwrap()
        .unwrap();
    let second = Connectives::prepare(&[row(&[40, 42], &[1, 1])], &[chain(&[42, 40])])
        .unwrap()
        .unwrap();
    let mut work = Work::new(u64::MAX);
    assert_eq!(first.chains(3, &mut work).unwrap(), [0]);
    assert!(first.chains(40, &mut work).unwrap().is_empty());
    assert_eq!(second.chains(40, &mut work).unwrap(), [0]);
    assert!(second.chains(3, &mut work).unwrap().is_empty());
    assert_eq!(first.rows(0, &mut work).unwrap(), [0]);
    assert_eq!(second.rows(0, &mut work).unwrap(), [0]);
}

#[test]
fn sparse_lookup_charges_every_attempted_comparison() {
    let rows = [row(&[1, 2], &[1, 1]), row(&[4, 5], &[1, 1])];
    let chains = [chain(&[2, 1]), chain(&[0, 3]), chain(&[5, 4])];
    let anchors = Connectives::prepare(&rows, &chains).unwrap().unwrap();
    for reverse in [false, true] {
        for key in 0..7 {
            let query = |work: &mut Work<'_>| {
                if reverse {
                    anchors.rows(key, work)
                } else {
                    anchors.chains(key, work)
                }
            };
            let mut full = Work::new(u64::MAX);
            query(&mut full).unwrap();
            assert!(full.spent > 0);
            for limit in 0..full.spent {
                let mut limited = Work::new(limit);
                assert_eq!(query(&mut limited), Err(Stop::WorkLimit));
                assert_eq!(limited.spent, limit);
            }
            let mut exact = Work::new(full.spent);
            assert_eq!(query(&mut exact).unwrap(), query(&mut full).unwrap());
        }
    }
}
