//! Root failed-literal probing for a fixed classical candidate query.

use super::{Budget, State, increment};
use crate::{Cnf, Incomplete, Literal};

impl State {
    // Only a unit conflict under a trial assumption permits the opposite root
    // assignment. Watch moves survive undo just as in ordinary backtracking.
    // This runs before the decision permutation exists; no decision or rank
    // state is changed. An interrupted trial discards the entire cursor state.
    pub(super) fn probe(
        &mut self,
        cnf: &Cnf,
        prefix: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Incomplete> {
        if prefix > self.values.len() {
            return Err(Incomplete::InvalidWitness);
        }
        for variable in 0..prefix {
            budget.tick()?;
            if self.values[variable].is_some() {
                continue;
            }
            for positive in [false, true] {
                budget.tick()?;
                let start = self.trail.len();
                if !self.assign(Literal::new(variable, positive)) {
                    return Err(Incomplete::InvalidWitness);
                }
                let consistent = self.propagate(cnf, budget)?;
                while self.trail.len() > start {
                    budget.tick()?;
                    let literal = self.trail.pop().ok_or(Incomplete::InvalidWitness)?;
                    self.values[literal.variable()] = None;
                }
                self.propagation_head = start;
                if !consistent {
                    increment(&mut budget.statistics.conflicts)?;
                    if !self.assign(Literal::new(variable, !positive)) {
                        return Err(Incomplete::InvalidWitness);
                    }
                    increment(&mut budget.statistics.propagations)?;
                    if !self.propagate(cnf, budget)? {
                        return Ok(false);
                    }
                    break;
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdmissionLimits, Control, SearchLimits, SearchStatistics};

    #[test]
    fn both_failed_polarities_and_watch_undo_are_classical_certificates() {
        let control = Control::default();
        for forced in [false, true] {
            let p = Literal::new(0, forced);
            let q = Literal::new(1, true);
            let cnf = Cnf::new(
                2,
                vec![vec![p, q], vec![p, q.negated()]],
                AdmissionLimits::default(),
            )
            .unwrap();
            let mut budget = Budget {
                quota: crate::search::LocalQuota,
                limits: SearchLimits::default(),
                control: &control,
                statistics: SearchStatistics::default(),
            };
            let mut state = State::new(&cnf, &mut budget).unwrap();
            assert!(state.initialize(&cnf, &mut budget).unwrap());
            assert!(state.propagate(&cnf, &mut budget).unwrap());
            assert_eq!(state.values, [None, None]);
            assert!(state.probe(&cnf, 2, &mut budget).unwrap());
            assert_eq!(state.values, [Some(forced), None]);
            assert_eq!(state.trail, [p]);
            assert_eq!(state.propagation_head, state.trail.len());
            assert!(state.decisions.is_empty());
            assert!(state.order.is_empty());
        }
        let clauses = [false, true]
            .into_iter()
            .flat_map(|p| {
                [false, true]
                    .into_iter()
                    .map(move |q| vec![Literal::new(0, p), Literal::new(1, q)])
            })
            .collect();
        let cnf = Cnf::new(2, clauses, AdmissionLimits::default()).unwrap();
        let mut budget = Budget {
            quota: crate::search::LocalQuota,
            limits: SearchLimits::default(),
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let mut state = State::new(&cnf, &mut budget).unwrap();
        assert!(state.initialize(&cnf, &mut budget).unwrap());
        assert!(state.propagate(&cnf, &mut budget).unwrap());
        assert!(!state.probe(&cnf, 2, &mut budget).unwrap());
    }
}
