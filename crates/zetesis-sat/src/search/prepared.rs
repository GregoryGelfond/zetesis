//! Retained watch topology and unconditional consequences of one immutable CNF.

use std::mem::size_of;

use super::{Budget, Quota, Solve, Workspace, increment, search_seeded};
use crate::{Assignment, Cnf, Incomplete, Literal};

/// The caller binds this workspace to an exact immutable CNF owner and must
/// invalidate it before changing owners. Shape equality is insufficient.
/// `indexed` certifies that every nonunit clause has two distinct positions and
/// exactly one list node for each position. The separate base capability records
/// only unit consequences of that immutable CNF, before any query parameters.
#[derive(Debug, Default)]
pub(crate) struct PreparedWorkspace {
    workspace: Workspace,
    indexed: bool,
    base: BaseClosure,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BaseClosure {
    /// No completed unconditional propagation result may be reused.
    #[default]
    Unprepared,
    /// All assignments in this prefix follow from the immutable CNF alone.
    /// Quiescence does not assert that every clause is satisfied.
    Closed { trail_len: usize },
    /// The immutable CNF's unit propagation contradicts, without parameters.
    Refuted,
}

impl PreparedWorkspace {
    pub(crate) fn invalidate(&mut self) {
        self.indexed = false;
        self.base = BaseClosure::Unprepared;
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
        if !self.prepare_base(cnf, units, has_empty_clause, budget)? {
            // Each query encountering a retained contradiction records that
            // conflict outcome; work/propagations count only executed steps.
            increment(&mut budget.statistics.conflicts)?;
            return Ok(None);
        }
        search_seeded(&mut self.workspace.0, cnf, assumptions, budget)
    }

    fn prepare_base(
        &mut self,
        cnf: &Cnf,
        units: &[usize],
        has_empty_clause: bool,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<bool, Incomplete> {
        match self.base {
            BaseClosure::Closed { .. } => return Ok(true),
            BaseClosure::Refuted => return Ok(false),
            BaseClosure::Unprepared => (),
        }
        if has_empty_clause {
            self.base = BaseClosure::Refuted;
            return Ok(false);
        }
        let state = &mut self.workspace.0;
        for &clause in units {
            budget.tick()?;
            let literal = cnf.clause_at(clause).at(0);
            if state.value(literal).is_none() {
                increment(&mut budget.statistics.propagations)?;
            }
            if !state.assign(literal) {
                self.base = BaseClosure::Refuted;
                return Ok(false);
            }
        }
        // No candidate parameter has been assigned. A stopped propagation keeps
        // Unprepared; reset must undo its whole unfinished trail before retry.
        if !state.propagate(cnf, budget)? {
            self.base = BaseClosure::Refuted;
            return Ok(false);
        }
        self.base = BaseClosure::Closed {
            trail_len: state.trail.len(),
        };
        Ok(true)
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
        let base_len = match self.base {
            BaseClosure::Closed { trail_len } => trail_len,
            BaseClosure::Unprepared => 0,
            BaseClosure::Refuted => return Ok(()),
        };
        // Every Some value has exactly one live trail entry. Admission precedes
        // each pop/clear pair, preserving unfinished suffix undo on a stop.
        // The completed base prefix is never a candidate's truth assumption.
        while state.trail.len() > base_len {
            budget.tick()?;
            let literal = state.trail.pop().ok_or(Incomplete::InvalidWitness)?;
            state.values[literal.variable()] = None;
        }
        state.decisions.clear();
        state.order.clear();
        state.ranks.clear();
        state.ordering.clear();
        // At the base fixpoint, a base-false watch is protected by a base-true
        // other watch. That true watch never moves; later moved watches were
        // nonfalse under a stronger query assignment and remain nonfalse after
        // suffix undo. Thus no completed base event needs to be replayed.
        state.propagation_head = base_len;
        state.next_position = 0;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/support/prepared_search.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/support/prepared_base.rs"]
mod base_tests;
