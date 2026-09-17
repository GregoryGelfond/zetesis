//! Exact node lookup; the node sequence alone fixes dense identity and order.
//!
//! A key is a kind and two child identities the builder assigned densely, so
//! it carries nothing an input author chooses and needs no randomized hash: a
//! fixed multiplicative mix of the three words places it. No hash-table
//! traversal emits nodes or selects IDs. Expected lookup is constant table
//! work; collisions can require linear work and growth can rehash prior keys.
//! Hashes establish bucket placement only: the complete node key decides
//! identity.

use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, Hasher};

use themelios_base::span::Location;
use zetesis_ferraris::Node;

use crate::formula::ceiling;
use crate::{FormulaFailure, FormulaResource};

type Key = (u8, usize, usize);
pub(super) type Index = HashMap<Key, usize, BuildHasherDefault<NodeHasher>>;

/// Mixes the words of a node key by multiplication with an odd constant
/// after rotating the running value, the scheme of the Rust compiler's own
/// interner hash; the low bits of each word reach every bit of the result.
#[derive(Default)]
pub(super) struct NodeHasher(u64);

impl Hasher for NodeHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u8(&mut self, word: u8) {
        self.write_u64(u64::from(word));
    }
    fn write_usize(&mut self, word: usize) {
        self.write_u64(word as u64);
    }
    fn write_u64(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

pub(super) fn intern<S: BuildHasher>(
    index: &mut HashMap<Key, usize, S>,
    nodes: &mut Vec<Node>,
    node: Node,
    bound: (FormulaResource, usize),
    location: Location,
) -> Result<(usize, bool), FormulaFailure> {
    let key = key(node);
    if let Some(&id) = index.get(&key) {
        return Ok((id, false));
    }
    // HashMap::entry may reserve for a vacant key before yielding the entry.
    // Perform miss admission before any mutating table operation instead.
    ceiling(bound.0, nodes.len() as u128 + 1, bound.1 as u128, location)?;
    let id = nodes.len();
    nodes.push(node);
    index.insert(key, id);
    Ok((id, true))
}

fn key(node: Node) -> Key {
    match node {
        Node::False => (0, 0, 0),
        Node::Atom(atom) => (1, atom, 0),
        Node::And(left, right) => (2, left, right),
        Node::Or(left, right) => (3, left, right),
        Node::Implies(left, right) => (4, left, right),
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{BuildHasherDefault, Hasher};

    use themelios_base::source::SourceId;
    use themelios_base::span::{ByteOffset, Span};

    use super::*;

    #[derive(Default)]
    struct Collision;
    impl Hasher for Collision {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _: &[u8]) {}
    }
    fn location() -> Location {
        Location {
            source: SourceId::new(4),
            span: Span::empty(ByteOffset::new(2)),
        }
    }
    fn sequence() -> [Node; 6] {
        [
            Node::False,
            Node::Implies(0, 0),
            Node::Atom(0),
            Node::And(2, 2),
            Node::Or(2, 3),
            Node::Implies(4, 2),
        ]
    }
    #[test]
    fn collisions_preserve_first_node_identity() {
        let mut index = HashMap::<_, _, BuildHasherDefault<Collision>>::default();
        let mut nodes = Vec::new();
        for (id, node) in sequence().into_iter().enumerate() {
            assert_eq!(
                intern(
                    &mut index,
                    &mut nodes,
                    node,
                    (FormulaResource::Nodes, 6),
                    location()
                )
                .unwrap(),
                (id, true)
            );
        }
        for (id, node) in sequence().into_iter().enumerate().rev() {
            assert_eq!(
                intern(
                    &mut index,
                    &mut nodes,
                    node,
                    (FormulaResource::Nodes, 6),
                    location()
                )
                .unwrap(),
                (id, false)
            );
        }
        assert_eq!(nodes, sequence());
        assert_eq!(index.len(), sequence().len());
    }
    #[test]
    fn a_node_ceiling_preserves_both_owners() {
        let mut index = Index::default();
        let mut nodes = Vec::new();
        intern(
            &mut index,
            &mut nodes,
            Node::False,
            (FormulaResource::Nodes, 1),
            location(),
        )
        .unwrap();
        assert!(
            matches!(intern(&mut index, &mut nodes, Node::Atom(0), (FormulaResource::Nodes, 1), location()), Err(FormulaFailure::Limit { resource: FormulaResource::Nodes, observed: 2, limit: 1, location: found }) if found == location())
        );
        assert_eq!(nodes, [Node::False]);
        assert_eq!(index.len(), 1);
        assert_eq!(
            intern(
                &mut index,
                &mut nodes,
                Node::Atom(0),
                (FormulaResource::Nodes, 2),
                location()
            )
            .unwrap(),
            (1, true)
        );
    }
    #[test]
    fn zero_node_admission_allocates_no_index() {
        let mut index = Index::default();
        let mut nodes = Vec::new();
        assert!(
            intern(
                &mut index,
                &mut nodes,
                Node::False,
                (FormulaResource::Nodes, 0),
                location()
            )
            .is_err()
        );
        assert_eq!(index.capacity(), 0);
        assert_eq!(nodes.capacity(), 0);
    }

    #[test]
    fn refused_growth_preserves_index_capacity() {
        let mut index = Index::default();
        let mut nodes = Vec::new();
        intern(
            &mut index,
            &mut nodes,
            Node::False,
            (FormulaResource::Nodes, usize::MAX),
            location(),
        )
        .unwrap();
        while index.len() < index.capacity() {
            let node = Node::Atom(nodes.len());
            intern(
                &mut index,
                &mut nodes,
                node,
                (FormulaResource::Nodes, usize::MAX),
                location(),
            )
            .unwrap();
        }
        let capacity = index.capacity();
        let node_capacity = nodes.capacity();
        let before = nodes.clone();
        let proposed = Node::Atom(nodes.len());
        let mut control_index = index.clone();
        let mut control_nodes = nodes.clone();
        intern(
            &mut control_index,
            &mut control_nodes,
            proposed,
            (FormulaResource::Nodes, usize::MAX),
            location(),
        )
        .unwrap();
        assert!(
            control_index.capacity() > capacity,
            "control insertion actually grows the index"
        );
        let limit = nodes.len();
        assert!(
            intern(
                &mut index,
                &mut nodes,
                proposed,
                (FormulaResource::Nodes, limit),
                location()
            )
            .is_err()
        );
        assert_eq!(index.capacity(), capacity);
        assert_eq!(nodes.capacity(), node_capacity);
        assert_eq!(nodes, before);
        assert_eq!(index.len(), limit);
    }
}
