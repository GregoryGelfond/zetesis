//! Exact complete semantic projections, indexed by a bounded binary trie.

use crate::search::Budget;
use crate::{Assignment, Incomplete};

#[derive(Debug)]
pub(super) struct Projections {
    width: usize,
    nodes: Vec<[Option<usize>; 2]>,
    empty_blocked: bool,
}
impl Projections {
    pub(super) fn new(width: usize) -> Self {
        Self {
            width,
            nodes: Vec::new(),
            empty_blocked: false,
        }
    }

    /// The caller has admitted one logical exclusion of exactly this width.
    /// Every complete-depth path is one excluded projection; intermediate
    /// prefixes are not keys. Only a fully constructed suffix is attached.
    pub(super) fn insert(
        &mut self,
        width: usize,
        value: impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        budget.tick()?;
        if width != self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if width == 0 {
            self.empty_blocked = true;
            return Ok(());
        }
        if self.nodes.is_empty() {
            self.suffix(0, &value, budget)?;
            return Ok(());
        }
        let mut node = 0;
        for variable in 0..width {
            budget.tick()?;
            let branch = usize::from(value(variable));
            if let Some(child) = self.nodes[node][branch] {
                node = child;
            } else {
                let child = self.suffix(variable + 1, &value, budget)?;
                self.nodes[node][branch] = Some(child);
                return Ok(());
            }
        }
        Ok(())
    }

    // At most one node per admitted logical exclusion bit plus one root.
    // Request room for the missing suffix, not another full key or clause.
    // Amortized reservation avoids copying the arena for each inserted key.
    // Shape bounds count nodes; spare capacity/allocator rounding remain
    // outside admission units, which are not an allocated-byte ceiling.
    fn suffix(
        &mut self,
        depth: usize,
        value: &impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Incomplete> {
        let start = self.nodes.len();
        let additional = self
            .width
            .checked_sub(depth)
            .and_then(|n| n.checked_add(1))
            .ok_or(Incomplete::CounterOverflow)?;
        start
            .checked_add(additional)
            .ok_or(Incomplete::CounterOverflow)?;
        self.nodes
            .try_reserve(additional)
            .map_err(|_| Incomplete::Allocation)?;
        let result = (|| {
            for variable in depth..self.width {
                budget.tick()?;
                let mut children = [None, None];
                children[usize::from(value(variable))] = Some(self.nodes.len() + 1);
                self.nodes.push(children);
            }
            budget.tick()?;
            self.nodes.push([None, None]);
            Ok(start)
        })();
        if result.is_err() {
            self.nodes.truncate(start);
        }
        result
    }

    pub(super) fn permits(
        &self,
        assignment: &Assignment,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Incomplete> {
        budget.tick()?;
        if assignment.variables() < self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if self.width == 0 {
            return Ok(!self.empty_blocked);
        }
        if self.nodes.is_empty() {
            return Ok(true);
        }
        let mut node = 0;
        for variable in 0..self.width {
            budget.tick()?;
            let value = usize::from(
                assignment
                    .value(variable)
                    .ok_or(Incomplete::InvalidWitness)?,
            );
            let Some(child) = self.nodes[node][value] else {
                return Ok(true);
            };
            node = child;
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::Projections;
    use crate::search::cursor::tests::budget;
    use crate::{Control, Incomplete};

    #[test]
    fn unrepresentable_node_reservation_keeps_the_index_empty() {
        let control = Control::default();
        // Real non-ZST trie nodes require more than isize::MAX bytes. Vec's
        // fallible reservation must refuse before allocating or reading bits.
        let mut index = Projections::new(isize::MAX as usize);
        assert_eq!(
            index.insert(
                isize::MAX as usize,
                |_| panic!("no key read before reservation"),
                &mut budget(&control)
            ),
            Err(Incomplete::Allocation)
        );
        assert!(index.nodes.is_empty());
        assert!(!index.empty_blocked);
    }
}
