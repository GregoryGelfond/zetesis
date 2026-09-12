//! Retained candidate traversal with persistent exact semantic exclusions.

use super::{Budget, State, increment};
use crate::{Assignment, Cnf, Incomplete, Solve};
use zetesis_ferraris::Interpretation;

mod projections;
use projections::Projections;

/// During one traversal, the caller retains one CNF and only appends clauses,
/// without introducing variables or altering the prefix present at initialization.
/// In general mode, later clauses filter leaves. Projected mode instead owns
/// exact exclusions outside the CNF: restart replaces traversal state while
/// preserving the fixed-width index. Final membership is checked on the
/// completed assignment independently of DPLL state; it shares the same index,
/// not a second history or a second exclusion implementation.
#[derive(Debug, Default)]
pub(crate) struct Cursor {
    state: Option<State>,
    semantic_prefix: Option<usize>,
    probe_root: bool,
    projections: Option<Projections>,
    after_model: bool,
    terminal: Option<Result<(), Incomplete>>,
}
impl Cursor {
    /// This mode admits exact, complete semantic-prefix exclusions separately
    /// from the CNF. Initial regions skip the optional root precheck.
    pub(crate) fn projected(semantic_prefix: usize) -> Self {
        Self {
            semantic_prefix: Some(semantic_prefix),
            projections: Some(Projections::new(semantic_prefix)),
            ..Self::default()
        }
    }

    /// A successfully strengthened candidate query runs the bounded root probe.
    /// The exact projection index remains enabled in both scheduling modes.
    pub(crate) fn restart(&mut self) {
        self.state = None;
        self.probe_root = true;
        self.after_model = false;
        self.terminal = None;
    }

    pub(crate) fn exclude(
        &mut self,
        cnf: &mut Cnf,
        candidate: &Interpretation,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        budget.tick()?;
        let width = candidate.theory().atom_count();
        if self.semantic_prefix != Some(width) || width > cnf.variables() {
            return Err(Incomplete::InvalidWitness);
        }
        let projections = self
            .projections
            .as_mut()
            .ok_or(Incomplete::InvalidWitness)?;
        let checkpoint = cnf.checkpoint();
        cnf.admit_exclusion(width)?;
        let result = projections.insert(width, |atom| candidate.contains(atom), budget);
        if result.is_err() {
            cnf.rollback(checkpoint);
        }
        result
    }

    pub(crate) fn query(&mut self, cnf: &Cnf, budget: &mut Budget<'_>) -> Solve {
        if let Some(terminal) = self.terminal {
            return match terminal {
                Ok(()) => Solve::Unsat,
                Err(error) => Solve::Inconclusive(error),
            };
        }
        match self.advance(cnf, budget) {
            Ok(Some(assignment)) => Solve::Sat(assignment),
            Ok(None) => {
                self.state = None;
                self.terminal = Some(Ok(()));
                Solve::Unsat
            }
            Err(error) => {
                self.state = None;
                self.terminal = Some(Err(error));
                Solve::Inconclusive(error)
            }
        }
    }

    fn advance(
        &mut self,
        cnf: &Cnf,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Assignment>, Incomplete> {
        // Resumption polls even when a previous model had no decision frames.
        budget.tick()?;
        if self.state.is_none() {
            let mut state = State::new(cnf, budget)?;
            if !state.initialize(cnf, budget)? || !state.propagate(cnf, budget)? {
                increment(&mut budget.statistics.conflicts)?;
                return Ok(None);
            }
            if let Some(prefix) = self.semantic_prefix
                && self.probe_root
                && !state.probe(cnf, prefix, budget)?
            {
                increment(&mut budget.statistics.conflicts)?;
                return Ok(None);
            }
            state.prioritize(cnf, budget)?;
            self.state = Some(state);
        }
        let state = self.state.as_mut().ok_or(Incomplete::InvalidWitness)?;
        if cnf.variables() != state.values.len() || cnf.clauses().len() < state.base_clauses {
            return Err(Incomplete::InvalidWitness);
        }
        if self.projections.is_some() && cnf.clauses().len() != state.base_clauses {
            return Err(Incomplete::InvalidWitness);
        }
        if self.after_model {
            self.after_model = false;
            if !state.backtrack(budget)? {
                return Ok(None);
            }
        }
        loop {
            budget.tick()?;
            if !state.propagate(cnf, budget)? {
                increment(&mut budget.statistics.conflicts)?;
                if !state.backtrack(budget)? {
                    return Ok(None);
                }
            } else if !state.branch(budget)? {
                // Base clauses are independently validated by finish; all
                // semantic exclusions are separately tested before projection.
                let assignment = state.finish(cnf, budget)?;
                let allowed = if let Some(projections) = &self.projections {
                    projections.permits(&assignment, budget)?
                } else {
                    permitted(cnf, state.base_clauses, &assignment, budget)?
                };
                if allowed {
                    self.after_model = true;
                    return Ok(Some(assignment));
                }
                increment(&mut budget.statistics.conflicts)?;
                if !state.backtrack(budget)? {
                    return Ok(None);
                }
            }
        }
    }
}

fn permitted(
    cnf: &Cnf,
    base_clauses: usize,
    assignment: &Assignment,
    budget: &mut Budget<'_>,
) -> Result<bool, Incomplete> {
    for clause in cnf.clauses().skip(base_clauses) {
        budget.tick()?;
        let mut satisfied = false;
        for literal in clause.iter() {
            budget.tick()?;
            if assignment
                .value(literal.variable())
                .ok_or(Incomplete::InvalidWitness)?
                == literal.positive()
            {
                satisfied = true;
                break;
            }
        }
        if !satisfied {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod exclusions;
