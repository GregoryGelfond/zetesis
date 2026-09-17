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

use super::{PatternOccurrence, comparison};
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

fn binds(occurrence: &PatternOccurrence<'_>, variable: usize) -> bool {
    occurrence
        .atom()
        .terms()
        .iter()
        .any(|term| matches!(term, Term::Variable(bound) if *bound == variable))
}

fn is_test(occurrence: &PatternOccurrence<'_>, bound: &[bool]) -> bool {
    occurrence.atom().terms().iter().all(|term| match term {
        Term::Variable(variable) => is_bound(bound, *variable),
        Term::Constant(_) => true,
    })
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
        for term in occurrences[position].atom().terms() {
            if let Term::Variable(variable) = term
                && let Some(slot) = bound.get_mut(*variable)
            {
                *slot = true;
            }
        }
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
