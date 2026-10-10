use super::*;
use crate::test_support::location;
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
struct Collision;
impl Hasher for Collision {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}

type CollisionIndex = Index<BuildHasherDefault<Collision>>;

fn single(
    index: &mut CollisionIndex,
    nodes: &mut FormulaNodes,
    node: NodeView<'_>,
) -> (usize, bool) {
    super::super::intern(
        index,
        nodes,
        node,
        [
            (FormulaResource::Nodes, 20),
            (FormulaResource::Operands, 100),
        ],
        location(),
        || Ok(()),
    )
    .unwrap()
}

fn family(
    index: &mut CollisionIndex,
    nodes: &mut FormulaNodes,
    weights: &[u64],
    bounds: &[i128],
    limits: (usize, usize),
    work: impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<Vec<Entry>, FormulaFailure> {
    let mut entries: Vec<_> = bounds.iter().copied().map(Entry::new).collect();
    let mut pending = vec![0; bounds.len()];
    let mut table = vec![None; bounds.len() * 2 + 1];
    let mut matched_rows = vec![None; bounds.len() * 2 + 1];
    intern(
        index,
        nodes,
        Family {
            conditions: &[0, 0],
            weights,
            entries: &mut entries,
            pending: &mut pending,
            table: &mut table,
            matched_rows: &mut matched_rows,
        },
        [
            (FormulaResource::Nodes, limits.0),
            (FormulaResource::Operands, limits.1),
        ],
        location(),
        work,
    )?;
    Ok(entries)
}

#[test]
fn family_collisions_preserve_canonical_numeric_identity() {
    let mut index = CollisionIndex::default();
    let mut nodes = FormulaNodes::default();
    single(&mut index, &mut nodes, NodeView::Atom(0));
    single(
        &mut index,
        &mut nodes,
        NodeView::Threshold(Threshold {
            conditions: &[0, 0],
            weights: &[2, 3],
            bound: 2,
        }),
    );
    let entries = family(
        &mut index,
        &mut nodes,
        &[2, 3],
        &[-1, 2, 2, 5],
        (20, 100),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.node, entry.inserted))
            .collect::<Vec<_>>(),
        [(2, true), (1, false), (1, false), (3, true)]
    );
    assert_eq!(
        nodes.parts().rows().len(),
        2,
        "one new row for both absent bounds"
    );
    assert_eq!(nodes.parts().conditions().len(), 4);
    assert_eq!(
        single(
            &mut index,
            &mut nodes,
            NodeView::Threshold(Threshold {
                conditions: &[0, 0],
                weights: &[2, 3],
                bound: 5,
            })
        ),
        (3, false)
    );
    let other = family(&mut index, &mut nodes, &[3, 2], &[2], (20, 100), || Ok(())).unwrap();
    assert_eq!((other[0].node, other[0].inserted), (4, true));
}

fn bounded(
    work_limit: u128,
) -> (
    Result<Vec<Entry>, FormulaFailure>,
    u128,
    FormulaNodes,
    CollisionIndex,
) {
    let mut index = CollisionIndex::default();
    let mut nodes = FormulaNodes::default();
    single(&mut index, &mut nodes, NodeView::Atom(0));
    let mut spent = 0;
    let result = family(
        &mut index,
        &mut nodes,
        &[2, 3],
        &[1, 4, 1],
        (20, 100),
        || {
            if spent == work_limit {
                return Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    observed: spent + 1,
                    limit: work_limit,
                    location: location(),
                });
            }
            spent += 1;
            Ok(())
        },
    );
    (result, spent, nodes, index)
}

#[test]
fn stopped_family_publishes_neither_graph_nor_index() {
    let (complete, work, _, _) = bounded(u128::MAX);
    complete.unwrap();
    for limit in 0..work {
        let (result, spent, nodes, index) = bounded(limit);
        assert!(result.is_err());
        assert_eq!(spent, limit);
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().rows().is_empty());
        assert_eq!(index.links.len(), 1);
        assert_eq!(index.buckets.len(), 1);
    }
    let (complete, spent, nodes, _) = bounded(work);
    complete.unwrap();
    assert_eq!(spent, work);
    assert_eq!(nodes.view().len(), 3);
    assert_eq!(nodes.parts().conditions(), [0, 0]);
    assert_eq!(nodes.parts().rows().len(), 1);
}

#[test]
fn family_dimensions_refuse_before_publication() {
    for (limits, resource) in [
        ((2, 100), FormulaResource::Nodes),
        ((20, 3), FormulaResource::Operands),
    ] {
        let mut index = CollisionIndex::default();
        let mut nodes = FormulaNodes::default();
        single(&mut index, &mut nodes, NodeView::Atom(0));
        let result = family(&mut index, &mut nodes, &[2, 3], &[1, 4], limits, || Ok(()));
        assert!(
            matches!(result, Err(FormulaFailure::Limit { resource: actual, .. }) if actual == resource)
        );
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().rows().is_empty());
        assert_eq!(index.links.len(), 1);
    }
}

#[test]
fn existing_family_compares_its_complete_row_once() {
    let mut index: Index = Index::default();
    let mut nodes = FormulaNodes::default();
    let limits = [
        (FormulaResource::Nodes, usize::MAX),
        (FormulaResource::Operands, usize::MAX),
    ];
    super::super::intern(
        &mut index,
        &mut nodes,
        NodeView::Atom(0),
        limits,
        location(),
        || Ok(()),
    )
    .unwrap();
    let conditions = vec![0; 512];
    let weights = vec![1; 512];
    for pass in 0..2 {
        let mut entries: Vec<_> = (1..=64).map(Entry::new).collect();
        let mut pending = vec![0; 64];
        let mut table = vec![None; 128];
        let mut matched_rows = vec![None; 128];
        let mut work = 0;
        intern(
            &mut index,
            &mut nodes,
            Family {
                conditions: &conditions,
                weights: &weights,
                entries: &mut entries,
                pending: &mut pending,
                table: &mut table,
                matched_rows: &mut matched_rows,
            },
            limits,
            location(),
            || {
                work += 1;
                Ok(())
            },
        )
        .unwrap();
        if pass == 1 {
            assert!(entries.iter().all(|entry| !entry.inserted));
            assert_eq!(matched_rows.iter().filter(|row| row.is_some()).count(), 1);
            assert!(
                work < 3 * conditions.len() + 20 * entries.len(),
                "shared-row lookup spent {work} operations"
            );
        }
    }
    assert_eq!(nodes.parts().conditions().len(), conditions.len());
    assert_eq!(nodes.parts().rows().len(), 1);
}
