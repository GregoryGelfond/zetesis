//! An append-ID AVL tree. Paths own only tentative metadata; publication follows
//! every fallible comparison, reservation and work admission. Balance is right
//! height minus left height. Inserting one leaf causes at most one rotation;
//! only that search path and its parent link change. No payload is stored here.

use std::{cmp::Ordering, num::NonZeroUsize};

pub(crate) mod spine;

pub(crate) type Link = Option<NonZeroUsize>;

pub(crate) fn encoded(position: usize) -> NonZeroUsize {
    NonZeroUsize::new(position.checked_add(1).expect("admitted position")).expect("positive ID")
}

pub(crate) fn position(value: NonZeroUsize) -> usize {
    value.get() - 1
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Node {
    pub children: [Link; 2],
    pub balance: i8,
}

#[derive(Clone, Copy)]
pub(crate) struct Step {
    pub id: usize,
    pub node: Node,
    pub right: bool,
    pub changed: bool,
}

#[derive(Default)]
pub(crate) struct Index {
    pub nodes: Vec<Node>,
    pub root: Link,
    pub path: Vec<Step>,
    pub peak: u128,
}

/// Metadata already established by one successful insertion plan. The changed
/// suffix includes the new leaf; earlier path cells remain published unchanged.
#[derive(Clone, Copy)]
pub(crate) struct Planned {
    pub root: Link,
    pub changed_from: usize,
    pub rotation: Option<Rotation>,
}

#[derive(Clone, Copy)]
pub(crate) enum Rotation {
    Single(usize),
    Double,
}

impl Index {
    /// Plan against an unpublished root whose nodes were supplied in `path`.
    /// This supports several leaves in one bounded metadata transaction.
    pub fn plan_from<E>(
        &mut self,
        root: Link,
        id: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Link, E> {
        self.plan_changes_from(root, id, before)
            .map(|plan| plan.root)
    }

    /// Compute a new root and its actual changed suffix without changing any
    /// published link. The path ends at the absent child's new leaf. Propagation
    /// stops once subtree height is unchanged. A consumer reusing an all-right
    /// path must establish that property separately before invoking the planner.
    pub fn plan_changes_from<E>(
        &mut self,
        root: Link,
        id: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Planned, E> {
        let mut child = Some(encoded(id));
        let mut changed_from = self.path.len() - 1;
        for level in (0..self.path.len() - 1).rev() {
            before()?;
            let step = &mut self.path[level];
            let side = usize::from(step.right);
            step.node.children[side] = child;
            step.node.balance += if step.right { 1 } else { -1 };
            step.changed = true;
            changed_from = level;
            child = Some(encoded(step.id));
            if step.node.balance == 0 {
                return Ok(Planned {
                    root,
                    changed_from,
                    rotation: None,
                });
            }
            if step.node.balance.abs() == 2 {
                let (rotated, rotation) = self.rotate(level, before)?;
                child = rotated;
                if level == 0 {
                    return Ok(Planned {
                        root: child,
                        changed_from,
                        rotation: Some(rotation),
                    });
                }
                before()?;
                let parent = &mut self.path[level - 1];
                parent.node.children[usize::from(parent.right)] = child;
                parent.changed = true;
                return Ok(Planned {
                    root,
                    changed_from: level - 1,
                    rotation: Some(rotation),
                });
            }
        }
        Ok(Planned {
            root: child,
            changed_from,
            rotation: None,
        })
    }

    fn rotate<E>(
        &mut self,
        level: usize,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(Link, Rotation), E> {
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
            return Ok((Some(encoded(child.id)), Rotation::Single(level)));
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
        Ok((Some(encoded(pivot.id)), Rotation::Double))
    }

    /// Publish a suffix established by `plan_changes_from`; nodes before this
    /// boundary were not mutated. The owner pre-admits every write and publishes
    /// its root in the same indivisible transaction, without callbacks.
    pub fn publish_nodes_from(&mut self, changed_from: usize) {
        let leaf = self.path.last().expect("planned leaf");
        self.nodes.push(leaf.node);
        for step in &self.path[changed_from..self.path.len() - 1] {
            if step.changed {
                self.nodes[step.id] = step.node;
            }
        }
    }
}

/// Where a query stands against the last node in order, the maximum.
pub(crate) enum Last {
    /// The query equals the maximum, at this position.
    Found(usize),
    /// The query follows every node; its place is after the maximum.
    Beyond,
    /// The tree is empty, or the query precedes the maximum: the last node
    /// cannot place it, and the full search decides.
    Search,
}

/// Classify a query against the owner's last node before searching. A query
/// beyond that node is absent and its insertion path is the right spine. The
/// owner may prepare that path directly without first recording its directions.
/// This comparison reads no links and changes neither published nodes nor scratch.
pub(crate) fn compare_last<C, E>(
    root: Link,
    last: Option<usize>,
    context: &mut C,
    compare: impl FnOnce(usize, &mut C) -> Result<Ordering, E>,
) -> Result<Last, E> {
    let (Some(_), Some(last)) = (root, last) else {
        return Ok(Last::Search);
    };
    Ok(match compare(last, context)? {
        Ordering::Equal => Last::Found(last),
        Ordering::Less => Last::Search,
        Ordering::Greater => Last::Beyond,
    })
}

/// Search the owner's ordered identities without owning or copying any payload.
/// `compare` admits one node visit before comparing the query with the ID's
/// authoritative value. The callback's ordering must be the tree's ordering.
/// `descend` records only successful comparisons; an error publishes no result
/// and changes no node. A miss visits at most the AVL height in nodes.
pub(crate) fn search<E>(
    nodes: &[Node],
    mut cursor: Link,
    mut compare: impl FnMut(usize) -> Result<Ordering, E>,
    mut descend: impl FnMut(bool),
) -> Result<Option<usize>, E> {
    while let Some(next) = cursor {
        let id = position(next);
        let order = compare(id)?;
        if order.is_eq() {
            return Ok(Some(id));
        }
        let right = order == Ordering::Greater;
        descend(right);
        cursor = nodes[id].children[usize::from(right)];
    }
    Ok(None)
}

/// Fixed traversal state, never another index or an atom owner.
///
/// Let `N(h)` be the minimum node count of an AVL of height h, with empty height
/// zero. Its recurrence gives `N(h) >= 2*N(h-2)+1` and hence
/// `N(h) >= 2^ceil(h/2)-1`. For a nonempty tree with n nodes and bit width b,
/// `n < 2^b` implies `h <= 2*b`. Since `b <= usize::BITS`, two target-sized words
/// hold every descent. The planned insertion leaf is not a descent bit.
/// The empty tree records no directions. This argument uses mathematical `n+1`;
/// no potentially overflowing machine addition is needed to compute the bound.
///
/// Checked packing protects the representation independently of that AVL
/// invariant. A failed push from an actual search means the internal AVL height
/// invariant was broken, not a user resource refusal or missing atom.
/// This local stack record lives only during entry lookup/path preparation;
/// its two words and checked length are excluded from named vector capacities.
#[derive(Default)]
pub(crate) struct Directions {
    words: [usize; 2],
    length: usize,
}

impl Directions {
    pub(crate) fn push(&mut self, right: bool) -> Option<()> {
        let next = self.length.checked_add(1)?;
        if next > self.words.len() * usize::BITS as usize {
            return None;
        }
        // The newest direction occupies bit zero. Transfer the low word's
        // oldest bit before shifting; the admitted length ensures that no
        // recorded bit can leave the high word. Decode positions only on replay.
        let carry = self.words[0] >> (usize::BITS - 1);
        self.words[0] = (self.words[0] << 1) | usize::from(right);
        self.words[1] = (self.words[1] << 1) | carry;
        self.length = next;
        Some(())
    }

    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn get(&self, position: usize) -> Option<bool> {
        if position >= self.length {
            return None;
        }
        let (word, mask) = Self::position(self.length - 1 - position)?;
        Some(*self.words.get(word)? & mask != 0)
    }

    fn position(position: usize) -> Option<(usize, usize)> {
        let width = usize::BITS as usize;
        let offset = u32::try_from(position % width).ok()?;
        Some((position / width, 1_usize.checked_shl(offset)?))
    }
}
