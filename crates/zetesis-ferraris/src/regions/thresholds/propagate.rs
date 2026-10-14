//! A shared row consumes each grouped condition event once. Numeric sums never
//! read queued bits as processed evidence. Guard and forcing cursors advance
//! monotonically, so each completed scan position is paid once per branch.

use crate::Theory;
use crate::regions::{Closure, Count, Narrower, Step, Work, bit};
use zetesis_cpu::Stop;

impl<C: Count> Closure<'_, C> {
    pub(in crate::regions) fn seed_thresholds(
        &mut self,
        index: &Narrower,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for row in 0..index.thresholds.rows.len() {
            step = step.join(self.threshold_row(index, row, frozen, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
        Ok(step)
    }

    pub(in crate::regions) fn threshold_guard(
        &mut self,
        index: &Narrower,
        theory: &Theory,
        node: usize,
        bound: i128,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let row = index.thresholds.row(theory, node);
        let state = &mut self.known.thresholds[row];
        if bit(self.known.sure, node) {
            if state.lower >= bound {
                return Ok(Step::Unchanged);
            }
            state.lower = bound;
        } else {
            if state.upper.is_some_and(|upper| upper <= bound) {
                return Ok(Step::Unchanged);
            }
            state.upper = Some(bound);
        }
        if frozen.is_none() && index.thresholds.has_requirement_links(row) {
            self.lists.queue_threshold(row);
        }
        self.threshold_row(index, row, frozen, work)
    }

    /// Called once when the canonical node event is processed, including a
    /// frozen-false condition's event. No pending condition updates a sum.
    pub(in crate::regions) fn threshold_condition(
        &mut self,
        index: &Narrower,
        node: usize,
        value: bool,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for &edge in index.thresholds.condition(node) {
            work.tick()?;
            let edge = index.thresholds.incidences[edge];
            let state = &mut self.known.thresholds[edge.row];
            if value {
                state.known += edge.weight;
            } else {
                state.possible -= edge.weight;
            }
            step = step.join(self.threshold_row(index, edge.row, frozen, work)?);
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
        Ok(step)
    }

    pub(super) fn threshold_row(
        &mut self,
        index: &Narrower,
        row: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        work.tick()?;
        let state = self.known.thresholds[row];
        if state.lower > state.possible
            || state
                .upper
                .is_some_and(|upper| state.known.max(state.lower) >= upper)
        {
            return Ok(Step::Contradiction);
        }
        if frozen.is_none()
            && index.thresholds.rows[row].has_nonunit_coefficient()
            && state.has_residual_interval()
        {
            self.lists.queue_residual(row);
        }
        let step = self.threshold_guards(index, row, frozen, work)?;
        if step == Step::Contradiction {
            return Ok(step);
        }
        let forced = self.threshold_terms(index, row, true, work)?;
        if forced == Step::Contradiction {
            return Ok(forced);
        }
        Ok(step
            .join(forced)
            .join(self.threshold_terms(index, row, false, work)?))
    }

    /// Bound implications and numeric truth use the same ordered guard ranges.
    /// Frozen-false targets are constants and supply no row requirement.
    fn threshold_guards(
        &mut self,
        index: &Narrower,
        row: usize,
        frozen: Option<&[bool]>,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let guards = &index.thresholds.rows[row].guards;
        let mut step = Step::Unchanged;
        loop {
            let state = &mut self.known.thresholds[row];
            let Some(&(node, bound)) = guards.get(state.sure_guard) else {
                break;
            };
            work.tick()?;
            if bound > state.known.max(state.lower) {
                break;
            }
            state.sure_guard += 1;
            if frozen.is_none_or(|truth| truth[node]) {
                step = step.join(self.sure(node));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        loop {
            let state = &mut self.known.thresholds[row];
            if state.never_guard == 0 {
                break;
            }
            let (node, bound) = guards[state.never_guard - 1];
            work.tick()?;
            if bound <= state.possible && state.upper.is_none_or(|upper| bound < upper) {
                break;
            }
            state.never_guard -= 1;
            if frozen.is_none_or(|truth| truth[node]) {
                step = step.join(self.never(node));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        Ok(step)
    }

    /// Descending coefficients cross each forcing cutoff at most once. Decided
    /// bits may be skipped before their events run: they need no further forcing,
    /// but their contributions still wait for `threshold_condition` to update sums.
    fn threshold_terms(
        &mut self,
        index: &Narrower,
        row: usize,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let state = self.known.thresholds[row];
        let cutoff = if value {
            if state.known >= state.lower {
                return Ok(Step::Unchanged);
            }
            // threshold_row checked lower <= possible, so this cannot overflow.
            state.possible - state.lower
        } else {
            let Some(upper) = state.upper else {
                return Ok(Step::Unchanged);
            };
            // threshold_row checked upper > known >= 0.
            upper - state.known
        };
        let terms = &index.thresholds.rows[row].terms;
        let mut step = Step::Unchanged;
        loop {
            let state = &mut self.known.thresholds[row];
            let cursor = if value {
                &mut state.true_term
            } else {
                &mut state.false_term
            };
            let Some(&(condition, weight)) = terms.get(*cursor) else {
                break;
            };
            work.tick()?;
            let decided = bit(self.known.sure, condition) || bit(self.known.never, condition);
            if !decided
                && if value {
                    weight <= cutoff
                } else {
                    weight < cutoff
                }
            {
                break;
            }
            *cursor += 1;
            if !decided {
                step = step.join(if value {
                    self.sure(condition)
                } else {
                    self.never(condition)
                });
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        Ok(step)
    }
}
