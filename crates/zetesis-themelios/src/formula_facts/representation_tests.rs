//! Representation, admission-coordinate identity, and transactional reservation.

use super::{FactArena, FactBlock, Growth, Item, Items, Layout, LeasedReserve, Segment};
use crate::formula_support::components::{Predicate, Scalar};
use crate::formula_support::testing::{Fixture, prepare};
use crate::formula_support::{Counters, StorageLease};
use crate::{FormulaFailure, FormulaResource, ProgramSite, StatementId};
use zetesis_core::{Value, ValueLimits};

#[derive(Debug, PartialEq, Eq)]
struct FactReceipt {
    predicate: Predicate,
    rows: Vec<RowReceipt>,
}

#[derive(Debug, PartialEq, Eq)]
struct RowReceipt {
    cells: Vec<Scalar>,
    evidence: Vec<ProgramSite>,
}

fn receipt(block: FactBlock<'_>) -> FactReceipt {
    FactReceipt {
        predicate: block.predicate(),
        rows: block
            .rows()
            .map(|row| RowReceipt {
                cells: row.cells().collect(),
                evidence: row.evidence().to_vec(),
            })
            .collect(),
    }
}

fn receipts(items: &Items) -> Vec<Option<FactReceipt>> {
    items
        .iter()
        .map(|item| match item {
            Item::Rule(..) => None,
            Item::Facts(block) => Some(receipt(block)),
        })
        .collect()
}

fn values(prepared: &crate::formula::Preparation) -> Vec<Vec<Value>> {
    let mut counters = Counters::default();
    let components = prepared
        .catalog
        .component_view(&prepared.limits, &mut counters, prepared.location)
        .unwrap()
        .unwrap();
    prepared
        .program
        .items()
        .blocks()
        .flat_map(FactBlock::rows)
        .map(|row| {
            row.cells()
                .map(|cell| {
                    cell.get(components, &prepared.limits, &mut counters, row.location())
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap()
                })
                .collect()
        })
        .collect()
}

#[test]
fn intervals_resolve_source_occurrences_in_last_argument_fastest_order() {
    let prepared = prepare("p(-2..1, 4..6). ");
    let actual = values(&prepared);
    let expected: Vec<_> = (-2..=1)
        .flat_map(|left| (4..=6).map(move |right| vec![Value::Number(left), Value::Number(right)]))
        .collect();
    assert_eq!(actual, expected);
    let arena = &prepared.program.items().facts;
    assert_eq!(
        arena.segments.len(),
        3,
        "first column's initial coordinate is separated from its tail"
    );
    assert_eq!(
        arena
            .columns
            .iter()
            .map(|column| column.width)
            .collect::<Vec<_>>(),
        [4, 3]
    );
    assert_eq!(
        arena
            .segments
            .iter()
            .map(|segment| segment.len())
            .sum::<usize>(),
        7
    );
}

#[test]
fn interval_storage_depends_on_segments_instead_of_width() {
    let narrow = prepare("p(0..3). ");
    let wide = prepare("p(0..999). ");
    let narrow = &narrow.program.items().facts;
    let wide = &wide.program.items().facts;
    assert_eq!(narrow.segments.len(), 1);
    assert_eq!(wide.segments.len(), 1);
    assert_eq!(narrow.retained_bytes(), wide.retained_bytes());
    assert_eq!(wide.leased_bytes(), Some(wide.retained_bytes()));
}

#[test]
fn checked_spans_reject_numeric_or_occurrence_gaps() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    let mut fixture = Fixture::default();
    let zero = fixture.scalar(&Value::Number(0), location);
    let duplicate = fixture.scalar(&Value::Number(0), location);
    let one = fixture.scalar(&Value::Number(1), location);
    let two = fixture.scalar(&Value::Number(2), location);
    assert_ne!(
        zero, duplicate,
        "equal canonical numbers have separate occurrences"
    );
    let mut gap = Segment::integer(0, 0, zero);
    assert!(
        !gap.append(1, one),
        "a numeric successor is insufficient across a coordinate gap"
    );
    assert_eq!(gap.len(), 1);
    assert_eq!(gap.get(0), zero);
    let mut contiguous = Segment::integer(0, 0, duplicate);
    assert!(
        !contiguous.append(2, one),
        "a coordinate successor is insufficient across a numeric gap"
    );
    assert!(contiguous.append(1, one));
    assert!(contiguous.append(2, two));
    assert_eq!(
        (0..contiguous.len())
            .map(|index| contiguous.get(index))
            .collect::<Vec<_>>(),
        [duplicate, one, two]
    );
}

#[test]
fn integer_endpoints_resolve_without_overflow() {
    for (text, expected) in [
        ("p((-2147483647-1)..-2147483647).", [i32::MIN, i32::MIN + 1]),
        ("p(2147483646..2147483647).", [i32::MAX - 1, i32::MAX]),
    ] {
        let prepared = prepare(text);
        assert_eq!(
            values(&prepared),
            expected.map(|value| vec![Value::Number(value)])
        );
        assert_eq!(prepared.program.items().facts.segments.len(), 1);
    }
}

#[test]
fn runs_store_arguments_in_separate_columns_without_changing_rows() {
    let prepared = prepare("p(1,10). p(2,20). p(3,30). ");
    let arena = &prepared.program.items().facts;
    assert_eq!(arena.run_columns.len(), 2);
    assert!(arena.segments.is_empty());
    let block = prepared.program.items().blocks().next().unwrap();
    let rows: Vec<_> = block
        .rows()
        .map(|row| row.cells().collect::<Vec<_>>())
        .collect();
    for (position, column) in arena.run_columns.iter().enumerate() {
        assert_eq!(
            *column,
            rows.iter().map(|row| row[position]).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        values(&prepared),
        [
            vec![Value::Number(1), Value::Number(10)],
            vec![Value::Number(2), Value::Number(20)],
            vec![Value::Number(3), Value::Number(30)],
        ]
    );
    assert_eq!(arena.leased_bytes(), Some(arena.retained_bytes()));
}

/// Exact, accounted reservations, with a failure injected before one allocation.
struct Reserved {
    calls: usize,
    fail_at: Option<usize>,
    location: ProgramSite,
}

impl Reserved {
    fn new(fail_at: Option<usize>, location: ProgramSite) -> Self {
        Self {
            calls: 0,
            fail_at,
            location,
        }
    }
}

impl LeasedReserve for Reserved {
    fn reserve_leased<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
    ) -> Result<(), FormulaFailure> {
        let call = self.calls;
        self.calls += 1;
        if self.fail_at == Some(call) {
            return Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed: 1,
                limit: 0,
                location: self.location,
            });
        }
        let other = lease.bytes() - values.capacity() * size_of::<T>();
        values.try_reserve_exact(additional).unwrap();
        lease.observe(other + values.capacity() * size_of::<T>(), self.location)
    }
}

#[test]
fn a_failed_run_reservation_keeps_all_retained_buffers_leased() {
    let location = ProgramSite::statement(StatementId::new(0), None);
    // Six outer vectors, then each of the three independently allocated columns.
    for fail_at in 0..9 {
        let counters = Counters::default();
        let mut arena = FactArena::default();
        let mut reserved = Reserved::new(Some(fail_at), location);
        let result = arena
            .reserve(
                Growth {
                    blocks: 1,
                    run_columns: 3,
                    evidence: 1,
                    ends: 1,
                    ..Growth::default()
                },
                || counters.lease(),
                &mut reserved,
            )
            .and_then(|()| arena.reserve_row(None, 3, &mut reserved));
        assert!(result.is_err(), "allocation {fail_at}");
        assert_eq!(
            arena.leased_bytes(),
            Some(arena.retained_bytes()),
            "allocation {fail_at}"
        );
        assert!(arena.blocks.is_empty());
        assert!(arena.evidence.is_empty());
        assert!(arena.ends.is_empty());
        assert!(arena.run_columns.iter().all(Vec::is_empty));
        drop(arena);
        assert_eq!(counters.workspace_bytes(), 0);
    }
}

#[test]
fn a_failed_run_extension_publishes_no_partial_row() {
    for fail_at in 0..2 {
        let mut prepared = prepare("p(1,10). p(2,20). p(3,30). p(4,40). ");
        let before = receipts(prepared.program.items());
        let location = prepared.location;
        let arena = prepared.program.items_mut().arena();
        let mut reserved = Reserved::new(Some(fail_at), location);
        assert!(arena.reserve_row(Some(0), 2, &mut reserved).is_err());
        assert_eq!(arena.leased_bytes(), Some(arena.retained_bytes()));
        assert_eq!(receipts(prepared.program.items()), before);
    }
}

#[test]
fn splitting_preserves_product_spans_run_columns_and_evidence() {
    let mut prepared =
        prepare("a(-2..1,4..6). b(1,10). b(2,20). c(1..3). d. e(5,50). e(6,60). r(X):-c(X). ");
    let before = receipts(prepared.program.items());
    assert_eq!(before.len(), 6);
    let selected = [1, 1, 0, 1, 1, 0];
    let counters = Counters::default();
    let (mut moved_lease, mut kept_lease) = (counters.lease(), counters.lease());
    let mut reserved = Reserved::new(None, prepared.location);
    let source_bytes = prepared.program.items().arena_receipt();
    let room = prepared
        .program
        .items()
        .reserve_split(&selected, &mut reserved, &mut moved_lease, &mut kept_lease)
        .unwrap();
    let moved_bytes = room.moved.retained_bytes();
    assert_eq!(moved_lease.bytes() as u128, moved_bytes);
    let moved = prepared.program.split_items(&selected, room);
    assert_eq!(
        moved.retained_bytes(),
        moved_bytes,
        "copy uses only reserved capacities"
    );
    assert_eq!(moved.spare_room(), [0; 9]);
    let (mut expected_moved, mut expected_kept) = (Vec::new(), Vec::new());
    for (receipt, flag) in before.into_iter().zip(selected) {
        if flag == 0 {
            expected_kept.push(receipt);
        } else {
            expected_moved.push(receipt);
        }
    }
    assert_eq!(receipts(&moved), expected_moved);
    assert_eq!(receipts(prepared.program.items()), expected_kept);
    assert_eq!(
        prepared.program.items().arena_receipt(),
        source_bytes,
        "source buffers remain leased until their owner drops"
    );
    assert_eq!(
        moved.facts.leased_bytes(),
        None,
        "the moved sequence has its external lease"
    );
    assert!(matches!(
        moved.facts.blocks[0].layout,
        Layout::Product { .. }
    ));
}

#[test]
fn a_failed_split_reservation_moves_no_item() {
    // Establish the number of requests, then refuse each outer or nested
    // reservation in turn. The source is unchanged even after some destination
    // column buffers were successfully reserved.
    let prepared = prepare("a(1..3,4..5). b(1,10). b(2,20). c. r(X):-a(X,4). ");
    let selected = vec![1; prepared.program.items().len()];
    let before = receipts(prepared.program.items());
    let source_bytes = prepared.program.items().arena_receipt();
    let counters = Counters::default();
    let calls = {
        let (mut moved, mut kept) = (counters.lease(), counters.lease());
        let mut reserved = Reserved::new(None, prepared.location);
        prepared
            .program
            .items()
            .reserve_split(&selected, &mut reserved, &mut moved, &mut kept)
            .unwrap();
        reserved.calls
    };
    for fail_at in 0..calls {
        let (mut moved, mut kept) = (counters.lease(), counters.lease());
        let mut reserved = Reserved::new(Some(fail_at), prepared.location);
        assert!(
            prepared
                .program
                .items()
                .reserve_split(&selected, &mut reserved, &mut moved, &mut kept)
                .is_err()
        );
        assert_eq!(receipts(prepared.program.items()), before);
        assert_eq!(prepared.program.items().arena_receipt(), source_bytes);
        drop((moved, kept));
        assert_eq!(counters.workspace_bytes(), 0);
    }
}

#[test]
fn compact_column_reader_resolves_every_stored_occurrence() {
    for text in ["p(-2..1, 4..6).", "p(1,10). p(2,20). p(3,30).", "p(a;b,c)."] {
        let prepared = prepare(text);
        for block in prepared.program.items().blocks() {
            let columns: Vec<Vec<Scalar>> = block
                .columns()
                .map(|column| {
                    column
                        .segments()
                        .flat_map(|segment| {
                            let (scalar, span) = match segment {
                                super::ColumnSegment::Scalar(scalar) => (Some(scalar), None),
                                super::ColumnSegment::Integers { scalars, .. } => {
                                    (None, Some(scalars))
                                }
                            };
                            scalar.into_iter().chain(span.into_iter().flat_map(|span| {
                                (0..span.len()).map(move |offset| span.get(offset))
                            }))
                        })
                        .collect()
                })
                .collect();
            assert_eq!(columns.len(), block.arity());
            match &block.block().layout {
                Layout::Product {
                    columns: stored, ..
                } => {
                    for (actual, index) in columns.iter().zip(stored.clone()) {
                        let source = &block.arena.columns[index];
                        assert_eq!(
                            *actual,
                            (0..source.width)
                                .map(|offset| { source.cell(&block.arena.segments, offset) })
                                .collect::<Vec<_>>()
                        );
                    }
                }
                Layout::Run { .. } => {
                    let rows: Vec<Vec<Scalar>> =
                        block.rows().map(|row| row.cells().collect()).collect();
                    for (argument, actual) in columns.iter().enumerate() {
                        assert_eq!(
                            *actual,
                            rows.iter().map(|row| row[argument]).collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn compact_column_reader_keeps_integer_hulls_and_source_coordinates() {
    let prepared = prepare("p(-2..1, 4..6).");
    let block = prepared.program.items().blocks().next().unwrap();
    let mut counters = Counters::default();
    let components = prepared
        .catalog
        .component_view(&prepared.limits, &mut counters, prepared.location)
        .unwrap()
        .unwrap();
    let mut intervals = 0;
    for column in block.columns() {
        for segment in column.segments() {
            if let super::ColumnSegment::Integers { first, scalars } = segment {
                intervals += 1;
                for offset in 0..scalars.len() {
                    let actual = scalars
                        .get(offset)
                        .get(
                            components,
                            &prepared.limits,
                            &mut counters,
                            block.location(),
                        )
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap();
                    assert_eq!(
                        actual,
                        Value::Number(
                            i32::try_from(i64::from(first) + i64::try_from(offset).unwrap())
                                .unwrap()
                        )
                    );
                }
            }
        }
    }
    assert_eq!(
        intervals, 3,
        "the first column's occurrence gap remains explicit"
    );
}
