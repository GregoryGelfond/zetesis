use super::*;
use crate::relation::{Limits, Resource};
use std::mem::size_of_val;

fn column(ids: &[u32]) -> OwnedColumn {
    let mut work = Work::new(Limits::default(), size_of::<OwnedColumn>() as u128).unwrap();
    let mut column =
        OwnedColumn::zeroed(ids.len(), ids.iter().copied().max().unwrap_or(0), &mut work).unwrap();
    for (row, &id) in ids.iter().enumerate() {
        column.set(row, id);
    }
    column
}

fn work(column: &OwnedColumn, limits: Limits) -> Work {
    Work::new(
        limits,
        (size_of::<OwnedColumn>() + column.retained_bytes()) as u128,
    )
    .unwrap()
}

#[test]
fn physical_width_preserves_decoded_equality() {
    let bytes = [0_u8, 255, 0, 13];
    let halves = [0_u16, 255, 0, 13];
    let words = [0_u32, 255, 0, 13];
    assert_eq!(Column::U8(&bytes), Column::U16(&halves));
    assert_eq!(Column::U16(&halves), Column::U32(&words));
    assert_eq!(Column::U8(&bytes).get(4), None);
    assert_eq!(
        Column::U8(&bytes).iter().rev().collect::<Vec<_>>(),
        [13, 0, 255, 0]
    );
}

#[test]
fn sparse_identifiers_choose_sufficient_width() {
    for (ids, bits) in [
        ([0, 255], 8),
        ([0, 256], 16),
        ([0, 65_535], 16),
        ([0, 65_536], 32),
        ([0, u32::MAX], 32),
    ] {
        let column = column(&ids);
        assert_eq!(column.view().bits(), bits);
        assert_eq!(column.view().iter().collect::<Vec<_>>(), ids);
    }
}

#[test]
fn widening_preserves_every_existing_identifier() {
    let mut column = column(&[255, 0, 13, 255]);
    for (next, bits) in [(256, 16), (65_536, 32), (u32::MAX, 32)] {
        let before = column.view().iter().collect::<Vec<_>>();
        let mut work = work(&column, Limits::default());
        column.reserve(next, &mut work).unwrap();
        assert_eq!(column.view().iter().collect::<Vec<_>>(), before);
        column.push(next);
        assert_eq!(column.view().bits(), bits);
        assert_eq!(
            column.view().iter().collect::<Vec<_>>(),
            [before, vec![next]].concat()
        );
    }
}

#[test]
fn interrupted_widening_retains_the_original_owner() {
    for limit in 0..4 {
        let mut column = column(&[255, 0, 13, 255]);
        let capacity = column.retained_bytes();
        let mut work = work(
            &column,
            Limits {
                max_work: limit,
                ..Limits::default()
            },
        );
        let initial = work.live;
        assert!(matches!(
            column.reserve(256, &mut work),
            Err(Failure::Limit {
                resource: Resource::Work,
                ..
            })
        ));
        assert_eq!(work.used, u128::from(limit));
        assert_eq!(work.live, initial);
        assert_eq!(column.retained_bytes(), capacity);
        assert_eq!(column.view().bits(), 8);
        assert_eq!(column.view().iter().collect::<Vec<_>>(), [255, 0, 13, 255]);
    }
}

#[test]
fn widening_charges_each_copied_cell_once() {
    let mut column = column(&[255, 0, 13, 255]);
    let mut work = work(
        &column,
        Limits {
            max_work: 4,
            ..Limits::default()
        },
    );
    column.reserve(256, &mut work).unwrap();
    assert_eq!(work.used, 4);
}

#[test]
fn later_reservation_failure_preserves_the_published_rows() {
    let mut columns = [column(&[255, 0]), column(&[255, 0])];
    let bytes = size_of_val(&columns)
        + columns
            .iter()
            .map(OwnedColumn::retained_bytes)
            .sum::<usize>();
    let mut work = Work::new(
        Limits {
            max_work: 2,
            ..Limits::default()
        },
        bytes as u128,
    )
    .unwrap();
    columns[0].reserve(256, &mut work).unwrap();
    assert!(matches!(
        columns[1].reserve(257, &mut work),
        Err(Failure::Limit {
            resource: Resource::Work,
            ..
        })
    ));
    assert_eq!(columns[0].view().bits(), 16);
    assert_eq!(columns[1].view().bits(), 8);
    for column in &columns {
        assert_eq!(column.view().iter().collect::<Vec<_>>(), [255, 0]);
    }
    assert_eq!(
        work.live,
        size_of_val(&columns)
            + columns
                .iter()
                .map(OwnedColumn::retained_bytes)
                .sum::<usize>()
    );
}

#[test]
fn widening_peak_includes_both_buffers() {
    let mut column = column(&[255, 0, 13, 255]);
    let mut work = work(&column, Limits::default());
    let initial = work.live;
    column.reserve(256, &mut work).unwrap();
    assert_eq!(work.peak, initial + column.retained_bytes());
    assert_eq!(
        work.live,
        size_of::<OwnedColumn>() + column.retained_bytes()
    );
}

#[test]
fn widening_byte_limit_is_inclusive() {
    let mut reference = column(&[255, 0, 13, 255]);
    let mut measured = work(&reference, Limits::default());
    reference.reserve(256, &mut measured).unwrap();
    for (limit, accepted) in [(measured.peak, true), (measured.peak - 1, false)] {
        let mut column = column(&[255, 0, 13, 255]);
        let mut work = work(
            &column,
            Limits {
                max_bytes: limit,
                ..Limits::default()
            },
        );
        let result = column.reserve(256, &mut work);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(Failure::Limit {
                    resource: Resource::Bytes,
                    ..
                })
            ));
        }
        assert_eq!(column.view().iter().collect::<Vec<_>>(), [255, 0, 13, 255]);
    }
}

#[test]
fn reset_retains_reusable_physical_capacity() {
    let mut column = column(&[65_536, 0]);
    let capacity = column.retained_bytes();
    column.clear();
    assert!(column.view().is_empty());
    assert_eq!(column.retained_bytes(), capacity);
    let mut work = work(
        &column,
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    );
    column.reserve(1, &mut work).unwrap();
    column.push(1);
    assert_eq!(column.view().get(0), Some(1));
}
