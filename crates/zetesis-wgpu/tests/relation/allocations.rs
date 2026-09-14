use super::*;

fn spare_buffers(plan: &packing::Plan) -> [Vec<u32>; 3] {
    [plan.query_bytes, plan.equality_bytes, plan.mask_bytes].map(|bytes| {
        let length = usize::try_from(bytes / 4).unwrap();
        let values = packing::vector(length + 7).unwrap();
        assert!(values.capacity() > length);
        values
    })
}

fn peak(plan: &packing::Plan, buffers: &[Vec<u32>; 3]) -> u64 {
    plan.column_bytes
        + plan.transport_bytes
        + PARAM_BYTES
        + buffers
            .iter()
            .map(|buffer| u64::try_from(buffer.capacity()).unwrap() * 4)
            .sum::<u64>()
}

#[test]
fn retained_capacity_uses_the_inclusive_total_limit() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let value = Value::Number(1);
    let queries = [relation
        .query(&[(0, &value)], relation::Limits::default())
        .unwrap()];
    for deficit in [0, 1] {
        let mut plan = plan(&relation, &queries);
        let mut buffers = spare_buffers(&plan);
        let expected = peak(&plan, &buffers);
        assert!(expected > plan.accounted_bytes);
        let mut reservations = 0;
        let result = plan.pack_with(
            &queries,
            &Control::default(),
            expected - deficit,
            |length| {
                let buffer = std::mem::take(&mut buffers[reservations]);
                assert!(buffer.capacity() > length);
                reservations += 1;
                Ok(buffer)
            },
        );
        assert_eq!(reservations, 3);
        if deficit == 0 {
            let packed = result.unwrap();
            assert_eq!(plan.stats().accounted_bytes, expected);
            assert_eq!(packed.records.len(), 4);
            assert_eq!(packed.equalities.len(), 2);
            assert_eq!(packed.masks.len(), 2);
        } else {
            assert_eq!(result.err().unwrap().kind(), GpuErrorKind::Capacity);
        }
    }
}

#[test]
fn excess_capacity_stops_remaining_reservations() {
    let (predicate, atoms) = source(1);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let minimum = plan.accounted_bytes;
    let mut reservations = 0;
    let error = plan
        .pack_with(&queries, &Control::default(), minimum, |length| {
            reservations += 1;
            packing::vector(length + 1)
        })
        .err()
        .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert_eq!(reservations, 1);
    assert_eq!(plan.accounted_bytes, minimum);
}

#[test]
fn host_reservation_failure_remains_allocation() {
    let (predicate, atoms) = source(1);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    for failed in 0..3 {
        let mut plan = plan(&relation, &queries);
        let minimum = plan.accounted_bytes;
        let mut reservations = 0;
        let error = plan
            .pack_with(&queries, &Control::default(), u64::MAX, |length| {
                let requested = if reservations == failed {
                    usize::MAX
                } else {
                    length
                };
                reservations += 1;
                // A real non-ZST reservation rejects this unrepresentable layout;
                // no allocator OOM or particular supported-allocator slack is assumed.
                packing::vector(requested)
            })
            .err()
            .unwrap();
        assert_eq!(error.kind(), GpuErrorKind::Allocation);
        assert_eq!(reservations, failed + 1);
        assert_eq!(plan.accounted_bytes, minimum);
    }
}

#[test]
fn retained_payload_sums_refuse_overflow() {
    assert_eq!(
        packing::accounted_bytes(4, 100, [12, 20, 28], 196).unwrap(),
        196
    );
    for (columns, transport, host, limit) in [
        (4, 100, [12, 20, 28], 195),
        (0, 0, [0; 3], 0),
        (u64::MAX, 0, [0; 3], u64::MAX),
        (0, u64::MAX, [0; 3], u64::MAX),
        (0, 0, [u64::MAX, 1, 0], u64::MAX),
    ] {
        assert_eq!(
            packing::accounted_bytes(columns, transport, host, limit)
                .unwrap_err()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn decode_preserves_the_preallocated_mask_storage() {
    let (predicate, atoms) = source(3);
    let relation = Relation::from_catalog(
        &predicate,
        &atoms,
        &[2, 0, 2, 1],
        relation::Limits::default(),
    )
    .unwrap();
    let queries = [
        relation
            .query(&[(0, &Value::Number(2))], relation::Limits::default())
            .unwrap(),
        relation
            .query(&[(0, &Value::Number(0))], relation::Limits::default())
            .unwrap(),
    ];
    let mut plan = plan(&relation, &queries);
    // Each query scans 64 padded rows, 32 mask bits, one receipt, and
    // four row equalities: 101 work units, independently of packed records.
    let input = [
        0x434f_4c32,
        1,
        0,
        0,
        101,
        0b0101,
        0x434f_4c32,
        1,
        1,
        0,
        101,
        0b0010,
    ];
    let packed = plan
        .pack_with(&queries, &Control::default(), u64::MAX, |length| {
            packing::vector(length + 7)
        })
        .unwrap();
    let pointer = packed.masks.as_ptr();
    let capacity = packed.masks.capacity();
    let masks = plan
        .decode(
            &relation,
            &queries,
            &input,
            packed.masks,
            &Control::default(),
        )
        .unwrap();
    assert!(std::ptr::eq(masks.relation(), &raw const relation));
    assert_eq!(masks.words.as_ptr(), pointer);
    assert_eq!(masks.words.capacity(), capacity);
    assert_eq!(masks.words(0), Some([0b0101].as_slice()));
    assert_eq!(masks.words(1), Some([0b0010].as_slice()));
    let first = masks.selection(0, relation::Limits::default()).unwrap();
    assert_eq!(first.positions(), [0, 2]);
    assert_eq!(
        [
            first.row(0).unwrap().source_index(),
            first.row(1).unwrap().source_index()
        ],
        [2, 2]
    );
    let second = masks.selection(1, relation::Limits::default()).unwrap();
    assert_eq!(second.positions(), [1]);
    assert_eq!(second.row(0).unwrap().source_index(), 0);
}

#[test]
fn decode_refuses_an_incomplete_output_allocation() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![u32::MAX, 1]]);
    let mut packed = plan.pack(&queries, &Control::default(), u64::MAX).unwrap();
    packed.masks.pop();
    let error = plan
        .decode(
            &relation,
            &queries,
            &input,
            packed.masks,
            &Control::default(),
        )
        .err()
        .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Readback);
}

#[test]
fn cancelled_preparation_reserves_nothing() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let control = Control::default();
    control.cancel();
    let mut reservations = 0;
    let error = plan
        .pack_with(&queries, &control, u64::MAX, |length| {
            reservations += 1;
            packing::vector(length)
        })
        .err()
        .unwrap();
    assert!(matches!(
        RelationGpuError::from(error),
        RelationGpuError::Stopped(Stop::Cancelled)
    ));
    assert_eq!(reservations, 0);
}

#[test]
fn cancellation_after_reservation_stops_preparation() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let control = Control::default();
    let mut reservations = 0;
    let error = plan
        .pack_with(&queries, &control, u64::MAX, |length| {
            reservations += 1;
            let vector = packing::vector(length)?;
            control.cancel();
            Ok(vector)
        })
        .err()
        .unwrap();
    assert!(matches!(
        RelationGpuError::from(error),
        RelationGpuError::Stopped(Stop::Cancelled)
    ));
    assert_eq!(reservations, 1);
}
