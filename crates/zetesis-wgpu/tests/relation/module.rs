use super::*;
use zetesis_core::{Atom, Predicate, Value};

mod allocations;

#[test]
fn relation_dispatch_requires_a_positive_wait() {
    for rows in [0, 1] {
        let (predicate, atoms) = source(rows);
        let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
        let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
        for count in [0, 1] {
            for timeout in [Duration::ZERO, Duration::from_nanos(1)] {
                let result = packing::Plan::new(
                    &relation,
                    &queries[..count],
                    RelationGpuLimits {
                        timeout,
                        ..RelationGpuLimits::default()
                    },
                    &wgpu::Limits::default(),
                    1,
                );
                let dispatches = rows != 0 && count != 0;
                assert_eq!(result.is_ok(), !dispatches || !timeout.is_zero());
                if let Err(error) = result {
                    assert_eq!(error.kind(), GpuErrorKind::Capacity);
                }
            }
        }
    }
}

fn source(rows: usize) -> (Predicate, Vec<Atom>) {
    let predicate = Predicate::new("row", 1).unwrap();
    let atoms = (0..rows)
        .map(|row| {
            Atom::new(
                predicate.clone(),
                vec![Value::Number(i32::try_from(row % 3).unwrap())],
            )
            .unwrap()
        })
        .collect();
    (predicate, atoms)
}

fn plan(relation: &Relation<'_>, queries: &[relation::Query<'_, '_>]) -> packing::Plan {
    packing::Plan::new(
        relation,
        queries,
        RelationGpuLimits::default(),
        &wgpu::Limits::default(),
        1,
    )
    .unwrap()
}

// Synthetic receipts test decoding only. Actual device execution is established
// separately by the physical tests and independent typed row reference.
fn records(
    plan: &mut packing::Plan,
    queries: &[relation::Query<'_, '_>],
    masks: &[Vec<u32>],
) -> Vec<u32> {
    let packed = plan.pack(queries, &Control::default(), u64::MAX).unwrap();
    masks
        .iter()
        .enumerate()
        .flat_map(|(index, mask)| {
            let mut record = vec![
                RECEIPT_MARKER,
                1,
                u32::try_from(index).unwrap(),
                packed.records[index * 4 + 3],
            ];
            record.extend_from_slice(mask);
            record
        })
        .collect()
}

fn output(plan: &packing::Plan) -> Vec<u32> {
    vec![0; usize::try_from(plan.mask_bytes / 4).unwrap()]
}

#[test]
fn relation_shader_uses_only_portable_capabilities() {
    let module = naga::front::wgsl::parse_str(SHADER).expect("relation WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("uniform barriers and guarded row access validate");
    let entry = module
        .entry_points
        .iter()
        .find(|entry| entry.name == "select_rows")
        .unwrap();
    assert_eq!(entry.workgroup_size, [ROWS_PER_GROUP, 1, 1]);
}

#[test]
fn mask_reconstruction_preserves_catalog_occurrences() {
    let (predicate, atoms) = source(3);
    let indices = [2, 0, 2, 1];
    let relation =
        Relation::from_catalog(&predicate, &atoms, &indices, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![0b1101]]);
    let result = plan
        .decode(
            &relation,
            &queries,
            &input,
            output(&plan),
            &Control::default(),
        )
        .unwrap();
    let selection = result.selection(0, relation::Limits::default()).unwrap();
    assert_eq!(selection.positions(), [0, 2, 3]);
    let catalog: Vec<_> = (0..selection.positions().len())
        .map(|index| selection.row(index).unwrap().source_index())
        .collect();
    assert_eq!(catalog, [2, 2, 1]);
}

#[test]
fn decoding_requires_the_complete_query_population() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [
        relation.query(&[], relation::Limits::default()).unwrap(),
        relation.query(&[], relation::Limits::default()).unwrap(),
    ];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![u32::MAX, 1], vec![0, 0]]);
    for length in [0, 1, input.len() - 1] {
        assert!(
            plan.decode(
                &relation,
                &queries,
                &input[..length],
                output(&plan),
                &Control::default()
            )
            .is_err()
        );
    }
    assert!(
        plan.decode(
            &relation,
            &queries[..1],
            &input,
            output(&plan),
            &Control::default()
        )
        .is_err()
    );
    let result = plan
        .decode(
            &relation,
            &queries,
            &input,
            output(&plan),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(result.query_count(), 2);
    assert_eq!(result.words(0), Some([u32::MAX, 1].as_slice()));
    assert_eq!(result.words(1), Some([0, 0].as_slice()));
    assert_eq!(result.words(2), None);
}

#[test]
fn query_receipts_require_each_expected_field() {
    let (predicate, atoms) = source(1);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![1]]);
    for field in 0..RECEIPT_WORDS as usize {
        let mut changed = input.clone();
        changed[field] ^= 1;
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
fn nonzero_mask_padding_is_refused() {
    for rows in [1, 31, 33, 63, 65] {
        let (predicate, atoms) = source(rows);
        let relation =
            Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
        let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
        let mut plan = plan(&relation, &queries);
        let mut mask = vec![0; rows.div_ceil(32)];
        *mask.last_mut().unwrap() = 1 << (rows % 32);
        let input = records(&mut plan, &queries, &[mask]);
        assert_eq!(
            plan.decode(
                &relation,
                &queries,
                &input,
                output(&plan),
                &Control::default()
            )
            .err()
            .unwrap()
            .kind(),
            GpuErrorKind::Readback
        );
    }
}

#[test]
fn row_tiles_preserve_mask_word_boundaries() {
    for rows in [31, 32, 33, 63, 64, 65] {
        let (predicate, atoms) = source(rows);
        let relation =
            Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
        let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
        let plan = plan(&relation, &queries);
        assert_eq!(plan.words as usize, rows.div_ceil(32));
        assert_eq!(
            plan.workgroups,
            [u32::try_from(rows.div_ceil(64)).unwrap(), 1, 1]
        );
    }
}

#[test]
fn all_dispatch_axes_observe_the_device_ceiling() {
    assert_eq!(packing::workgroups([7, 1, 1], 7).unwrap(), 7);
    assert_eq!(packing::workgroups([7, 3, 1], 7).unwrap(), 21);
    for axis in 0..3 {
        let mut groups = [1; 3];
        groups[axis] = 8;
        assert!(packing::workgroups(groups, 7).is_err());
    }
    assert!(packing::workgroups([u32::MAX; 3], u32::MAX).is_err());
}

#[test]
fn mask_scans_consume_work_for_empty_selections() {
    let (predicate, atoms) = source(65);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![0, 0, 0]]);
    let result = plan
        .decode(
            &relation,
            &queries,
            &input,
            output(&plan),
            &Control::default(),
        )
        .unwrap();
    assert!(
        result
            .selection(
                0,
                relation::Limits {
                    max_work: 5,
                    ..relation::Limits::default()
                }
            )
            .is_err()
    );
    assert!(
        result
            .selection(
                0,
                relation::Limits {
                    max_work: 6,
                    ..relation::Limits::default()
                }
            )
            .unwrap()
            .positions()
            .is_empty()
    );
}

#[test]
fn batch_budgets_are_inclusive() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let value = Value::Number(1);
    let queries = [relation
        .query(&[(0, &value)], relation::Limits::default())
        .unwrap()];
    let reference = plan(&relation, &queries);
    let limits = RelationGpuLimits {
        max_bytes: reference.accounted_bytes,
        max_work: reference.work,
        ..RelationGpuLimits::default()
    };
    assert!(packing::Plan::new(&relation, &queries, limits, &wgpu::Limits::default(), 1).is_ok());
    assert!(
        packing::Plan::new(
            &relation,
            &queries,
            RelationGpuLimits {
                max_bytes: limits.max_bytes - 1,
                ..limits
            },
            &wgpu::Limits::default(),
            1
        )
        .is_err()
    );
    assert!(
        packing::Plan::new(
            &relation,
            &queries,
            RelationGpuLimits {
                max_work: limits.max_work - 1,
                ..limits
            },
            &wgpu::Limits::default(),
            1
        )
        .is_err()
    );
}

#[test]
fn reconstruction_budgets_include_live_row_capacity() {
    let (predicate, atoms) = source(33);
    let relation = Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let mut plan = plan(&relation, &queries);
    let input = records(&mut plan, &queries, &[vec![0b101, 1]]);
    let masks = plan
        .decode(
            &relation,
            &queries,
            &input,
            output(&plan),
            &Control::default(),
        )
        .unwrap();
    let selected = 3;
    let max_bytes = relation.storage().retained_bytes
        + size_of::<RelationGpuMasks<'_, '_>>()
        + masks.words.capacity() * size_of::<u32>()
        + size_of::<Selection<'_, '_>>()
        + selected * size_of::<usize>();
    let max_work = 2 * 2 + 2 * selected as u64;
    let limits = relation::Limits {
        max_bytes,
        max_work,
        ..relation::Limits::default()
    };
    let result = masks.selection(0, limits).unwrap();
    assert_eq!(result.positions(), [0, 2, 32]);
    assert_eq!(result.work(), u128::from(max_work));
    assert!(
        masks
            .selection(
                0,
                relation::Limits {
                    max_bytes: max_bytes - 1,
                    ..limits
                }
            )
            .is_err()
    );
    assert!(
        masks
            .selection(
                0,
                relation::Limits {
                    max_work: max_work - 1,
                    ..limits
                }
            )
            .is_err()
    );
}

#[test]
fn vacuous_batches_reserve_no_transport() {
    let predicate = Predicate::new("empty", 0).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], relation::Limits::default()).unwrap();
    let queries = [relation.query(&[], relation::Limits::default()).unwrap()];
    let plan = plan(&relation, &queries);
    assert_eq!(plan.queries, 1);
    assert_eq!(plan.workgroups, [0; 3]);
    assert_eq!(plan.transport_bytes, 0);
    assert_eq!(plan.work, 0);
}
