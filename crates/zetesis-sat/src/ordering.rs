//! A bounded permutation of unassigned variables; clauses remain unchanged.

use crate::search::{Budget, Quota, increment, reserve};
use crate::{Clause, Incomplete};

#[derive(Debug, Default)]
pub(crate) struct Workspace {
    scores: Vec<u64>,
    buffer: Vec<usize>,
}

impl Workspace {
    pub(crate) fn clear(&mut self) {
        self.scores.clear();
        self.buffer.clear();
    }

    pub(crate) fn reserve(&mut self, count: usize) -> Result<(), Incomplete> {
        reserve(&mut self.scores, count)?;
        reserve(&mut self.buffer, count)
    }

    pub(crate) fn required_bytes(&self, variables: usize) -> u128 {
        self.scores.capacity().max(variables) as u128 * std::mem::size_of::<u64>() as u128
            + self.buffer.capacity().max(variables) as u128 * std::mem::size_of::<usize>() as u128
    }

    pub(crate) fn variables<'a>(
        &mut self,
        order: &mut Vec<usize>,
        clauses: impl Iterator<Item = Clause<'a>>,
        values: &[Option<bool>],
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<(), Incomplete> {
        self.scores.clear();
        reserve(&mut self.scores, values.len())?;
        let scores = &mut self.scores;
        let mut unassigned = 0;
        for value in values {
            budget.tick()?;
            scores.push(0);
            unassigned += usize::from(value.is_none());
        }
        for clause in clauses {
            let mut satisfied = false;
            for literal in clause.iter() {
                budget.tick()?;
                if values[literal.variable()] == Some(literal.positive()) {
                    satisfied = true;
                    break;
                }
            }
            if satisfied {
                continue;
            }
            for literal in clause.iter() {
                budget.tick()?;
                if values[literal.variable()].is_none() {
                    increment(&mut scores[literal.variable()])?;
                }
            }
        }
        sort_into(scores, values, unassigned, order, &mut self.buffer, budget)
    }
}

fn sort_into(
    scores: &[u64],
    values: &[Option<bool>],
    count: usize,
    order: &mut Vec<usize>,
    buffer: &mut Vec<usize>,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<(), Incomplete> {
    order.clear();
    buffer.clear();
    reserve(order, count)?;
    reserve(buffer, count)?;
    // This runs after root propagation/probing and before the first decision.
    // Every decision's trail start follows these assignments, so backtracking
    // never clears them. A strengthened query constructs a new cursor/order.
    for (variable, value) in values.iter().enumerate() {
        budget.tick()?;
        if value.is_none() {
            order.push(variable);
            buffer.push(variable);
        }
    }
    let mut width = 1usize;
    while width < count {
        let mut start = 0;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            let (mut left, mut right) = (start, middle);
            for slot in &mut buffer[start..end] {
                budget.tick()?;
                let take_left = right == end
                    || (left < middle
                        && (scores[order[left]] > scores[order[right]]
                            || (scores[order[left]] == scores[order[right]]
                                && order[left] < order[right])));
                if take_left {
                    *slot = order[left];
                    left += 1;
                } else {
                    *slot = order[right];
                    right += 1;
                }
            }
            start = end;
        }
        std::mem::swap(order, buffer);
        width = width.saturating_mul(2);
    }
    Ok(())
}

#[cfg(test)]
fn variables<'a>(
    clauses: impl Iterator<Item = Clause<'a>>,
    values: &[Option<bool>],
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Vec<usize>, Incomplete> {
    let mut order = Vec::new();
    Workspace::default().variables(&mut order, clauses, values, budget)?;
    Ok(order)
}

#[cfg(test)]
fn sort(
    scores: &[u64],
    values: &[Option<bool>],
    count: usize,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Vec<usize>, Incomplete> {
    let mut order = Vec::new();
    sort_into(scores, values, count, &mut order, &mut Vec::new(), budget)?;
    Ok(order)
}

#[cfg(test)]
mod tests {
    use crate::search::Budget;
    use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};

    #[test]
    fn bounded_order_is_a_complete_permutation_matching_independent_sort() {
        let cancellation = Cancellation::default();
        for count in 0..129 {
            let scores: Vec<_> = (0..count).map(|n| (n * n + 7 * n + count) % 17).collect();
            let mut expected: Vec<_> = (0..scores.len()).collect();
            expected.sort_by_key(|&index| (std::cmp::Reverse(scores[index]), index));
            let mut budget = Budget {
                quota: crate::search::LocalQuota,
                limits: SearchLimits::default(),
                cancellation: &cancellation,
                statistics: SearchStatistics::default(),
            };
            let values = vec![None; scores.len()];
            assert_eq!(
                super::sort(&scores, &values, scores.len(), &mut budget).unwrap(),
                expected
            );
            if count > 0 {
                let mut limited = Budget {
                    quota: crate::search::LocalQuota,
                    limits: SearchLimits {
                        max_work: budget.statistics.work - 1,
                        ..Default::default()
                    },
                    cancellation: &cancellation,
                    statistics: SearchStatistics::default(),
                };
                assert_eq!(
                    super::sort(&scores, &values, scores.len(), &mut limited),
                    Err(Incomplete::WorkLimit)
                );
            }
        }
    }
}

#[cfg(test)]
mod root_contracts;
