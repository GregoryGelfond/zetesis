//! Selected discovery order derives from the existing semantic AVL trees.

use super::{
    AtomAppender, Failure, Limits, Link, Node, Step, Subtree, admit, cells, path_bound, population,
    position, reserve,
};

enum Selection<'a> {
    Prefix(usize),
    Mask { words: &'a [u64], count: usize },
}

impl Selection<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Prefix(count) | Self::Mask { count, .. } => *count,
        }
    }

    fn contains(&self, position: usize) -> bool {
        match self {
            Self::Prefix(count) => position < *count,
            Self::Mask { words, .. } => words[position / 64] & (1_u64 << (position % 64)) != 0,
        }
    }
}

impl AtomAppender<'_> {
    /// Prospective total named storage for dense selected ordering.
    ///
    /// Returns `None` when the strategy is inapplicable: for discovery population
    /// n and selected count d, it requires 2 <= d <= n <= 2*d. This protects small
    /// deltas from repeatedly traversing a large historical index. It does not
    /// inspect the selected positions or establish their validity.
    ///
    /// Includes this owner's current storage, an n-bit mask, d-position output,
    /// their headers and conservative old/new path-buffer overlap. The caller's
    /// selected slice and other live owners are excluded; compare the result with
    /// the allowance remaining for this owner. The estimate uses requested
    /// capacities. Actual allocator slack is checked by [`Self::order_selected_with`]
    /// and can still cause a typed refusal. This read neither allocates nor changes
    /// a peak receipt; it is not evidence that the operation completed.
    #[must_use]
    pub fn selected_order_storage(&self, selected: usize) -> Option<u128> {
        let count = self.len();
        if selected < 2 || selected > count || count > selected.saturating_mul(2) {
            return None;
        }
        let path = path_bound(count);
        Some(
            self.storage_bytes()
                + size_of::<Vec<u64>>() as u128
                + cells::<u64>(count.div_ceil(64))
                + size_of::<Vec<usize>>() as u128
                + cells::<usize>(selected)
                + if self.index.path.capacity() < path {
                    // Incremental growth may overlap two later buffers, not
                    // merely the original capacity and the final buffer.
                    2 * cells::<Step>(path) - cells::<Step>(self.index.path.capacity())
                } else {
                    0
                },
        )
    }

    /// Replace distinct discovery positions by their exact typed atom order.
    ///
    /// Positions may select any committed or pending discoveries, including old
    /// identities that a caller has not previously selected. Canonical rows with
    /// no discovery position are outside this operation. Inorder traversal of
    /// the existing semantic index supplies order; numeric IDs only select rows.
    /// For n discoveries and d selected positions, this visits O(n+d) metadata
    /// and needs an n-bit mask, d-position temporary output and O(log n) retained
    /// path scratch. No atom or term payload is copied or compared.
    ///
    /// All replacement writes receive permits before any caller position changes.
    /// On refusal or a callback panic the caller slice is unchanged. The owner
    /// retains any grown path capacity, and its peak includes actual temporary
    /// buffers and growth overlap even when they are dropped on failure/unwind.
    /// Empty and singleton selections need no temporary buffers.
    ///
    /// # Errors
    /// Rejects duplicate or out-of-range positions as a canonical shape error,
    /// and propagates population, storage, allocation and caller-work refusals.
    /// [`Self::selected_order_storage`] is an optional strategy preflight; once
    /// chosen, a refusal is not a signal to silently retry another algorithm.
    pub fn order_selected_with<E>(
        &mut self,
        selected: &mut [usize],
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        population(self.len(), limits)?;
        admit(self.storage_bytes(), limits)?;
        if selected.len() > self.len() {
            return Err(shape());
        }
        if selected.len() < 2 {
            if !selected.is_empty() {
                before().map_err(Failure::Stopped)?;
                if selected[0] >= self.len() {
                    return Err(shape());
                }
            }
            return Ok(());
        }
        let mask = self.selection_mask(selected, limits, &mut before)?;
        let extra = size_of::<Vec<u64>>() as u128 + cells::<u64>(mask.capacity());
        let output = self.collect_order(
            &Selection::Mask {
                words: &mask,
                count: selected.len(),
            },
            extra,
            limits,
            &mut before,
        )?;
        for _ in 0..selected.len() {
            before().map_err(Failure::Stopped)?;
        }
        selected.copy_from_slice(&output);
        Ok(())
    }

    fn selection_mask<E>(
        &mut self,
        selected: &[usize],
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<u64>, Failure<E>> {
        let mut checked = || before().map_err(Failure::Stopped);
        let mut mask = Vec::new();
        let words = self.len().div_ceil(64);
        let live = self.storage_bytes() + size_of::<Vec<u64>>() as u128;
        reserve(
            &mut mask,
            words,
            words,
            live,
            &mut self.index.peak,
            limits,
            &mut checked,
        )?;
        for _ in 0..words {
            checked()?;
            mask.push(0_u64);
        }
        for position in selected {
            checked()?;
            let position = *position;
            if position >= self.len() {
                return Err(shape());
            }
            let word = &mut mask[position / 64];
            let bit = 1_u64 << (position % 64);
            if *word & bit != 0 {
                return Err(shape());
            }
            checked()?;
            *word |= bit;
        }
        Ok(mask)
    }

    pub(super) fn ordered_prefix_with<E>(
        &mut self,
        count: usize,
        limits: Limits,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        self.collect_order(&Selection::Prefix(count), 0, limits, &mut before)
    }

    fn collect_order<E>(
        &mut self,
        selection: &Selection<'_>,
        extra: u128,
        limits: Limits,
        before: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Vec<usize>, Failure<E>> {
        // Traversal repurposes the semantic mutation buffer. Revoke its
        // published-spine certificate before any fallible preparation.
        self.spines.semantic = None;
        let count = self.len();
        population(count, limits)?;
        let base = self.storage_bytes() - cells::<Step>(self.index.path.capacity())
            + extra
            + size_of::<Vec<usize>>() as u128;
        Traversal {
            nodes: &self.index.nodes,
            path: &mut self.index.path,
            subtrees: self.subtrees,
            peak: &mut self.index.peak,
            base,
            path_bound: path_bound(count),
            limits,
        }
        .collect(selection, &mut || before().map_err(Failure::Stopped))
    }
}

/// `base` excludes the mutable path and output capacities. Their current
/// capacities are added before each reserve, including simultaneous growth.
struct Traversal<'a> {
    nodes: &'a [Node],
    path: &'a mut Vec<Step>,
    subtrees: &'a [Subtree],
    peak: &'a mut u128,
    base: u128,
    path_bound: usize,
    limits: Limits,
}

impl Traversal<'_> {
    fn live(&self, output: usize) -> u128 {
        self.base + cells::<Step>(self.path.capacity()) + cells::<usize>(output)
    }

    fn collect<E>(
        mut self,
        selection: &Selection<'_>,
        before: &mut impl FnMut() -> Result<(), Failure<E>>,
    ) -> Result<Vec<usize>, Failure<E>> {
        let mut output = Vec::new();
        let count = selection.len();
        let live = self.live(0);
        reserve(
            &mut output,
            count,
            count,
            live,
            self.peak,
            self.limits,
            before,
        )?;
        before()?;
        self.path.clear();
        // Predicate subtrees and each subtree's argument ordering are semantic.
        // Each descent pushes a new ancestor, and each pop visits it once.
        for relation in 0..self.subtrees.len() {
            before()?;
            let mut cursor = self.subtrees[relation].root;
            loop {
                self.descend(cursor, output.capacity(), before)?;
                before()?;
                let Some(step) = self.path.pop() else { break };
                if selection.contains(step.id) {
                    before()?;
                    output.push(step.id);
                }
                cursor = step.node.children[1];
            }
        }
        if output.len() != count {
            return Err(shape());
        }
        Ok(output)
    }

    fn descend<E>(
        &mut self,
        mut cursor: Link,
        output_capacity: usize,
        before: &mut impl FnMut() -> Result<(), Failure<E>>,
    ) -> Result<(), Failure<E>> {
        while let Some(next) = cursor {
            before()?;
            let id = position(next);
            let node = self.nodes[id];
            let live = self.live(output_capacity);
            reserve(
                self.path,
                1,
                self.path_bound,
                live,
                self.peak,
                self.limits,
                before,
            )?;
            before()?;
            self.path.push(Step {
                id,
                node,
                right: false,
                changed: false,
            });
            cursor = node.children[0];
        }
        Ok(())
    }
}

fn shape<E>() -> Failure<E> {
    Failure::Catalog(crate::catalog::Error::Shape)
}
