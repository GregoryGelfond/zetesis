//! Synthetic protocol controls; these do not qualify device mask semantics.

use super::*;

#[test]
fn tile_dimensions_preserve_the_uniform_word_layout() {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    let (_, dimensions) = module
        .types
        .iter()
        .find(|(_, value)| value.name.as_deref() == Some("Dimensions"))
        .unwrap();
    let naga::TypeInner::Struct { members, span } = &dimensions.inner else {
        panic!("relation dimensions are a uniform struct");
    };
    assert_eq!(u64::from(*span), PARAM_BYTES);
    let fields: Vec<_> = members
        .iter()
        .map(|member| (member.name.as_deref(), member.offset))
        .collect();
    assert_eq!(
        fields,
        [
            (Some("rows"), 0),
            (Some("columns"), 4),
            (Some("queries"), 8),
            (Some("words"), 12),
            (Some("epoch"), 16),
            (Some("tiles"), 20),
            (Some("reserved1"), 24),
            (Some("reserved2"), 28),
        ]
    );
}

#[test]
fn every_tile_receipt_requires_each_expected_field() {
    for rows in [63, 64, 65, 127, 128, 129] {
        let (predicate, atoms) = source(rows);
        let relation =
            Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
        let queries = [
            relation.query(&[], relation::Limits::default()).unwrap(),
            relation.query(&[], relation::Limits::default()).unwrap(),
        ];
        let mut plan = plan(&relation, &queries);
        let masks = [vec![0; rows.div_ceil(32)], vec![0; rows.div_ceil(32)]];
        let input = records(&mut plan, &queries, &masks);
        let valid = plan
            .decode(
                &relation,
                &queries,
                &input,
                output(&plan),
                &Control::default(),
            )
            .unwrap();
        assert_eq!(valid.query_count(), 2);
        let tiles = rows.div_ceil(64);
        let stride = tiles * 5 + rows.div_ceil(32);
        for query in 0..2 {
            for tile in 0..tiles {
                for field in 0..5 {
                    let mut changed = input.clone();
                    changed[query * stride + tile * 5 + field] ^= 1;
                    let error = plan
                        .decode(
                            &relation,
                            &queries,
                            &changed,
                            output(&plan),
                            &Control::default(),
                        )
                        .err()
                        .unwrap();
                    assert_eq!(error.kind(), GpuErrorKind::Readback);
                }
            }
        }
    }
}

#[test]
fn later_tiles_cannot_reuse_missing_or_foreign_receipts() {
    let (predicate, atoms) = source(129);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [
        relation.query(&[], relation::Limits::default()).unwrap(),
        relation.query(&[], relation::Limits::default()).unwrap(),
    ];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![0; 5], vec![0; 5]]);
    // Final tile of the final query: the first query has already decoded by
    // this point, but a failure still returns no public mask batch.
    let target = 20 + 10;
    for replacement in [
        [0; 5],
        [RECEIPT_MARKER, 0, 1, 2, 355],
        input[10..15].try_into().unwrap(),
        input[20..25].try_into().unwrap(),
    ] {
        let mut changed = input.clone();
        changed[target..target + 5].copy_from_slice(&replacement);
        let error = plan
            .decode(
                &relation,
                &queries,
                &changed,
                output(&plan),
                &Control::default(),
            )
            .err()
            .unwrap();
        assert_eq!(error.kind(), GpuErrorKind::Readback);
    }
    // Neither a removed tile nor an extra trailing receipt may disappear into
    // chunks_exact's remainder when all remaining identities are valid.
    for length in [input.len() - 5, input.len() + 5] {
        let mut changed = input.clone();
        changed.resize(length, 0);
        let error = plan
            .decode(
                &relation,
                &queries,
                &changed,
                output(&plan),
                &Control::default(),
            )
            .err()
            .unwrap();
        assert_eq!(error.kind(), GpuErrorKind::Readback);
    }
}

#[test]
fn tile_receipt_costs_are_inclusively_admitted() {
    // Two queries, one with one equality: work is
    // 2*(64*T + 32*W + T) + rows. Expected bytes include both device result
    // buffers, resident columns and the three minimum host vectors/parameters.
    for (rows, result, transport, accounted, work) in [
        (63, 56, 184, 524, 321),
        (64, 56, 184, 528, 322),
        (65, 104, 280, 636, 517),
        (127, 112, 296, 908, 643),
        (128, 112, 296, 912, 644),
        (129, 160, 392, 1020, 839),
    ] {
        let (predicate, atoms) = source(rows);
        let relation =
            Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
        let value = Value::Number(1);
        let queries = [
            relation.query(&[], relation::Limits::default()).unwrap(),
            relation
                .query(&[(0, &value)], relation::Limits::default())
                .unwrap(),
        ];
        let exact = RelationGpuLimits {
            max_bytes: accounted,
            max_work: work,
            ..RelationGpuLimits::default()
        };
        let plan = packing::Plan::new(&relation, &queries, exact, &wgpu::Limits::default(), 1)
            .unwrap();
        assert_eq!(plan.result_bytes, result);
        assert_eq!(plan.transport_bytes, transport);
        assert_eq!(plan.accounted_bytes, accounted);
        assert_eq!(plan.work, work);
        assert_eq!(plan.params()[5], u32::try_from(rows.div_ceil(64)).unwrap());
        for limits in [
            RelationGpuLimits {
                max_bytes: accounted - 1,
                ..exact
            },
            RelationGpuLimits {
                max_work: work - 1,
                ..exact
            },
        ] {
            assert_eq!(
                packing::Plan::new(&relation, &queries, limits, &wgpu::Limits::default(), 1)
                    .err()
                    .unwrap()
                    .kind(),
                GpuErrorKind::Capacity
            );
        }
    }
}

#[test]
fn older_epochs_cannot_fill_a_later_tile() {
    let (predicate, atoms) = source(129);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut first = plan(&relation, &queries);
    let mut input = records(&mut first, &queries, &[vec![0; 5]]);
    let next = packing::Plan::new(
        &relation,
        &queries,
        RelationGpuLimits::default(),
        &wgpu::Limits::default(),
        2,
    )
    .unwrap();
    for receipt in input[..15].chunks_exact_mut(5) {
        receipt[1] = 2;
    }
    next.decode(
        &relation,
        &queries,
        &input,
        output(&next),
        &Control::default(),
    )
    .unwrap();
    // The final tile retains the preceding invocation's otherwise valid record.
    input[11] = 1;
    assert_eq!(
        next.decode(
            &relation,
            &queries,
            &input,
            output(&next),
            &Control::default(),
        )
        .err()
        .unwrap()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn tile_receipts_follow_uniform_mask_completion() {
    let module = naga::front::wgsl::parse_str(SHADER).unwrap();
    let body = &module.entry_points[0].function.body;
    let barrier = body
        .iter()
        .position(|statement| {
            matches!(statement, naga::Statement::ControlBarrier(flags) if flags.contains(naga::Barrier::STORAGE))
        })
        .unwrap();
    let mut conditionals: Vec<_> = body
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            matches!(statement, naga::Statement::If { .. }).then_some(index)
        })
        .collect();
    // Naga also lowers short-circuit Boolean expressions into conditionals.
    // The final two are the mask writers and receipt writer. The barrier is
    // outside both guards, including on partial final tiles.
    let receipt = conditionals.pop().unwrap();
    let mask = conditionals.pop().unwrap();
    assert!(mask < barrier && barrier < receipt);
}
