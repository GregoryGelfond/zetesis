//! The order in which one rule's positive body is joined.
//!
//! The order is chosen once per join from what the body says before a row is
//! read: the rows each occurrence is offered, the variables it binds and the
//! variables each comparison waits on. Extending the bound prefix one
//! occurrence at a time, the next occurrence is chosen by:
//!
//! 1. an occurrence whose variables are all bound is a test, offering at most
//!    one row per prefix row and never widening the join, so it precedes
//!    every generator;
//! 2. among generators, the one offered fewer rows: a relation's size, or
//!    within a semi-naive round the rows its partition offers, so that the
//!    pivot occurrence's new rows are joined first when they are the fewest;
//! 3. among relations of one size, the occurrence that decides the most
//!    comparisons still waiting on its variables, then the one that binds
//!    the most variables such comparisons wait on;
//! 4. otherwise the earlier occurrence of the canonical body.
//!
//! Every order yields the same complete bindings, since the join is a
//! conjunction, and the semi-naive partition is by source occurrence and
//! does not read this order. The choice costs one work unit per occurrence
//! considered against each waiting comparison.

use std::cmp::Reverse;

use themelios_base::span::Location;
use zetesis_core::Term;

use super::{PatternOccurrence, PositivePattern, comparison};
use crate::expansion::Budget;
use crate::formula_ir::LiteralIr;
use crate::{ExpansionResource, FormulaFailure};

/// The variables a comparison waits on, and whether it has been decided.
struct Waiting {
    variables: Vec<usize>,
    decided: bool,
}

impl Waiting {
    fn of(literals: &[LiteralIr], bound: &[bool]) -> Vec<Self> {
        literals
            .iter()
            .filter_map(comparison)
            .map(|(left, _, right)| {
                let variables: Vec<usize> = left.inputs().chain(right.inputs()).collect();
                let decided = variables.iter().all(|&variable| is_bound(bound, variable));
                Self { variables, decided }
            })
            .collect()
    }

    /// Whether binding `occurrence` on top of `bound` decides this comparison.
    fn decided_by(&self, occurrence: &PatternOccurrence<'_>, bound: &[bool]) -> bool {
        !self.decided
            && self
                .variables
                .iter()
                .all(|&variable| is_bound(bound, variable) || binds(occurrence, variable))
    }

    /// The variables this comparison still waits on that `occurrence` binds.
    fn advanced_by(&self, occurrence: &PatternOccurrence<'_>, bound: &[bool]) -> usize {
        if self.decided {
            return 0;
        }
        self.variables
            .iter()
            .filter(|&&variable| !is_bound(bound, variable) && binds(occurrence, variable))
            .count()
    }
}

/// Apply `visit` to every variable slot the occurrence binds when it
/// matches: its whole arguments and, for a structural pattern, the captured
/// subterms.
fn each_slot(occurrence: &PatternOccurrence<'_>, mut visit: impl FnMut(usize)) {
    match occurrence.pattern {
        PositivePattern::Flat(atom) => {
            for term in atom.terms() {
                if let Term::Variable(variable) = term {
                    visit(*variable);
                }
            }
        }
        PositivePattern::Structural(pattern) => pattern.slots().for_each(visit),
    }
}

fn binds(occurrence: &PatternOccurrence<'_>, variable: usize) -> bool {
    let mut found = false;
    each_slot(occurrence, |slot| found |= slot == variable);
    found
}

fn is_test(occurrence: &PatternOccurrence<'_>, bound: &[bool]) -> bool {
    let mut all = true;
    each_slot(occurrence, |slot| all &= is_bound(bound, slot));
    all
}

/// What the prefixes of one join order decide: for each literal, the depth
/// at which its comparison is decided, and whether some check waits for the
/// complete row.
pub(super) struct Decisions {
    /// By literal index: the index of the occurrence, in join order, after
    /// whose match every variable the comparison reads is bound. A variable
    /// the prefix binds is bound before any occurrence, so a comparison over
    /// prefix variables alone is decided at depth zero. `None` marks a
    /// literal that is not a comparison or reads a variable no occurrence
    /// binds, which no prefix decides.
    at: Vec<Option<usize>>,
    /// The deepest decision, so a shallower prefix cannot certify the row.
    last: Option<usize>,
    /// Whether some literal is decided by no prefix: a comparison over a
    /// generated variable, or a guard, range or tuple check that only its
    /// complete-row operation validates.
    on_completion: bool,
}

impl Decisions {
    pub(super) fn of(
        literals: &[LiteralIr],
        occurrences: &[PatternOccurrence<'_>],
        prefix: &[bool],
    ) -> Self {
        let mut bound_at: Vec<Option<usize>> =
            prefix.iter().map(|&bound| bound.then_some(0)).collect();
        for (depth, occurrence) in occurrences.iter().enumerate() {
            each_slot(occurrence, |slot| {
                if let Some(entry) = bound_at.get_mut(slot)
                    && entry.is_none()
                {
                    *entry = Some(depth);
                }
            });
        }
        let at: Vec<Option<usize>> = literals
            .iter()
            .map(|literal| {
                let (left, _, right) = comparison(literal)?;
                left.inputs()
                    .chain(right.inputs())
                    .try_fold(0, |depth, variable| {
                        bound_at
                            .get(variable)
                            .copied()
                            .flatten()
                            .map(|at| depth.max(at))
                    })
            })
            .collect();
        let last = at.iter().filter_map(|depth| *depth).max();
        let on_completion = literals.iter().zip(&at).any(|(literal, decision)| {
            decision.is_none()
                && !matches!(
                    literal,
                    LiteralIr::Atom(..) | LiteralIr::PatternAtom(_) | LiteralIr::ProjectedAtom(..)
                )
        });
        Self {
            at,
            last,
            on_completion,
        }
    }

    /// The literals whose comparisons `depth` decides.
    pub(super) fn decided_at(&self, depth: usize) -> impl Iterator<Item = usize> + '_ {
        self.at
            .iter()
            .enumerate()
            .filter_map(move |(index, at)| (*at == Some(depth)).then_some(index))
    }

    /// Whether a prefix of `depth` occurrences has decided every check, so
    /// its conjunction certifies the row.
    pub(super) fn certifies(&self, depth: usize) -> bool {
        !self.on_completion && self.last <= Some(depth)
    }
}

/// A variable outside the frame is a head or generator slot the positive
/// join never binds.
fn is_bound(bound: &[bool], variable: usize) -> bool {
    bound.get(variable).copied().unwrap_or(false)
}

/// Arrange `occurrences` in join order, given the variables `bound` by the
/// enclosing prefix, the rows offered to each occurrence and the comparisons
/// of `literals`. On return `bound` also holds the variables the occurrences
/// bind.
pub(super) fn arrange(
    occurrences: &mut [PatternOccurrence<'_>],
    literals: &[LiteralIr],
    bound: &mut [bool],
    row_count: impl Fn(&PatternOccurrence<'_>) -> usize,
    budget: &mut Budget,
    location: Location,
) -> Result<(), FormulaFailure> {
    let mut waiting = Waiting::of(literals, bound);
    for position in 0..occurrences.len() {
        let mut best = None;
        for (candidate, occurrence) in occurrences.iter().enumerate().skip(position) {
            let (mut decides, mut advances) = (0, 0);
            for comparison in &waiting {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                decides += usize::from(comparison.decided_by(occurrence, bound));
                advances += comparison.advanced_by(occurrence, bound);
            }
            let rank = (
                !is_test(occurrence, bound),
                row_count(occurrence),
                Reverse(decides),
                Reverse(advances),
                occurrence.source,
            );
            if best.as_ref().is_none_or(|(_, least)| rank < *least) {
                best = Some((candidate, rank));
            }
        }
        let (chosen, _) = best.expect("a candidate remains at every position");
        occurrences.swap(position, chosen);
        each_slot(&occurrences[position], |slot| {
            if let Some(entry) = bound.get_mut(slot) {
                *entry = true;
            }
        });
        for comparison in &mut waiting {
            comparison.decided |= comparison
                .variables
                .iter()
                .all(|&variable| is_bound(bound, variable));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
