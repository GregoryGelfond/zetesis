//! Assignment-independent watch topology for an authenticated immutable CNF.

use std::mem::size_of;

use super::{Budget, Quota, Solve, Workspace, increment, search_seeded};
use crate::{Assignment, Cnf, Incomplete, Literal};

/// The caller binds this workspace to an exact immutable CNF owner and must
/// invalidate it before changing owners. Shape equality is insufficient.
/// `indexed` certifies that every nonunit clause has two distinct positions and
/// exactly one list node for each position. It never certifies an assignment.
#[derive(Debug, Default)]
pub(crate) struct PreparedWorkspace {
    workspace: Workspace,
    indexed: bool,
}

impl PreparedWorkspace {
    pub(crate) fn invalidate(&mut self) {
        self.indexed = false;
    }

    pub(crate) fn reserve(&mut self, variables: usize, clauses: usize) -> Result<(), Incomplete> {
        self.workspace.reserve(variables, clauses)
    }

    pub(crate) fn retained_bytes(&self) -> u128 {
        (size_of::<Self>() - size_of::<Workspace>()) as u128 + self.workspace.retained_bytes()
    }

    pub(crate) fn required_bytes(&self, variables: usize, clauses: usize) -> u128 {
        (size_of::<Self>() - size_of::<Workspace>()) as u128
            + self.workspace.required_bytes(variables, clauses)
    }

    /// Unit indices and the empty-clause fact come from the same immutable CNF.
    /// A failed initial index is rebuilt on retry. Once complete, watch moves
    /// preserve the registry even if inspection or later search is interrupted:
    /// every fallible replacement inspection precedes its no-fail link mutation.
    pub(crate) fn query(
        &mut self,
        cnf: &Cnf,
        units: &[usize],
        has_empty_clause: bool,
        assumptions: &[Literal],
        budget: &mut Budget<'_, impl Quota>,
    ) -> Solve {
        match self.run(cnf, units, has_empty_clause, assumptions, budget) {
            Ok(Some(assignment)) => Solve::Sat(assignment),
            Ok(None) => Solve::Unsat,
            Err(error) => Solve::Inconclusive(error),
        }
    }

    fn run(
        &mut self,
        cnf: &Cnf,
        units: &[usize],
        has_empty_clause: bool,
        assumptions: &[Literal],
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<Option<Assignment>, Incomplete> {
        self.reset(cnf, budget)?;
        if has_empty_clause {
            increment(&mut budget.statistics.conflicts)?;
            return Ok(None);
        }
        let state = &mut self.workspace.0;
        for &clause in units {
            budget.tick()?;
            let literal = cnf.clause_at(clause).at(0);
            if state.value(literal).is_none() {
                increment(&mut budget.statistics.propagations)?;
            }
            if !state.assign(literal) {
                increment(&mut budget.statistics.conflicts)?;
                return Ok(None);
            }
        }
        search_seeded(state, cnf, assumptions, budget)
    }

    fn reset(&mut self, cnf: &Cnf, budget: &mut Budget<'_, impl Quota>) -> Result<(), Incomplete> {
        let state = &mut self.workspace.0;
        if !self.indexed {
            // reset may stop while filling any array; none of that partial
            // topology is reusable. Publish readiness only after the full scan.
            state.reset(cnf, budget)?;
            for clause in 0..state.base_clauses {
                budget.tick()?;
                if cnf.clause_at(clause).len() >= 2 {
                    state.index_clause(cnf, clause)?;
                }
            }
            self.indexed = true;
            return Ok(());
        }
        budget.tick()?;
        // Every Some value has exactly one live trail entry. Admission precedes
        // each pop/clear pair, so a stop preserves the remaining undo work.
        // Never clear the trail while any corresponding truth remains assigned.
        while let Some(&literal) = state.trail.last() {
            budget.tick()?;
            state.values[literal.variable()] = None;
            state.trail.pop();
        }
        state.decisions.clear();
        state.order.clear();
        state.ranks.clear();
        state.ordering.clear();
        state.propagation_head = 0;
        state.next_position = 0;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/support/prepared_search.rs"]
mod tests;
