use super::*;
use crate::test_support::location;
use std::hash::{BuildHasherDefault, Hasher};
use zetesis_ferraris::{ExtremumCut, Threshold};

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
    let mut entries: Vec<_> = bounds
        .iter()
        .copied()
        .map(|bound| Entry::new(Rule::Threshold(bound)))
        .collect();
    let mut pending = vec![0; bounds.len()];
    let mut table = vec![None; bounds.len() * 2 + 1];
    let mut matched_rows = vec![None; bounds.len() * 2 + 1];
    intern(
        index,
        nodes,
        Family {
            row: Row {
                conditions: &[0, 0],
                values: weights,
                domain: NumericDomain::Weights,
            },
            entries: &mut entries,
            pending: Pending::Weights(&mut pending),
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
        let mut entries: Vec<_> = (1..=64)
            .map(|bound| Entry::new(Rule::Threshold(bound)))
            .collect();
        let mut pending = vec![0; 64];
        let mut table = vec![None; 128];
        let mut matched_rows = vec![None; 128];
        let mut work = 0;
        intern(
            &mut index,
            &mut nodes,
            Family {
                row: Row {
                    conditions: &conditions,
                    values: &weights,
                    domain: NumericDomain::Weights,
                },
                entries: &mut entries,
                pending: Pending::Weights(&mut pending),
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

fn ordinal<S: BuildHasher>(
    index: &mut Index<S>,
    nodes: &mut FormulaNodes,
    row: ExtremumRow<'_>,
    rules: &[ExtremumCutRule],
    limits: (usize, usize),
    work: impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<Vec<Entry>, FormulaFailure> {
    let mut entries: Vec<_> = rules
        .iter()
        .copied()
        .map(|rule| Entry::new(Rule::Extremum(rule)))
        .collect();
    let mut pending = vec![ExtremumCutRule::Prefix(0); rules.len()];
    let mut table = vec![None; rules.len() * 2 + 1];
    let mut matched_rows = vec![None; rules.len() * 2 + 1];
    intern(
        index,
        nodes,
        Family {
            row: Row {
                conditions: row.conditions,
                values: row.ranks,
                domain: NumericDomain::Ordinal {
                    rank_count: row.rank_count,
                },
            },
            entries: &mut entries,
            pending: Pending::Ordinal(&mut pending),
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

fn ordinal_row() -> ExtremumRow<'static> {
    ExtremumRow {
        conditions: &[0, 0],
        ranks: &[1, 3],
        rank_count: 4,
    }
}

#[test]
fn collisions_distinguish_domain_carrier_rule_and_ordered_occurrences() {
    use ExtremumCutRule::{Prefix, Suffix};
    let mut index = CollisionIndex::default();
    let mut nodes = FormulaNodes::default();
    single(&mut index, &mut nodes, NodeView::Atom(0));
    family(&mut index, &mut nodes, &[1, 3], &[2], (20, 100), || Ok(())).unwrap();
    let rules = [Prefix(2), Prefix(2), Suffix(2), Prefix(0), Prefix(4)];
    let entries = ordinal(
        &mut index,
        &mut nodes,
        ordinal_row(),
        &rules,
        (20, 100),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.node, entry.inserted))
            .collect::<Vec<_>>(),
        [(2, true), (2, false), (3, true), (4, true), (5, true)]
    );
    assert_eq!(
        (nodes.parts().rows().len(), nodes.parts().conditions().len()),
        (2, 4)
    );
    assert_eq!(
        single(
            &mut index,
            &mut nodes,
            NodeView::ExtremumCut(ExtremumCut {
                row: ordinal_row(),
                rule: &Prefix(2)
            })
        ),
        (2, false)
    );
    for row in [
        ExtremumRow {
            rank_count: 5,
            ..ordinal_row()
        },
        ExtremumRow {
            ranks: &[3, 1],
            ..ordinal_row()
        },
    ] {
        let entries = ordinal(&mut index, &mut nodes, row, &[Prefix(2)], (20, 100), || {
            Ok(())
        })
        .unwrap();
        assert!(entries[0].inserted);
    }
    assert_eq!(
        (
            nodes.parts().rows().len(),
            nodes.parts().extremum_cuts().len()
        ),
        (4, 6)
    );
}

fn bounded_ordinal(
    limit: u128,
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
    let result = ordinal(
        &mut index,
        &mut nodes,
        ordinal_row(),
        &[
            ExtremumCutRule::Prefix(2),
            ExtremumCutRule::Suffix(3),
            ExtremumCutRule::Prefix(2),
        ],
        (20, 100),
        || {
            if spent == limit {
                return Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    observed: spent + 1,
                    limit,
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
fn every_ordinal_work_prefix_preserves_graph_and_index() {
    let (result, work, _, _) = bounded_ordinal(u128::MAX);
    result.unwrap();
    for limit in 0..work {
        let (result, spent, nodes, index) = bounded_ordinal(limit);
        assert!(result.is_err());
        assert_eq!(spent, limit);
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().rows().is_empty());
        assert!(nodes.parts().conditions().is_empty());
        assert!(nodes.parts().extremum_cuts().is_empty());
        assert_eq!((index.links.len(), index.buckets.len()), (1, 1));
    }
    let (result, spent, nodes, _) = bounded_ordinal(work);
    result.unwrap();
    assert_eq!(spent, work);
    assert_eq!(
        (
            nodes.view().len(),
            nodes.parts().rows().len(),
            nodes.parts().extremum_cuts().len()
        ),
        (3, 1, 2)
    );
}

#[test]
fn ordinal_admission_errors_keep_index_and_formula_lengths() {
    for (row, rule, expected) in [
        (
            ExtremumRow {
                ranks: &[1, 4],
                ..ordinal_row()
            },
            ExtremumCutRule::Prefix(0),
            AdmissionError::OrdinalRank,
        ),
        (
            ordinal_row(),
            ExtremumCutRule::Suffix(5),
            AdmissionError::OrdinalCut,
        ),
        (
            ExtremumRow {
                rank_count: 1,
                ..ordinal_row()
            },
            ExtremumCutRule::Prefix(0),
            AdmissionError::OrdinalRank,
        ),
    ] {
        let mut index = CollisionIndex::default();
        let mut nodes = FormulaNodes::default();
        single(&mut index, &mut nodes, NodeView::Atom(0));
        let result = ordinal(&mut index, &mut nodes, row, &[rule], (20, 100), || Ok(()));
        assert!(matches!(result, Err(FormulaFailure::Theory { error, .. }) if error == expected));
        assert_eq!(
            (nodes.view().len(), index.links.len(), index.buckets.len()),
            (1, 1, 1)
        );
        assert!(nodes.parts().rows().is_empty());
    }
}

#[test]
fn ordinal_family_dimensions_refuse_before_publication() {
    for (limits, expected) in [
        ((2, 100), FormulaResource::Nodes),
        ((20, 3), FormulaResource::Operands),
    ] {
        let mut index = CollisionIndex::default();
        let mut nodes = FormulaNodes::default();
        single(&mut index, &mut nodes, NodeView::Atom(0));
        let result = ordinal(
            &mut index,
            &mut nodes,
            ordinal_row(),
            &[ExtremumCutRule::Prefix(2), ExtremumCutRule::Suffix(3)],
            limits,
            || Ok(()),
        );
        assert!(
            matches!(result, Err(FormulaFailure::Limit { resource, .. }) if resource == expected)
        );
        assert_eq!(
            (nodes.view().len(), index.links.len(), index.buckets.len()),
            (1, 1, 1)
        );
        assert!(nodes.parts().extremum_cuts().is_empty());
    }
}

#[test]
fn existing_ordinal_family_compares_the_complete_row_once() {
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
    let ranks = vec![1; 512];
    let row = ExtremumRow {
        conditions: &conditions,
        ranks: &ranks,
        rank_count: 66,
    };
    let rules: Vec<_> = (1..=64).map(ExtremumCutRule::Prefix).collect();
    ordinal(
        &mut index,
        &mut nodes,
        row,
        &rules,
        (usize::MAX, usize::MAX),
        || Ok(()),
    )
    .unwrap();
    let mut work = 0;
    let reused = ordinal(
        &mut index,
        &mut nodes,
        row,
        &rules,
        (usize::MAX, usize::MAX),
        || {
            work += 1;
            Ok(())
        },
    )
    .unwrap();
    assert!(reused.iter().all(|entry| !entry.inserted));
    assert!(
        work < 3 * conditions.len() + 20 * rules.len(),
        "shared row spent {work} visits"
    );
    assert_eq!(
        (nodes.parts().rows().len(), nodes.parts().conditions().len()),
        (1, 512)
    );
}
