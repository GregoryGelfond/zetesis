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
//! does not read this order. Each arrangement resolves offered row counts once.
//! Rank evaluation, variable metadata scans and checked predicate comparisons
//! consume term work before inspection, including when no comparison waits.

use std::cmp::Reverse;

use themelios_base::span::Location;
use zetesis_core::TemplateTerm;

use super::{PatternOccurrence, PositivePattern, Relations, comparison, delta};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::{ExpansionResource, FormulaFailure};
use themelios_program::program::DefaultNegation;

/// The populations offered by one immutable support snapshot. A delta pivot
/// names an original source occurrence, independently of the chosen join order.
#[derive(Clone, Copy)]
pub(super) struct SourceRows<'view, 'source> {
    pub(super) relations: &'view Relations<'source>,
    pub(super) pivot: Option<usize>,
}

/// Immutable ordering and comparison readiness, built once for the offered
/// populations. Ordinary cursors own this plan; admitted hybrid scans borrow it.
#[derive(Clone)]
pub(super) struct Plan<'a> {
    pub(super) patterns: Vec<PatternOccurrence<'a>>,
    pub(super) decisions: Decisions,
    pub(super) pivot: Option<usize>,
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        literals: &'a [LiteralIr],
        prefix: &Binding,
        variables: usize,
        rows: SourceRows<'_, 'a>,
        budget: &mut Budget,
        location: Location,
        admit: Option<&mut dyn FnMut(Capacity) -> Result<(), FormulaFailure>>,
    ) -> Result<Self, FormulaFailure> {
        let mut space = Workspace {
            bytes: 0,
            location: Some(location),
            admit,
        };
        // Universals have separate local scopes. Their conditions cannot bind
        // outer variables, and truth/vacuity over an incomplete support round
        // cannot prune possible heads. Only ordinary positive atoms join here.
        let mut patterns = space.reserve(literals.len())?;
        for (source, literal) in literals.iter().enumerate() {
            work(budget, 1, location)?;
            let flat = match literal {
                LiteralIr::Atom(DefaultNegation::None, atom) => *atom,
                LiteralIr::PatternAtom(pattern) => pattern.atom,
                _ => continue,
            };
            let components = rows
                .relations
                .components()
                .ok_or_else(|| super::components::missing(location))?;
            let flat = flat.get_with(components, location, || {
                budget
                    .charge(ExpansionResource::TermWork, 1, location)
                    .map_err(Into::into)
            })?;
            let pattern = match literal {
                LiteralIr::Atom(..) => PositivePattern::Flat(flat),
                LiteralIr::PatternAtom(pattern) => PositivePattern::Structural(pattern.bind(flat)),
                _ => unreachable!("selected positive occurrence"),
            };
            patterns.push(PatternOccurrence { pattern, source });
        }
        for pattern in &patterns {
            work(budget, 1, location)?;
            let terms = pattern.atom().terms();
            for column in 0..terms.len() {
                budget.charge(ExpansionResource::TermWork, 1, location)?;
                let term = terms.at(column).expect("checked pattern arity");
                if let TemplateTerm::Variable(variable) = term
                    && variable < prefix.len()
                    && !prefix.is_bound(variable, location)?
                {
                    return Err(FormulaFailure::UnsafeVariable { variable, location });
                }
            }
        }
        work(budget, prefix.len() as u128, location)?;
        let mut prefix = space.collect(
            (0..prefix.len()).map(|slot| {
                prefix
                    .slots()
                    .is_bound(slot)
                    .expect("slot is inside the selected prefix")
            }),
            variables,
        )?;
        work(budget, variables as u128, location)?;
        prefix.resize(variables, false);
        work(budget, prefix.len() as u128, location)?;
        let mut bound = space.collect(prefix.iter().copied(), variables)?;
        arrange_with(
            &mut patterns,
            literals,
            &mut bound,
            |pattern, budget| {
                rows.relations
                    .row_counts_with(pattern.atom().predicate(), || work(budget, 1, location))
                    .map(|(old, total)| {
                        delta::interval(rows.pivot, pattern.source, old, total).len()
                    })
            },
            budget,
            &mut space,
        )?;
        let decisions = Decisions::with_space(literals, &patterns, &prefix, budget, &mut space)?;
        Ok(Self {
            patterns,
            decisions,
            pivot: rows.pivot,
        })
    }

    pub(super) fn retained_bytes(&self) -> u128 {
        self.patterns.capacity() as u128 * size_of::<PatternOccurrence<'_>>() as u128
            + self.decisions.at.capacity() as u128 * size_of::<Option<usize>>() as u128
    }
}

/// One cumulative planner capacity boundary. Requested capacity is checked
/// before allocation; allocated capacity is observed only after it succeeds.
#[derive(Clone, Copy)]
pub(super) enum Capacity {
    Requested(u128),
    Allocated(u128),
}

/// Prepared vectors reserve once before filling. The cumulative reservation is
/// a conservative preparation peak, including scratch already dropped. Only
/// successful allocations contribute to its observed capacity.
struct Workspace<'a> {
    bytes: u128,
    location: Option<Location>,
    admit: Option<&'a mut dyn FnMut(Capacity) -> Result<(), FormulaFailure>>,
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
        admit(Capacity::Requested(
            self.bytes + capacity as u128 * size_of::<T>() as u128,
        ))?;
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
        admit(Capacity::Allocated(self.bytes))?;
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
        budget: &mut Budget,
        space: &mut Workspace<'_>,
    ) -> Result<Vec<Self>, FormulaFailure> {
        let location = space.location.expect("arrangement has a source location");
        // Reserve outer storage first; expression scans are charged before the
        // filtered input iterator can skip any nonvariable operations.
        let capacity = if space.admit.is_some() {
            literals.len()
        } else {
            0
        };
        let mut waiting = space.reserve(capacity)?;
        for literal in literals {
            work(budget, 1, location)?;
            let Some((left, _, right)) = comparison(literal) else {
                continue;
            };
            work(
                budget,
                left.nodes.len() as u128 + right.nodes.len() as u128,
                location,
            )?;
            let variables = space.collect(
                left.inputs().chain(right.inputs()),
                left.nodes.len().saturating_add(right.nodes.len()),
            )?;
            work(budget, variables.len() as u128, location)?;
            let decided = variables.iter().all(|&variable| is_bound(bound, variable));
            waiting.push(Self { variables, decided });
        }
        Ok(waiting)
    }

    fn decided_by(
        &self,
        occurrence: &PatternOccurrence<'_>,
        bound: &[bool],
        budget: &mut Budget,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if self.decided {
            return Ok(false);
        }
        for &variable in &self.variables {
            work(budget, 1, location)?;
            if !is_bound(bound, variable) && !binds(occurrence, variable, budget, location)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn advanced_by(
        &self,
        occurrence: &PatternOccurrence<'_>,
        bound: &[bool],
        budget: &mut Budget,
        location: Location,
    ) -> Result<usize, FormulaFailure> {
        if self.decided {
            return Ok(0);
        }
        let mut count = 0;
        for &variable in &self.variables {
            work(budget, 1, location)?;
            if !is_bound(bound, variable) && binds(occurrence, variable, budget, location)? {
                count += 1;
            }
        }
        Ok(count)
    }
}

fn work(budget: &mut Budget, amount: u128, location: Location) -> Result<(), FormulaFailure> {
    budget
        .charge(ExpansionResource::TermWork, amount, location)
        .map_err(Into::into)
}

/// Pre-admit the complete metadata scan before applying any slot update.
/// Structural compilation retains at most one argument plan per flat column,
/// so the flat width also bounds the header scan that obtains its node count.
fn each_slot(
    occurrence: &PatternOccurrence<'_>,
    budget: &mut Budget,
    location: Location,
    visit: impl FnMut(usize),
) -> Result<(), FormulaFailure> {
    work(budget, 1, location)?;
    let width = occurrence.atom().terms().len();
    work(budget, width as u128, location)?;
    match occurrence.pattern {
        PositivePattern::Flat(atom) => atom.terms().variables().for_each(visit),
        PositivePattern::Structural(pattern) => {
            let nodes = pattern.node_count();
            work(budget, nodes as u128, location)?;
            pattern.slots().for_each(visit);
        }
    }
    Ok(())
}

fn binds(
    occurrence: &PatternOccurrence<'_>,
    variable: usize,
    budget: &mut Budget,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let mut found = false;
    each_slot(occurrence, budget, location, |slot| {
        found |= slot == variable;
    })?;
    Ok(found)
}

fn is_test(
    occurrence: &PatternOccurrence<'_>,
    bound: &[bool],
    budget: &mut Budget,
    location: Location,
) -> Result<bool, FormulaFailure> {
    let mut all = true;
    each_slot(occurrence, budget, location, |slot| {
        all &= is_bound(bound, slot);
    })?;
    Ok(all)
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
    pub(super) fn checked(
        literals: &[LiteralIr],
        occurrences: &[PatternOccurrence<'_>],
        prefix: &[bool],
        budget: &mut Budget,
        location: Location,
        admit: &mut dyn FnMut(Capacity) -> Result<(), FormulaFailure>,
    ) -> Result<Self, FormulaFailure> {
        Self::with_space(
            literals,
            occurrences,
            prefix,
            budget,
            &mut Workspace {
                bytes: 0,
                location: Some(location),
                admit: Some(admit),
            },
        )
    }

    fn with_space(
        literals: &[LiteralIr],
        occurrences: &[PatternOccurrence<'_>],
        prefix: &[bool],
        budget: &mut Budget,
        space: &mut Workspace<'_>,
    ) -> Result<Self, FormulaFailure> {
        let location = space.location.expect("decisions have a source location");
        work(budget, prefix.len() as u128, location)?;
        let mut bound_at =
            space.collect(prefix.iter().map(|&bound| bound.then_some(0)), prefix.len())?;
        for (depth, occurrence) in occurrences.iter().enumerate() {
            each_slot(occurrence, budget, location, |slot| {
                if let Some(entry) = bound_at.get_mut(slot)
                    && entry.is_none()
                {
                    *entry = Some(depth);
                }
            })?;
        }
        let mut at = space.reserve(literals.len())?;
        for literal in literals {
            work(budget, 1, location)?;
            let decision = if let Some((left, _, right)) = comparison(literal) {
                work(
                    budget,
                    left.nodes.len() as u128 + right.nodes.len() as u128,
                    location,
                )?;
                left.inputs()
                    .chain(right.inputs())
                    .try_fold(0, |depth, variable| {
                        bound_at
                            .get(variable)
                            .copied()
                            .flatten()
                            .map(|at| depth.max(at))
                    })
            } else {
                None
            };
            at.push(decision);
        }
        work(budget, at.len() as u128, location)?;
        let last = at.iter().filter_map(|depth| *depth).max();
        work(budget, literals.len() as u128, location)?;
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
#[cfg(test)]
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
        |occurrence, _| Ok(row_count(occurrence)),
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
    mut row_count: impl FnMut(&PatternOccurrence<'_>, &mut Budget) -> Result<usize, FormulaFailure>,
    budget: &mut Budget,
    space: &mut Workspace<'_>,
) -> Result<(), FormulaFailure> {
    let location = space.location.expect("arrangement has a source location");
    let mut offered = space.reserve(occurrences.len())?;
    for occurrence in occurrences.iter() {
        work(budget, 1, location)?;
        offered.push(row_count(occurrence, budget)?);
    }
    let mut waiting = Waiting::of(literals, bound, budget, space)?;
    for position in 0..occurrences.len() {
        let mut best = None;
        for (candidate, occurrence) in occurrences.iter().enumerate().skip(position) {
            work(budget, 1, location)?;
            let (mut decides, mut advances) = (0, 0);
            for comparison in &waiting {
                work(budget, 1, location)?;
                decides += usize::from(comparison.decided_by(occurrence, bound, budget, location)?);
                advances += comparison.advanced_by(occurrence, bound, budget, location)?;
            }
            let rank = (
                !is_test(occurrence, bound, budget, location)?,
                offered[candidate],
                Reverse(decides),
                Reverse(advances),
                occurrence.source,
            );
            if best.as_ref().is_none_or(|(_, least)| rank < *least) {
                best = Some((candidate, rank));
            }
        }
        work(budget, 1, location)?;
        let (chosen, _) = best.expect("a candidate remains at every position");
        occurrences.swap(position, chosen);
        offered.swap(position, chosen);
        each_slot(&occurrences[position], budget, location, |slot| {
            if let Some(entry) = bound.get_mut(slot) {
                *entry = true;
            }
        })?;
        for comparison in &mut waiting {
            work(budget, 1 + comparison.variables.len() as u128, location)?;
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
