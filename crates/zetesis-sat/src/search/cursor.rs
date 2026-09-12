//! Retained traversal of the original candidate CNF with later leaf filters.

use super::{Budget, State, increment};
use crate::{Assignment, Cnf, Incomplete, Solve};

mod projections;
use projections::Projections;

/// During one traversal, the caller retains one CNF and only appends clauses,
/// without introducing variables or altering the prefix present at initialization.
/// Later clauses filter complete assignments without entering that traversal's
/// watch registry. A fresh cursor after query refinement initializes from the
/// strengthened CNF, so earlier filters then belong to its watched prefix.
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
    /// This mode admits only exact, complete semantic-prefix blocks after the
    /// original CNF prefix. Initial regions skip the optional root precheck.
    pub(crate) fn projected(semantic_prefix: usize) -> Self {
        Self {
            semantic_prefix: Some(semantic_prefix),
            ..Self::default()
        }
    }

    /// A successfully strengthened candidate query runs the bounded root probe.
    /// The exact projection index remains enabled in both scheduling modes.
    pub(crate) fn refined(semantic_prefix: usize) -> Self {
        Self {
            probe_root: true,
            ..Self::projected(semantic_prefix)
        }
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
            if let Some(prefix) = self.semantic_prefix {
                if self.probe_root && !state.probe(cnf, prefix, budget)? {
                    increment(&mut budget.statistics.conflicts)?;
                    return Ok(None);
                }
                self.projections = Some(Projections::new(prefix, state.base_clauses));
            }
            state.prioritize(cnf, budget)?;
            self.state = Some(state);
        }
        let state = self.state.as_mut().ok_or(Incomplete::InvalidWitness)?;
        if cnf.variables() != state.values.len() || cnf.clauses().len() < state.base_clauses {
            return Err(Incomplete::InvalidWitness);
        }
        if let Some(projections) = &mut self.projections {
            projections.update(cnf, budget)?;
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
                // Base clauses are independently validated by finish; newer
                // semantic blocks are independently tested before projection.
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
    for clause in &cnf.clauses()[base_clauses..] {
        budget.tick()?;
        let mut satisfied = false;
        for literal in clause {
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
