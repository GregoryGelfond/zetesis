//! Necessary counted contributions of a conjunction under an upper bound.
//!
//! If B entails every distinct condition in C, then B also entails at least
//! |H union C| contributions, where H are the currently true unit-row terms.
//! An exclusive bound not exceeding that union makes B false. Ordinary Boolean
//! propagation then removes that alternative from any required disjunction.
//!
//! Preparation joins direct conjunction operands with unit-row incidences.
//! Original root implications add necessary aliases: A -> c supplies A => c;
//! (A or B) -> c supplies both aliases. Double-negated root implications have
//! the same original truth. No non-root implication or frozen truth is used.
//! Only direct operands are projected; an unrecognized structure loses a
//! deduction, never a possible interpretation. Repeated conditions count once.
//!
//! The immutable projection stores only conjunction/row intersections with at
//! least two distinct conditions. There is no row-by-conjunction Cartesian
//! table and no per-region cache. Preparing it sorts sparse incidences; storage
//! is linear in row offsets, retained intersections and their distinct conditions.
//! A dense incidence join can still produce a dense projection. Each
//! dirty row scans its intersections, stopping a condition scan once the bound
//! is exceeded. All runtime visits are charged and cancellable. Original-only
//! row notifications are coalesced by the existing queue.

use crate::regions::{Chain, Closure, Count, Narrower, Step, Work, bit};
use crate::{NodeView, Theory};
use zetesis_cpu::Stop;

use super::{Adjacency, Row, reserve};

#[derive(Clone, Debug)]
pub(super) struct Capacity {
    by_row: Adjacency,
    requirements: Vec<Requirement>,
    conditions: Vec<usize>,
}

#[derive(Clone, Debug)]
struct Requirement {
    root: usize,
    start: usize,
    end: usize,
}

impl Capacity {
    pub(super) fn prepare(
        theory: &Theory,
        rows: &[Row],
        by_condition: Option<&Adjacency>,
        incidences: &[super::Incidence],
        chains: &[Chain],
        extrema: &crate::regions::extrema::Extrema,
    ) -> Result<Option<Self>, Stop> {
        if !rows.iter().any(eligible) || !chains.iter().any(|chain| !chain.disjunction) {
            return Ok(None);
        }
        let by_condition = by_condition.expect("unit rows have incidences");
        let aliases = aliases(theory, rows, by_condition, incidences)?;
        let mut requirements = Vec::new();
        let mut conditions = Vec::new();
        let mut postings = Vec::new();
        let mut projected = Vec::new();
        for chain in chains.iter().filter(|chain| !chain.disjunction) {
            projected.clear();
            for &leaf in chain.leaves(extrema) {
                project(leaf, rows, by_condition, incidences, &mut projected)?;
                let start = aliases.partition_point(|&(source, _)| source < leaf);
                for &(source, condition) in &aliases[start..] {
                    if source != leaf {
                        break;
                    }
                    project(condition, rows, by_condition, incidences, &mut projected)?;
                }
            }
            projected.sort_unstable();
            projected.dedup();
            let mut start = 0;
            while start < projected.len() {
                let row = projected[start].0;
                let mut end = start + 1;
                while end < projected.len() && projected[end].0 == row {
                    end += 1;
                }
                if end - start >= 2 {
                    push(&mut postings, (row, requirements.len()))?;
                    let first = conditions.len();
                    conditions
                        .try_reserve(end - start)
                        .map_err(|_| Stop::Allocation)?;
                    conditions.extend(
                        projected[start..end]
                            .iter()
                            .map(|&(_, condition)| condition),
                    );
                    push(
                        &mut requirements,
                        Requirement {
                            root: chain.root,
                            start: first,
                            end: conditions.len(),
                        },
                    )?;
                }
                start = end;
            }
        }
        if requirements.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            by_row: Adjacency::build(rows.len(), postings.iter().copied())?,
            requirements,
            conditions,
        }))
    }

    pub(super) fn has_row(&self, row: usize) -> bool {
        !self.by_row[row].is_empty()
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        size_of::<Self>() as u128 + self.by_row.retained_bytes() - size_of::<Adjacency>() as u128
            + self.requirements.capacity() as u128 * size_of::<Requirement>() as u128
            + self.conditions.capacity() as u128 * size_of::<usize>() as u128
    }
}

fn eligible(row: &Row) -> bool {
    // Terms are distinct and sorted by descending weight. A singleton cannot
    // supply the two necessary contributions retained by this projection.
    row.terms.len() >= 2 && row.terms.first().is_some_and(|&(_, weight)| weight == 1)
}

fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Stop> {
    values.try_reserve(1).map_err(|_| Stop::Allocation)?;
    values.push(value);
    Ok(())
}

fn project(
    condition: usize,
    rows: &[Row],
    by_condition: &Adjacency,
    incidences: &[super::Incidence],
    projected: &mut Vec<(usize, usize)>,
) -> Result<(), Stop> {
    for &edge in &by_condition[condition] {
        let row = incidences[edge].row;
        if eligible(&rows[row]) {
            push(projected, (row, condition))?;
        }
    }
    Ok(())
}

/// Double negation has classical original truth; stripping it here creates no
/// rule or permission to use the inner implication as a frozen-reduct root.
fn original_root(theory: &Theory, mut node: usize) -> Result<usize, Stop> {
    let view = theory.view();
    while let NodeView::Implies(inner, falsum) =
        view.node(node).map_err(|_| Stop::InvalidProgram)?
    {
        if !view
            .node(falsum)
            .map_err(|_| Stop::InvalidProgram)?
            .is_false()
        {
            break;
        }
        let NodeView::Implies(value, other_false) =
            view.node(inner).map_err(|_| Stop::InvalidProgram)?
        else {
            break;
        };
        if !view
            .node(other_false)
            .map_err(|_| Stop::InvalidProgram)?
            .is_false()
        {
            break;
        }
        node = value;
    }
    Ok(node)
}

fn aliases(
    theory: &Theory,
    rows: &[Row],
    by_condition: &Adjacency,
    incidences: &[super::Incidence],
) -> Result<Vec<(usize, usize)>, Stop> {
    let mut result = reserve(0)?;
    let view = theory.view();
    for &root in theory.roots() {
        let root = original_root(theory, root)?;
        let NodeView::Implies(source, target) =
            view.node(root).map_err(|_| Stop::InvalidProgram)?
        else {
            continue;
        };
        if !by_condition[target]
            .iter()
            .any(|&edge| eligible(&rows[incidences[edge].row]))
        {
            continue;
        }
        push(&mut result, (source, target))?;
        if let NodeView::Or(children) = view.node(source).map_err(|_| Stop::InvalidProgram)? {
            for &child in children {
                push(&mut result, (child, target))?;
            }
        }
    }
    result.sort_unstable();
    result.dedup();
    Ok(result)
}

impl<C: Count> Closure<'_, C> {
    pub(in crate::regions) fn threshold_capacity(
        &mut self,
        index: &Narrower,
        row: usize,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let Some(capacity) = &index.thresholds.capacity else {
            return Ok(Step::Unchanged);
        };
        let state = self.known.thresholds[row];
        let Some(upper) = state.upper else {
            return Ok(Step::Unchanged);
        };
        let mut step = Step::Unchanged;
        for &entry in &capacity.by_row[row] {
            work.tick()?;
            let required = &capacity.requirements[entry];
            if bit(self.known.never, required.root) {
                continue;
            }
            let mut minimum = state.known;
            for &condition in &capacity.conditions[required.start..required.end] {
                work.tick()?;
                // known counts processed events. A pending true condition can
                // make this an underestimate, never an overestimate. Its later
                // processed event schedules this row again.
                minimum += i128::from(!bit(self.known.sure, condition));
                if minimum >= upper {
                    break;
                }
            }
            if minimum >= upper {
                step = step.join(self.never(required.root));
                if step == Step::Contradiction {
                    return Ok(step);
                }
            }
        }
        Ok(step)
    }
}

#[cfg(test)]
mod tests;
