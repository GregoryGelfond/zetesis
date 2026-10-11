use super::{ExtremaMeasure, ExtremumCutRule, ExtremumRow};
use zetesis_cpu::{Cancellation, Stop};

#[test]
fn every_mask_matches_selected_rank_extrema() {
    let row = ExtremumRow {
        conditions: &[0, 1, 2, 3],
        ranks: &[3, 0, 4, 3],
        rank_count: 5,
    };
    for mask in 0usize..16 {
        let selected: Vec<_> = row
            .ranks
            .iter()
            .enumerate()
            .filter_map(|(index, &rank)| (mask & (1 << index) != 0).then_some(rank))
            .collect();
        let measure = row
            .reduce(|condition| Ok::<_, Stop>(mask & (1 << condition) != 0))
            .unwrap();
        assert_eq!(measure.minimum, selected.iter().copied().min().unwrap_or(4));
        assert_eq!(measure.maximum, selected.iter().copied().max().unwrap_or(0));
        for boundary in 0..=5 {
            let prefix = boundary == 5 || selected.iter().any(|&rank| rank < boundary);
            let suffix = boundary == 0 || selected.iter().any(|&rank| rank >= boundary);
            assert_eq!(ExtremumCutRule::Prefix(boundary).holds(measure), prefix);
            assert_eq!(ExtremumCutRule::Suffix(boundary).holds(measure), suffix);
        }
    }
}

#[test]
fn empty_selection_uses_both_endpoint_identities() {
    for row in [
        ExtremumRow {
            conditions: &[],
            ranks: &[],
            rank_count: 7,
        },
        ExtremumRow {
            conditions: &[0, 1],
            ranks: &[1, 5],
            rank_count: 7,
        },
    ] {
        assert_eq!(
            row.reduce(|_| Ok::<_, Stop>(false)).unwrap(),
            ExtremaMeasure {
                minimum: 6,
                maximum: 0,
            }
        );
    }
}

#[test]
fn a_selected_greatest_endpoint_is_not_empty() {
    let measure = ExtremumRow {
        conditions: &[0],
        ranks: &[6],
        rank_count: 7,
    }
    .reduce(|_| Ok::<_, Stop>(true))
    .unwrap();
    assert_eq!(
        measure,
        ExtremaMeasure {
            minimum: 6,
            maximum: 6
        }
    );
    assert!(!ExtremumCutRule::Prefix(6).holds(measure));
    assert!(ExtremumCutRule::Suffix(6).holds(measure));
}

#[test]
fn a_selected_least_endpoint_is_not_empty() {
    let measure = ExtremumRow {
        conditions: &[0],
        ranks: &[0],
        rank_count: 7,
    }
    .reduce(|_| Ok::<_, Stop>(true))
    .unwrap();
    assert_eq!(
        measure,
        ExtremaMeasure {
            minimum: 0,
            maximum: 0
        }
    );
    assert!(ExtremumCutRule::Prefix(1).holds(measure));
    assert!(!ExtremumCutRule::Suffix(1).holds(measure));
}

#[test]
fn repeated_conditions_keep_every_rank_occurrence() {
    let row = ExtremumRow {
        conditions: &[7, 3, 7, 3],
        ranks: &[0, 2, 4, 1],
        rank_count: 5,
    };
    let mut visits = Vec::new();
    let measure = row
        .reduce(|condition| {
            visits.push(condition);
            Ok::<_, Stop>(condition == 7)
        })
        .unwrap();
    assert_eq!(visits, [7, 3, 7, 3]);
    assert_eq!(
        measure,
        ExtremaMeasure {
            minimum: 0,
            maximum: 4
        }
    );
}

#[test]
fn malformed_shape_refuses_before_callback() {
    for row in [
        ExtremumRow {
            conditions: &[0],
            ranks: &[],
            rank_count: 2,
        },
        ExtremumRow {
            conditions: &[],
            ranks: &[0],
            rank_count: 2,
        },
        ExtremumRow {
            conditions: &[],
            ranks: &[],
            rank_count: 0,
        },
        ExtremumRow {
            conditions: &[],
            ranks: &[],
            rank_count: 1,
        },
    ] {
        let mut visits = 0;
        assert_eq!(
            row.reduce(|_| {
                visits += 1;
                Ok::<_, Stop>(false)
            }),
            Err(Stop::InvalidProgram)
        );
        assert_eq!(visits, 0);
    }
}

#[test]
fn unselected_invalid_ranks_are_refused() {
    for rank in [2, u64::MAX] {
        let row = ExtremumRow {
            conditions: &[0, 1],
            ranks: &[0, rank],
            rank_count: 2,
        };
        assert_eq!(
            row.reduce(|_| Ok::<_, Stop>(false)),
            Err(Stop::InvalidProgram)
        );
    }
}

#[test]
fn callback_stop_survives_decisive_extrema() {
    let row = ExtremumRow {
        conditions: &[0, 1, 2],
        ranks: &[0, 4, 2],
        rank_count: 5,
    };
    let cancellation = Cancellation::default();
    let mut visits = Vec::new();
    let result = row.reduce(|condition| {
        visits.push(condition);
        if condition == 2 {
            cancellation.cancel();
        }
        cancellation.poll()?;
        Ok(true)
    });
    assert_eq!(result, Err(Stop::Cancelled));
    assert_eq!(visits, [0, 1, 2]);
}

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Stopped(Stop),
    Read(usize),
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

#[test]
fn callback_failure_is_returned_without_later_reads() {
    let mut visits = Vec::new();
    let result = ExtremumRow {
        conditions: &[3, 5, 7],
        ranks: &[0, 2, 4],
        rank_count: 5,
    }
    .reduce(|condition| {
        visits.push(condition);
        if condition == 5 {
            Err(Failure::Read(condition))
        } else {
            Ok(true)
        }
    });
    assert_eq!(result, Err(Failure::Read(5)));
    assert_eq!(visits, [3, 5]);
}

#[test]
fn maximum_carrier_size_does_not_narrow_ranks() {
    let row = ExtremumRow {
        conditions: &[0, 1],
        ranks: &[0, u64::MAX - 1],
        rank_count: u64::MAX,
    };
    let measure = row.reduce(|_| Ok::<_, Stop>(true)).unwrap();
    assert_eq!(
        measure,
        ExtremaMeasure {
            minimum: 0,
            maximum: u64::MAX - 1
        }
    );
    assert!(ExtremumCutRule::Prefix(u64::MAX).holds(measure));
    assert!(!ExtremumCutRule::Suffix(u64::MAX).holds(measure));
    let empty = row.reduce(|_| Ok::<_, Stop>(false)).unwrap();
    assert_eq!(
        empty,
        ExtremaMeasure {
            minimum: u64::MAX - 1,
            maximum: 0
        }
    );
}
