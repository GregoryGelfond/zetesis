//! Explicit physical qualification of relation equality filtering.
//! These checks establish ordered equality masks, not full ASP pattern matching.

use crate::support::physical;

use zetesis_backend::GpuApi;
use zetesis_core::{
    Atom, Predicate, Sign, Value, ValueLimits, ValueNode,
    atom_interner::{AtomInterner, Limits as AtomLimits},
    relation::{Catalog, Limits, Relation},
};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_wgpu::{
    GpuOptions, GpuRelationExecutor, GpuSelection, RelationGpuActivity, RelationGpuError,
    RelationGpuLimits,
};

fn executor(backend: GpuApi) -> GpuRelationExecutor {
    let executor =
        GpuRelationExecutor::new_selected(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    physical::verify(backend, executor.info());
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
    let mut queries: Vec<_> = specifications
        .iter()
        .map(|items| {
            let keys: Vec<_> = items
                .iter()
                .map(|(column, value)| (*column, value.into()))
                .collect();
            relation.query(&keys, Limits::default()).unwrap()
        })
        .collect();
    let mut prepared = executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let input = relation.all(Limits::default()).unwrap();
    for round in 0..2 {
        if round != 0 {
            queries.reverse();
            specifications.reverse();
        }
        let masks = prepared
            .filter(
                &queries,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(masks.query_count(), queries.len());
        assert!(relation.same_owner(masks.relation()));
        assert_completed_tiles(
            &prepared.activity(),
            rows,
            queries.len(),
            queries.iter().map(|query| query.equalities().len()).sum(),
        );
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

fn assert_completed_tiles(
    activity: &RelationGpuActivity,
    rows: usize,
    queries: usize,
    equalities: usize,
) {
    let rows = u64::try_from(rows).unwrap();
    let queries = u64::try_from(queries).unwrap();
    let equalities = u64::try_from(equalities).unwrap();
    let tiles = rows.div_ceil(64);
    let words = rows.div_ceil(32);
    assert_eq!(activity.submissions, u64::from(rows != 0));
    assert_eq!(activity.submitted_queries, u64::from(rows != 0) * queries);
    assert_eq!(activity.completed_queries, activity.submitted_queries);
    assert_eq!(activity.submitted_workgroups, tiles * queries);
    assert_eq!(
        activity.scheduled_work,
        queries * (64 * tiles + 32 * words + tiles) + rows * equalities
    );
    assert_eq!(activity.completed_work, activity.scheduled_work);
    // COL2 returns one five-word receipt for every tile, then complete masks.
    assert_eq!(activity.downloaded_bytes, 4 * queries * (5 * tiles + words));
}

fn qualify_masks(backend: GpuApi) {
    let mut executor = executor(backend);
    for rows in [0, 1, 31, 32, 33, 63, 64, 65, 127, 128, 129] {
        compare_rows(&mut executor, rows);
    }
    compare_catalog_growth(&mut executor);
    let predicate = Predicate::new("nullary", 0).unwrap();
    let atoms = [Atom::new(predicate.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let queries = [relation.query(&[], Limits::default()).unwrap()];
    let mut prepared = executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let masks = prepared
        .filter(
            &queries,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        masks.selection(0, Limits::default()).unwrap().positions(),
        [0]
    );
    assert_eq!(prepared.activity().completed_queries, 1);
    let empty = prepared
        .filter(&[], RelationGpuLimits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(empty.query_count(), 0);
    assert_eq!(prepared.activity(), RelationGpuActivity::default());
}

fn compare_catalog_growth(executor: &mut GpuRelationExecutor) {
    // This fixture's finite typed values and 65 rows fit this explicit owner
    // allowance. It is independent of the relation metadata budget.
    const CANONICAL_BYTES: usize = 1 << 20;
    let predicate = Predicate::with_sign("row", 2, Sign::Negative).unwrap();
    let values = values();
    let mut authority = AtomInterner::new();
    let allowance = AtomLimits::for_atoms(65, CANONICAL_BYTES);
    let declaration = authority
        .declare_predicate_with(&predicate, allowance, || Ok::<_, ()>(()))
        .unwrap();
    let mut catalog = Catalog::new(authority.read(), declaration, Limits::default()).unwrap();
    // Insertion order deliberately differs from typed-value order. The second
    // snapshot reuses the first dictionary IDs and crosses two mask boundaries.
    for rows in [31, 65] {
        for row in catalog.len()..rows {
            let atom = Atom::new(
                predicate.clone(),
                vec![
                    Value::Number(-i32::try_from(row).unwrap()),
                    values[row % values.len()].clone(),
                ],
            )
            .unwrap();
            let id = authority
                .entry_atom_with(&atom, allowance, || Ok::<_, ()>(()))
                .unwrap()
                .insert_with(allowance, || Ok::<_, ()>(()))
                .unwrap();
            catalog
                .insert(authority.get(id).unwrap(), Limits::default())
                .unwrap();
        }
        let relation = catalog.view(authority.read()).unwrap();
        let queries: Vec<_> = values
            .iter()
            .map(|value| {
                relation
                    .query(&[(1, value.into())], Limits::default())
                    .unwrap()
            })
            .collect();
        let mut prepared = executor
            .prepare(
                &relation,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let masks = prepared
            .filter(
                &queries,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            prepared.activity().completed_queries,
            u64::try_from(values.len()).unwrap()
        );
        assert_eq!(prepared.activity().submissions, 1);
        for (query, value) in values.iter().enumerate() {
            let expected: Vec<_> = catalog
                .atoms(authority.read())
                .unwrap()
                .iter()
                .enumerate()
                .filter_map(|(position, atom)| {
                    (atom.values().at(1).unwrap() == *value).then_some(position)
                })
                .collect();
            let selection = masks.selection(query, Limits::default()).unwrap();
            assert_eq!(selection.positions(), expected);
        }
    }
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_relation_masks_match_typed_rows() {
    qualify_masks(GpuApi::Metal);
}

#[test]
#[ignore = "requires actual Vulkan GPU; explicit physical qualification"]
fn vulkan_relation_masks_match_typed_rows() {
    qualify_masks(GpuApi::Vulkan);
}

fn qualify_refusals(backend: GpuApi) {
    let mut executor = executor(backend);
    let predicate = Predicate::new("row", 0).unwrap();
    let atoms = [Atom::new(predicate.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let foreign = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let queries = [relation.query(&[], Limits::default()).unwrap()];
    let foreign_queries = [foreign.query(&[], Limits::default()).unwrap()];
    let mut prepared = executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(matches!(
        prepared.filter(
            &foreign_queries,
            RelationGpuLimits::default(),
            &Cancellation::default()
        ),
        Err(RelationGpuError::Relation(
            zetesis_core::relation::Failure::Owner
        ))
    ));
    assert_eq!(prepared.activity(), RelationGpuActivity::default());
    prepared
        .filter(
            &queries,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
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
            .filter(&queries, limits, &Cancellation::default())
            .err()
            .unwrap();
        assert!(
            matches!(error, RelationGpuError::Gpu(ref error) if error.kind() == zetesis_wgpu::GpuErrorKind::Capacity)
        );
        assert_eq!(prepared.activity(), RelationGpuActivity::default());
        assert!(prepared.last_stats().is_none());
    }
    let cancelled = Cancellation::default();
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
        .filter(&queries, exact, &Cancellation::default())
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
    qualify_refusals(GpuApi::Metal);
}

#[test]
#[ignore = "requires actual Vulkan GPU; explicit physical qualification"]
fn vulkan_relation_refusals_preserve_prepared_view() {
    qualify_refusals(GpuApi::Vulkan);
}
