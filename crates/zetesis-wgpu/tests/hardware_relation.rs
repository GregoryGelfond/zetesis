//! Explicit physical qualification of relation equality filtering.
//! These checks establish ordered equality masks, not full ASP pattern matching.

#[path = "support/physical.rs"]
mod physical;

use zetesis_core::{
    Atom, Predicate, Sign, Value, ValueLimits, ValueNode,
    relation::{Catalog, Limits, Relation},
};
use zetesis_cpu::{Control, Stop};
use zetesis_wgpu::{
    GpuOptions, GpuRelationExecutor, RelationGpuActivity, RelationGpuError, RelationGpuLimits,
};

fn executor(backend: physical::Backend) -> GpuRelationExecutor {
    let executor =
        GpuRelationExecutor::new_selected(GpuOptions::default(), backend.selection()).unwrap();
    backend.verify(executor.info());
    executor
}

fn values() -> Vec<Value> {
    vec![
        Value::Number(0),
        Value::String("0".into()),
        Value::Symbol("zero".into()),
        Value::Infimum,
        Value::Supremum,
        Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(0)],
            ValueLimits::default(),
        )
        .unwrap(),
    ]
}

fn compare_rows(executor: &mut GpuRelationExecutor, rows: usize) {
    let predicate = Predicate::with_sign("row", 2, Sign::Negative).unwrap();
    let values = values();
    let atoms: Vec<_> = (0..rows)
        .map(|row| {
            Atom::new(
                predicate.clone(),
                vec![
                    values[row % values.len()].clone(),
                    values[(row / 3) % values.len()].clone(),
                ],
            )
            .unwrap()
        })
        .collect();
    // Reversed catalog indices and repeated source IDs remain original row
    // occurrences in this view; local mask positions are not catalog IDs.
    let indices: Vec<_> = (0..rows).map(|row| rows - 1 - (row / 2)).collect();
    let relation = Relation::from_catalog(&predicate, &atoms, &indices, Limits::default()).unwrap();
    let mut specifications = vec![Vec::new(), vec![(0, Value::Number(999))]];
    for value in &values {
        specifications.push(vec![(0, value.clone())]);
        specifications.push(vec![(0, value.clone()), (1, value.clone())]);
    }
    specifications.push(vec![(0, values[0].clone()), (0, values[1].clone())]);
    specifications.push(Vec::new());
    let queries: Vec<_> = specifications
        .iter()
        .map(|items| {
            let keys: Vec<_> = items
                .iter()
                .map(|(column, value)| (*column, value))
                .collect();
            relation.query(&keys, Limits::default()).unwrap()
        })
        .collect();
    let mut prepared = executor
        .prepare(&relation, RelationGpuLimits::default(), &Control::default())
        .unwrap();
    let input = relation.all(Limits::default()).unwrap();
    for _ in 0..2 {
        let masks = prepared
            .filter(&queries, RelationGpuLimits::default(), &Control::default())
            .unwrap();
        assert_eq!(masks.query_count(), queries.len());
        assert!(relation.same_owner(masks.relation()));
        let expected_submissions = u64::from(rows != 0);
        let activity = prepared.activity();
        assert_eq!(activity.submissions, expected_submissions);
        assert_eq!(
            activity.submitted_queries,
            expected_submissions * queries.len() as u64
        );
        assert_eq!(activity.completed_queries, activity.submitted_queries);
        assert_eq!(
            activity.submitted_workgroups,
            rows.div_ceil(64) as u64 * queries.len() as u64
        );
        assert_eq!(activity.completed_work, activity.scheduled_work);
        if rows != 0 {
            assert!(activity.downloaded_bytes > 0);
        }
        for (query, specification) in specifications.iter().enumerate() {
            let expected: Vec<_> = indices
                .iter()
                .enumerate()
                .filter_map(|(position, &source)| {
                    specification
                        .iter()
                        .all(|(column, value)| atoms[source].values().get(*column) == Some(value))
                        .then_some(position)
                })
                .collect();
            let selected = masks.selection(query, Limits::default()).unwrap();
            assert_eq!(selected.positions(), expected, "rows={rows} query={query}");
            let host = relation
                .select_mask(&queries[query], &input, Limits::default())
                .unwrap();
            assert_eq!(masks.words(query).unwrap(), host.words());
            for (offset, &position) in expected.iter().enumerate() {
                assert_eq!(
                    selected.row(offset).unwrap().source_index(),
                    indices[position]
                );
            }
        }
    }
}

fn qualify_masks(backend: physical::Backend) {
    let mut executor = executor(backend);
    for rows in [0, 1, 31, 32, 33, 63, 64, 65] {
        compare_rows(&mut executor, rows);
    }
    compare_catalog_growth(&mut executor);
    let predicate = Predicate::new("nullary", 0).unwrap();
    let atoms = [Atom::new(predicate.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let queries = [relation.query(&[], Limits::default()).unwrap()];
    let mut prepared = executor
        .prepare(&relation, RelationGpuLimits::default(), &Control::default())
        .unwrap();
    let masks = prepared
        .filter(&queries, RelationGpuLimits::default(), &Control::default())
        .unwrap();
    assert_eq!(
        masks.selection(0, Limits::default()).unwrap().positions(),
        [0]
    );
    assert_eq!(prepared.activity().completed_queries, 1);
    let empty = prepared
        .filter(&[], RelationGpuLimits::default(), &Control::default())
        .unwrap();
    assert_eq!(empty.query_count(), 0);
    assert_eq!(prepared.activity(), RelationGpuActivity::default());
}

fn compare_catalog_growth(executor: &mut GpuRelationExecutor) {
    let predicate = Predicate::with_sign("row", 2, Sign::Negative).unwrap();
    let values = values();
    let mut catalog = Catalog::new(predicate.clone(), Limits::default()).unwrap();
    // Insertion order deliberately differs from typed-value order. The second
    // snapshot reuses the first dictionary IDs and crosses two mask boundaries.
    for rows in [31, 65] {
        for row in catalog.atoms().len()..rows {
            let atom = Atom::new(
                predicate.clone(),
                vec![
                    Value::Number(-i32::try_from(row).unwrap()),
                    values[row % values.len()].clone(),
                ],
            )
            .unwrap();
            catalog.insert(atom, Limits::default()).unwrap();
        }
        let relation = catalog.view();
        let queries: Vec<_> = values
            .iter()
            .map(|value| relation.query(&[(1, value)], Limits::default()).unwrap())
            .collect();
        let mut prepared = executor
            .prepare(&relation, RelationGpuLimits::default(), &Control::default())
            .unwrap();
        let masks = prepared
            .filter(&queries, RelationGpuLimits::default(), &Control::default())
            .unwrap();
        assert_eq!(
            prepared.activity().completed_queries,
            u64::try_from(values.len()).unwrap()
        );
        assert_eq!(prepared.activity().submissions, 1);
        for (query, value) in values.iter().enumerate() {
            let expected: Vec<_> = catalog
                .atoms()
                .iter()
                .enumerate()
                .filter_map(|(position, atom)| (atom.values()[1] == *value).then_some(position))
                .collect();
            let selection = masks.selection(query, Limits::default()).unwrap();
            assert_eq!(selection.positions(), expected);
        }
    }
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_relation_masks_match_typed_rows() {
    qualify_masks(physical::Backend::Metal);
}

#[test]
#[ignore = "requires actual Vulkan GPU; explicit physical qualification"]
fn vulkan_relation_masks_match_typed_rows() {
    qualify_masks(physical::Backend::Vulkan);
}

fn qualify_refusals(backend: physical::Backend) {
    let mut executor = executor(backend);
    let predicate = Predicate::new("row", 0).unwrap();
    let atoms = [Atom::new(predicate.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let foreign = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let queries = [relation.query(&[], Limits::default()).unwrap()];
    let foreign_queries = [foreign.query(&[], Limits::default()).unwrap()];
    let mut prepared = executor
        .prepare(&relation, RelationGpuLimits::default(), &Control::default())
        .unwrap();
    assert!(matches!(
        prepared.filter(
            &foreign_queries,
            RelationGpuLimits::default(),
            &Control::default()
        ),
        Err(RelationGpuError::Relation(
            zetesis_core::relation::Failure::Owner
        ))
    ));
    assert_eq!(prepared.activity(), RelationGpuActivity::default());
    prepared
        .filter(&queries, RelationGpuLimits::default(), &Control::default())
        .unwrap();
    let bytes = prepared.last_stats().unwrap().accounted_bytes;
    let work = prepared.activity().completed_work;
    for limits in [
        RelationGpuLimits {
            timeout: std::time::Duration::ZERO,
            ..RelationGpuLimits::default()
        },
        RelationGpuLimits {
            max_queries: 0,
            ..RelationGpuLimits::default()
        },
        RelationGpuLimits {
            max_bytes: bytes - 1,
            ..RelationGpuLimits::default()
        },
        RelationGpuLimits {
            max_work: work - 1,
            ..RelationGpuLimits::default()
        },
    ] {
        let error = prepared
            .filter(&queries, limits, &Control::default())
            .err()
            .unwrap();
        assert!(
            matches!(error, RelationGpuError::Gpu(ref error) if error.kind() == zetesis_wgpu::GpuErrorKind::Capacity)
        );
        assert_eq!(prepared.activity(), RelationGpuActivity::default());
        assert!(prepared.last_stats().is_none());
    }
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(matches!(
        prepared.filter(
            &queries,
            RelationGpuLimits {
                timeout: std::time::Duration::ZERO,
                ..RelationGpuLimits::default()
            },
            &cancelled,
        ),
        Err(RelationGpuError::Stopped(Stop::Cancelled))
    ));
    let exact = RelationGpuLimits {
        max_queries: 1,
        max_bytes: bytes,
        max_work: work,
        ..RelationGpuLimits::default()
    };
    let masks = prepared
        .filter(&queries, exact, &Control::default())
        .unwrap();
    assert_eq!(
        masks.selection(0, Limits::default()).unwrap().positions(),
        [0]
    );
    assert_eq!(prepared.activity().completed_queries, 1);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_relation_refusals_preserve_prepared_view() {
    qualify_refusals(physical::Backend::Metal);
}

#[test]
#[ignore = "requires actual Vulkan GPU; explicit physical qualification"]
fn vulkan_relation_refusals_preserve_prepared_view() {
    qualify_refusals(physical::Backend::Vulkan);
}
