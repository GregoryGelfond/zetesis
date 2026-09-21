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

use super::{PatternOccurrence, PositivePattern, Support, comparison};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::{ExpansionResource, FormulaFailure};
use themelios_program::program::DefaultNegation;

/// Immutable ordering and comparison readiness. Ordinary cursors own this
/// plan and may replace it after partitioning; admitted hybrid scans borrow it.
#[derive(Clone)]
pub(super) struct Plan<'a> {
    pub(super) patterns: Vec<PatternOccurrence<'a>>,
    pub(super) decisions: Decisions,
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        support: &Support<'_>,
        budget: &mut Budget,
        location: Location,
        admit: Option<&mut dyn FnMut(u128) -> Result<(), FormulaFailure>>,
    ) -> Result<Self, FormulaFailure> {
        let mut space = Workspace {
            bytes: 0,
            location: Some(location),
            admit,
        };
        // Universals have separate local scopes. Their conditions cannot bind
        // outer variables, and truth/vacuity over an incomplete support round
        // cannot prune possible heads. Only ordinary positive atoms join here.
        let patterns = literals.iter().enumerate().filter_map(|(source, literal)| {
            let pattern = match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => PositivePattern::Flat(atom),
                LiteralIr::PatternAtom(pattern) => PositivePattern::Structural(pattern),
                _ => return None,
            };
            Some(PatternOccurrence { pattern, source })
        });
        let mut patterns = space.collect(patterns, literals.len())?;
        for pattern in &patterns {
            for term in pattern.atom().terms() {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                if let zetesis_core::Term::Variable(variable) = term
                    && prefix.slots().get(*variable).is_some_and(Option::is_none)
                {
                    return Err(FormulaFailure::UnsafeVariable {
                        variable: *variable,
                        location,
                    });
                }
            }
        }
        let mut prefix = space.collect(prefix.slots().iter().map(Option::is_some), variables)?;
        prefix.resize(variables, false);
        let mut bound = space.collect(prefix.iter().copied(), variables)?;
        arrange_with(
            &mut patterns,
            literals,
            &mut bound,
            |pattern| support.row_count(pattern.atom().predicate()),
            budget,
            &mut space,
        )?;
        let decisions = Decisions::with_space(literals, &patterns, &prefix, &mut space)?;
        Ok(Self {
            patterns,
            decisions,
        })
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.patterns.capacity() as u128 * size_of::<PatternOccurrence<'_>>() as u128
            + self.decisions.at.capacity() as u128 * size_of::<Option<usize>>() as u128
    }
}

/// Prepared vectors reserve once before filling. The cumulative reservation is
/// a conservative preparation peak, including scratch already dropped. Ordinary
/// joins keep their existing collection behavior and allocation boundaries.
struct Workspace<'a> {
    bytes: u128,
    location: Option<Location>,
    admit: Option<&'a mut dyn FnMut(u128) -> Result<(), FormulaFailure>>,
}
impl Workspace<'_> {
    fn collect<T>(
        &mut self,
        values: impl Iterator<Item = T>,
        capacity: usize,
    ) -> Result<Vec<T>, FormulaFailure> {
        if self.admit.is_none() {
            return Ok(values.collect());
        }
        let mut result = self.reserve(capacity)?;
        for value in values {
            assert!(
                result.len() < result.capacity(),
                "declared collection bound"
            );
            result.push(value);
        }
        Ok(result)
    }

    fn reserve<T>(&mut self, capacity: usize) -> Result<Vec<T>, FormulaFailure> {
        let Some(admit) = &mut self.admit else {
            return Ok(Vec::with_capacity(capacity));
        };
        admit(self.bytes + capacity as u128 * size_of::<T>() as u128)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(capacity)
            .map_err(|_| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Allocation,
                location: self
                    .location
                    .expect("bounded preparation has a source location"),
            })?;
        self.bytes += result.capacity() as u128 * size_of::<T>() as u128;
        admit(self.bytes)?;
        Ok(result)
    }
}

/// The variables a comparison waits on, and whether it has been decided.
struct Waiting {
    variables: Vec<usize>,
    decided: bool,
}

impl Waiting {
    fn of(
        literals: &[LiteralIr],
        bound: &[bool],
        space: &mut Workspace<'_>,
    ) -> Result<Vec<Self>, FormulaFailure> {
        // Reserve the outer vector before the inner input vectors so every
        // live allocation is admitted before its first element is written.
        let capacity = if space.admit.is_some() {
            literals.len()
        } else {
            0
        };
        let mut waiting = space.reserve(capacity)?;
        for (left, _, right) in literals.iter().filter_map(comparison) {
            let variables = space.collect(
                left.inputs().chain(right.inputs()),
                left.nodes.len().saturating_add(right.nodes.len()),
            )?;
            let decided = variables.iter().all(|&variable| is_bound(bound, variable));
            waiting.push(Self { variables, decided });
        }
        Ok(waiting)
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
#[derive(Clone)]
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
        Self::with_space(
            literals,
            occurrences,
            prefix,
            &mut Workspace {
                bytes: 0,
                location: None,
                admit: None,
            },
        )
        .expect("unbounded planning performs no fallible reservations")
    }

    fn with_space(
        literals: &[LiteralIr],
        occurrences: &[PatternOccurrence<'_>],
        prefix: &[bool],
        space: &mut Workspace<'_>,
    ) -> Result<Self, FormulaFailure> {
        let mut bound_at =
            space.collect(prefix.iter().map(|&bound| bound.then_some(0)), prefix.len())?;
        for (depth, occurrence) in occurrences.iter().enumerate() {
            each_slot(occurrence, |slot| {
                if let Some(entry) = bound_at.get_mut(slot)
                    && entry.is_none()
                {
                    *entry = Some(depth);
                }
            });
        }
        let at = literals.iter().map(|literal| {
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
        });
        let at = space.collect(at, literals.len())?;
        let last = at.iter().filter_map(|depth| *depth).max();
        let on_completion = literals.iter().zip(&at).any(|(literal, decision)| {
            decision.is_none()
                && !matches!(
                    literal,
                    LiteralIr::Atom(..) | LiteralIr::PatternAtom(_) | LiteralIr::ProjectedAtom(..)
                )
        });
        Ok(Self {
            at,
            last,
            on_completion,
        })
    }

    /// Whether some prefix decides the literal's comparison, so that a
    /// complete row has already passed it.
    pub(super) fn decides(&self, literal: usize) -> bool {
        self.at.get(literal).is_some_and(Option::is_some)
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
    arrange_with(
        occurrences,
        literals,
        bound,
        row_count,
        budget,
        &mut Workspace {
            bytes: 0,
            location: Some(location),
            admit: None,
        },
    )
}

fn arrange_with(
    occurrences: &mut [PatternOccurrence<'_>],
    literals: &[LiteralIr],
    bound: &mut [bool],
    row_count: impl Fn(&PatternOccurrence<'_>) -> usize,
    budget: &mut Budget,
    space: &mut Workspace<'_>,
) -> Result<(), FormulaFailure> {
    let location = space.location.expect("arrangement has a source location");
    let mut waiting = Waiting::of(literals, bound, space)?;
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
