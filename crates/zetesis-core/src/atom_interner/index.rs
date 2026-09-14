//! An append-ID AVL tree. Paths own only tentative metadata; publication follows
//! every fallible comparison, reservation and work admission. Balance is right
//! height minus left height. Inserting one leaf causes at most one rotation;
//! only that search path and its parent link change. No payload is stored here.

use std::num::NonZeroUsize;

pub(super) type Link = Option<NonZeroUsize>;

pub(super) fn encoded(position: usize) -> NonZeroUsize {
    NonZeroUsize::new(position.checked_add(1).expect("admitted position")).expect("positive ID")
}

pub(super) fn position(value: NonZeroUsize) -> usize {
    value.get() - 1
}

#[derive(Clone, Copy, Default)]
pub(super) struct Node {
    pub children: [Link; 2],
    pub balance: i8,
}

#[derive(Clone, Copy)]
pub(super) struct Step {
    pub id: usize,
    pub node: Node,
    pub right: bool,
    pub changed: bool,
}

#[derive(Default)]
pub(super) struct Index {
    pub nodes: Vec<Node>,
    pub root: Link,
    pub path: Vec<Step>,
    pub peak: u128,
}

impl Index {
    /// Compute a new root and path metadata without changing any published link.
    /// The recorded search path ends at an absent child. The inserted leaf has
    /// height one; propagation stops once subtree height is unchanged.
    pub fn plan<E>(
        &mut self,
        id: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Link, E> {
        let mut child = Some(encoded(id));
        for level in (0..self.path.len() - 1).rev() {
            before()?;
            let step = &mut self.path[level];
            let side = usize::from(step.right);
            step.node.children[side] = child;
            step.node.balance += if step.right { 1 } else { -1 };
            step.changed = true;
            child = Some(encoded(step.id));
            if step.node.balance == 0 {
                return Ok(self.root);
            }
            if step.node.balance.abs() == 2 {
                child = self.rotate(level, before)?;
                if level == 0 {
                    return Ok(child);
                }
                before()?;
                let parent = &mut self.path[level - 1];
                parent.node.children[usize::from(parent.right)] = child;
                parent.changed = true;
                return Ok(self.root);
            }
        }
        Ok(child)
    }

    fn rotate<E>(
        &mut self,
        level: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Link, E> {
        // Before the first imbalance, the heavy child and (for a double
        // rotation) grandchild are the next two nodes in the insertion path.
        before()?;
        let mut root = self.path[level];
        let mut child = self.path[level + 1];
        let side = usize::from(root.right);
        let opposite = 1 - side;
        let sign = if root.right { 1 } else { -1 };
        if child.node.balance == sign {
            before()?;
            root.node.children[side] = child.node.children[opposite];
            child.node.children[opposite] = Some(encoded(root.id));
            root.node.balance = 0;
            child.node.balance = 0;
            self.path[level] = root;
            self.path[level + 1] = child;
            return Ok(Some(encoded(child.id)));
        }
        before()?;
        let mut pivot = self.path[level + 2];
        root.node.children[side] = pivot.node.children[opposite];
        child.node.children[opposite] = pivot.node.children[side];
        pivot.node.children[opposite] = Some(encoded(root.id));
        pivot.node.children[side] = Some(encoded(child.id));
        root.node.balance = if pivot.node.balance == sign { -sign } else { 0 };
        child.node.balance = if pivot.node.balance == -sign { sign } else { 0 };
        pivot.node.balance = 0;
        pivot.changed = true;
        self.path[level] = root;
        self.path[level + 1] = child;
        self.path[level + 2] = pivot;
        Ok(Some(encoded(pivot.id)))
    }

    /// All writes below are pre-admitted as one indivisible publication. A
    /// callback cannot observe a half-rotated tree or an unindexed payload.
    pub fn publish(&mut self, root: Link) {
        let leaf = self.path.last().expect("planned leaf");
        self.nodes.push(leaf.node);
        for step in &self.path[..self.path.len() - 1] {
            if step.changed {
                self.nodes[step.id] = step.node;
            }
        }
        self.root = root;
    }
}
