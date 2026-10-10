//! Exact semantic node lookup over the sole paired formula owner.
//!
//! Hash buckets link node IDs; keys retain no copied operand rows. Collision
//! checks borrow complete ordered operands from the owner, so arena offsets do
//! not establish identity. Node publication order alone assigns dense IDs.

use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};

use crate::ProgramSite;
use crate::formula::ceiling;
use crate::word_hash::WordHasher;
use crate::{FormulaFailure, FormulaResource};
use zetesis_ferraris::{FormulaNodes, NodeView};

mod family;
pub(super) use family::{Entry, Family, intern as intern_family};

const END: usize = usize::MAX;

pub(super) struct Index<S = BuildHasherDefault<WordHasher>> {
    hash: S,
    buckets: HashMap<u64, usize, BuildHasherDefault<WordHasher>>,
    links: Vec<usize>,
}
impl<S: Default> Default for Index<S> {
    fn default() -> Self {
        Self {
            hash: S::default(),
            buckets: HashMap::default(),
            links: Vec::new(),
        }
    }
}

pub(super) fn intern<S: BuildHasher>(
    index: &mut Index<S>,
    nodes: &mut FormulaNodes,
    node: NodeView<'_>,
    bounds: [(FormulaResource, usize); 2],
    location: ProgramSite,
    mut work: impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<(usize, bool), FormulaFailure> {
    let hash = hash(&index.hash, node, &mut work)?;
    let previous = index.buckets.get(&hash).copied().unwrap_or(END);
    let mut cursor = previous;
    while cursor != END {
        work()?;
        let old = nodes
            .view()
            .node(cursor)
            .map_err(|error| FormulaFailure::Theory { error, location })?;
        if equal(old, node, &mut work)? {
            return Ok((cursor, false));
        }
        cursor = index.links[cursor];
    }
    let id = nodes.view().len();
    ceiling(bounds[0].0, id as u128 + 1, bounds[0].1 as u128, location)?;
    let occurrences = match node {
        NodeView::And(row) | NodeView::Or(row) => row.len(),
        NodeView::Threshold(row) => row.conditions.len(),
        NodeView::Implies(_, _) => 2,
        NodeView::Atom(_) | NodeView::False => 0,
    };
    ceiling(
        bounds[1].0,
        nodes.parts().occurrences() as u128 + occurrences as u128,
        bounds[1].1 as u128,
        location,
    )?;
    // Charge child validation, arena copying and any moved index cells before
    // their growth. Both reservations precede authoritative graph publication.
    let copies: usize = match node {
        NodeView::And(row) | NodeView::Or(row) if row.len() >= 3 => 1,
        NodeView::Threshold(_) => 2,
        _ => 0,
    };
    let growth = usize::from(index.links.len() == index.links.capacity()) * index.links.len()
        + usize::from(index.buckets.len() == index.buckets.capacity()) * index.buckets.len();
    let mut publication = occurrences as u128 * (1 + copies) as u128 + growth as u128;
    if matches!(node, NodeView::Threshold(_)) {
        publication += nodes.parts().pending_numeric_validation_work();
    }
    for _ in 0..publication {
        work()?;
    }
    index
        .links
        .try_reserve(1)
        .map_err(|error| FormulaFailure::MetadataAllocation { error, location })?;
    index
        .buckets
        .try_reserve(1)
        .map_err(|error| FormulaFailure::MetadataAllocation { error, location })?;
    let mut transaction = nodes.transaction();
    let found = transaction
        .push(node, bounds[0].1, bounds[1].1)
        .map_err(|error| FormulaFailure::Theory { error, location })?;
    debug_assert_eq!(
        found, id,
        "builder normalizes empty/singleton groups before interning"
    );
    transaction.commit();
    index.links.push(previous);
    index.buckets.insert(hash, id);
    Ok((id, true))
}

fn hash<S: BuildHasher>(
    builder: &S,
    node: NodeView<'_>,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<u64, FormulaFailure> {
    if let NodeView::Threshold(row) = node {
        let row_hash = row_hash(builder, row.conditions, row.weights, work)?;
        return threshold_hash(builder, row_hash, row.bound, work);
    }
    let mut hasher = builder.build_hasher();
    visit(node, |word| {
        work()?;
        word.hash(&mut hasher);
        Ok(())
    })?;
    Ok(hasher.finish())
}

fn row_hash<S: BuildHasher>(
    builder: &S,
    conditions: &[usize],
    weights: &[u64],
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<u64, FormulaFailure> {
    let mut hasher = builder.build_hasher();
    work()?;
    conditions.len().hash(&mut hasher);
    work()?;
    weights.len().hash(&mut hasher);
    for (&condition, &weight) in conditions.iter().zip(weights) {
        work()?;
        condition.hash(&mut hasher);
        work()?;
        weight.hash(&mut hasher);
    }
    Ok(hasher.finish())
}

fn threshold_hash<S: BuildHasher>(
    builder: &S,
    row_hash: u64,
    bound: i128,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<u64, FormulaFailure> {
    let mut hasher = builder.build_hasher();
    work()?;
    5_usize.hash(&mut hasher);
    work()?;
    row_hash.hash(&mut hasher);
    work()?;
    bound.hash(&mut hasher);
    Ok(hasher.finish())
}

fn visit(
    node: NodeView<'_>,
    mut word: impl FnMut(usize) -> Result<(), FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let pair;
    let (kind, operands) = match node {
        NodeView::False => (0, &[][..]),
        NodeView::Atom(atom) => {
            pair = [atom, 0];
            (1, &pair[..1])
        }
        NodeView::And(row) => (2, row),
        NodeView::Or(row) => (3, row),
        NodeView::Implies(left, right) => {
            pair = [left, right];
            (4, &pair[..])
        }
        NodeView::Threshold(_) => unreachable!("numeric identity was visited above"),
    };
    word(kind)?;
    word(operands.len())?;
    for &child in operands {
        word(child)?;
    }
    Ok(())
}

fn equal(
    left: NodeView<'_>,
    right: NodeView<'_>,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<bool, FormulaFailure> {
    match (left, right) {
        (NodeView::Threshold(a), NodeView::Threshold(b)) => {
            if a.bound != b.bound
                || a.conditions.len() != b.conditions.len()
                || a.weights.len() != b.weights.len()
            {
                return Ok(false);
            }
            for ((a, aw), (b, bw)) in a
                .conditions
                .iter()
                .zip(a.weights)
                .zip(b.conditions.iter().zip(b.weights))
            {
                work()?;
                if a != b || aw != bw {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (NodeView::And(a), NodeView::And(b)) | (NodeView::Or(a), NodeView::Or(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                work()?;
                if a != b {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => {
            work()?;
            Ok(left == right)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hasher;
    use themelios_base::source::SourceId;
    use themelios_base::span::{ByteOffset, Span};

    #[derive(Default)]
    struct Collision;
    impl Hasher for Collision {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _: &[u8]) {}
    }
    fn location() -> ProgramSite {
        ProgramSite::source(themelios_base::span::Location {
            source: SourceId::new(4),
            span: Span::empty(ByteOffset::new(2)),
        })
    }
    fn insert<S: BuildHasher>(
        index: &mut Index<S>,
        nodes: &mut FormulaNodes,
        node: NodeView<'_>,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<(usize, bool), FormulaFailure> {
        intern(
            index,
            nodes,
            node,
            [
                (FormulaResource::Nodes, max_nodes),
                (FormulaResource::Operands, max_operands),
            ],
            location(),
            || Ok(()),
        )
    }
    #[test]
    fn collisions_preserve_complete_operand_identity() {
        let mut index = Index::<BuildHasherDefault<Collision>>::default();
        let mut nodes = FormulaNodes::default();
        for atom in 0..3 {
            insert(&mut index, &mut nodes, NodeView::Atom(atom), 6, 20).unwrap();
        }
        let first = insert(&mut index, &mut nodes, NodeView::And(&[0, 1, 2, 1]), 6, 20).unwrap();
        assert_eq!(first, (3, true));
        assert_eq!(
            insert(&mut index, &mut nodes, NodeView::And(&[0, 1, 2, 1]), 6, 20).unwrap(),
            (3, false)
        );
        assert_eq!(
            insert(&mut index, &mut nodes, NodeView::And(&[0, 1, 2, 0]), 6, 20).unwrap(),
            (4, true)
        );
        assert_eq!(
            insert(&mut index, &mut nodes, NodeView::Or(&[0, 1, 2, 1]), 6, 20).unwrap(),
            (5, true)
        );
        assert_eq!(nodes.parts().operands().len(), 12);
    }
    #[test]
    fn threshold_identity_includes_numeric_columns_and_bound() {
        use zetesis_ferraris::Threshold;
        let mut index = Index::<BuildHasherDefault<Collision>>::default();
        let mut nodes = FormulaNodes::default();
        insert(&mut index, &mut nodes, NodeView::Atom(0), 10, 30).unwrap();
        let conditions = [0, 0];
        let weights = [u64::MAX, 2];
        let threshold = Threshold {
            conditions: &conditions,
            weights: &weights,
            bound: i128::from(u64::MAX) + 1,
        };
        assert_eq!(
            insert(
                &mut index,
                &mut nodes,
                NodeView::Threshold(threshold),
                10,
                30
            )
            .unwrap(),
            (1, true)
        );
        assert_eq!(
            insert(
                &mut index,
                &mut nodes,
                NodeView::Threshold(threshold),
                10,
                30
            )
            .unwrap(),
            (1, false)
        );
        assert_eq!(
            insert(
                &mut index,
                &mut nodes,
                NodeView::Threshold(Threshold {
                    bound: threshold.bound + 1,
                    ..threshold
                }),
                10,
                30
            )
            .unwrap(),
            (2, true)
        );
        assert_eq!(
            insert(
                &mut index,
                &mut nodes,
                NodeView::Threshold(Threshold {
                    weights: &[u64::MAX - 1, 2],
                    ..threshold
                }),
                10,
                30
            )
            .unwrap(),
            (3, true)
        );
    }

    #[test]
    fn refusal_precedes_index_growth() {
        let mut index = Index::<BuildHasherDefault<Collision>>::default();
        let mut nodes = FormulaNodes::default();
        assert!(insert(&mut index, &mut nodes, NodeView::False, 0, 0).is_err());
        assert_eq!(index.buckets.capacity(), 0);
        assert_eq!(index.links.capacity(), 0);
        assert_eq!(nodes.parts().node_capacity(), 0);
        insert(&mut index, &mut nodes, NodeView::False, 2, 0).unwrap();
        let capacities = (
            index.links.capacity(),
            index.buckets.capacity(),
            nodes.parts().node_capacity(),
        );
        assert!(matches!(
            insert(&mut index, &mut nodes, NodeView::And(&[0, 0, 0]), 2, 2),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Operands,
                observed: 3,
                limit: 2,
                ..
            })
        ));
        assert_eq!(
            capacities,
            (
                index.links.capacity(),
                index.buckets.capacity(),
                nodes.parts().node_capacity()
            )
        );
        assert_eq!(nodes.view().len(), 1);
        assert!(nodes.parts().operands().is_empty());
        assert_eq!(
            insert(&mut index, &mut nodes, NodeView::And(&[0, 0, 0]), 2, 3).unwrap(),
            (1, true)
        );
    }
}
