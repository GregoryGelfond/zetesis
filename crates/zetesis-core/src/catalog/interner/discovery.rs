//! Sparse inverse of published discovery: local `AtomId` order, never term order.
//!
//! Each node number is a discovery position. Its key comes from the existing
//! committed/pending map, so canonical-only rows retained after refusal allocate
//! no inverse entries. Both AVL views publish with that map in one transaction.

use super::{AtomAppender, AtomId, Failure, Index, Limits, Node, Planned, Step};
use super::{cells, index, path_bound, position, reserve};

pub(super) fn identity(committed: &[AtomId], pending: &[AtomId], id: usize) -> Option<AtomId> {
    if id < committed.len() {
        committed.get(id)
    } else {
        pending.get(id - committed.len())
    }
    .copied()
}

pub(super) fn find<E>(
    index: &Index,
    committed: &[AtomId],
    pending: &[AtomId],
    atom: AtomId,
    before: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    index::search(
        &index.nodes,
        index.root,
        |id| {
            before()?;
            Ok(atom.cmp(&identity(committed, pending, id).expect("published discovery")))
        },
        |_| {},
    )
}

pub(super) struct Plan {
    pub nodes: Planned,
    pub increasing: bool,
}

impl AtomAppender<'_> {
    /// Prepare the inverse insertion after canonical admission, without changing
    /// its published links. The caller publishes both indexes only after every
    /// remaining permit succeeds. All retained growth uses the owner's receipt.
    pub(super) fn prepare_discovery<E>(
        &mut self,
        atom: AtomId,
        extra: u128,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Plan, Failure<E>> {
        let retained = self.spines.discovery.take();
        let id = self.len();
        let bound = path_bound(id.checked_add(1).ok_or(Failure::Overflow)?);
        let fixed = self.storage_bytes() + extra
            - cells::<Node>(self.discovery.nodes.capacity())
            - cells::<Step>(self.discovery.path.capacity());
        let mut checked = || before().map_err(Failure::Stopped);
        let live = fixed
            + cells::<Node>(self.discovery.nodes.capacity())
            + cells::<Step>(self.discovery.path.capacity());
        reserve(
            &mut self.discovery.nodes,
            1,
            limits.max_atoms,
            live,
            &mut self.index.peak,
            limits,
            &mut checked,
        )?;
        let reuse = if let Some(spine) = retained {
            checked()?;
            spine.matches(self.discovery, self.discovery.root, spine.last())
                && atom
                    > identity(self.committed, self.pending, spine.last())
                        .expect("published right-spine discovery")
        } else {
            false
        };
        let Index {
            nodes, path, root, ..
        } = &mut *self.discovery;
        if !reuse {
            path.clear();
        }
        let mut increasing = true;
        let mut cursor = if reuse { None } else { *root };
        while let Some(next) = cursor {
            checked()?;
            let position = position(next);
            let known =
                identity(self.committed, self.pending, position).expect("published discovery");
            let order = atom.cmp(&known);
            if order.is_eq() {
                return Err(Failure::Catalog(super::super::Error::Shape));
            }
            let node = nodes[position];
            let right = order.is_gt();
            increasing &= right;
            let live = fixed + cells::<Node>(nodes.capacity()) + cells::<Step>(path.capacity());
            reserve(
                path,
                1,
                bound,
                live,
                &mut self.index.peak,
                limits,
                &mut checked,
            )?;
            checked()?;
            path.push(Step {
                id: position,
                node,
                right,
                changed: false,
            });
            cursor = node.children[usize::from(right)];
        }
        let live = fixed + cells::<Node>(nodes.capacity()) + cells::<Step>(path.capacity());
        reserve(
            path,
            1,
            bound,
            live,
            &mut self.index.peak,
            limits,
            &mut checked,
        )?;
        checked()?;
        path.push(Step {
            id,
            node: Node::default(),
            right: false,
            changed: true,
        });
        self.discovery
            .plan_changes_from(self.discovery.root, id, &mut checked)
            .map(|nodes| Plan { nodes, increasing })
    }
}
