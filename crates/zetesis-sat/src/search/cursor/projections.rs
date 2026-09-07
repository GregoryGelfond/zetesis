//! Exact complete semantic projections, indexed by a bounded binary trie.

use crate::search::Budget;
use crate::{Assignment, Cnf, Incomplete};

#[derive(Debug)]
pub(super) struct Projections {
    width: usize,
    next_clause: usize,
    nodes: Vec<[Option<usize>; 2]>,
    empty_blocked: bool,
}
impl Projections {
    pub(super) fn new(width: usize, next_clause: usize) -> Self {
        Self {
            width,
            next_clause,
            nodes: Vec::new(),
            empty_blocked: false,
        }
    }

    pub(super) fn update(&mut self, cnf: &Cnf, budget: &mut Budget<'_>) -> Result<(), Incomplete> {
        for clause in &cnf.clauses()[self.next_clause..] {
            budget.tick()?;
            if clause.len() != self.width {
                return Err(Incomplete::InvalidWitness);
            }
            // CNF clauses are canonical, so exact prefix coverage can be
            // checked in one pass. No auxiliary reference is admitted here.
            for (variable, literal) in clause.iter().enumerate() {
                budget.tick()?;
                if literal.variable() != variable {
                    return Err(Incomplete::InvalidWitness);
                }
            }
            if self.width == 0 {
                self.empty_blocked = true;
                continue;
            }
            if self.nodes.is_empty() {
                self.append_node()?;
            }
            let mut node = 0;
            for literal in clause {
                budget.tick()?;
                let value = usize::from(!literal.positive());
                node = if let Some(child) = self.nodes[node][value] {
                    child
                } else {
                    let child = self.append_node()?;
                    self.nodes[node][value] = Some(child);
                    child
                };
            }
        }
        self.next_clause = cnf.clauses().len();
        Ok(())
    }

    // At most one node per admitted blocking literal, plus the root; this
    // storage therefore inherits the CNF literal ceiling. No history copies.
    fn append_node(&mut self) -> Result<usize, Incomplete> {
        let index = self.nodes.len();
        self.nodes
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        self.nodes.push([None, None]);
        Ok(index)
    }

    pub(super) fn permits(
        &self,
        assignment: &Assignment,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Incomplete> {
        budget.tick()?;
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
