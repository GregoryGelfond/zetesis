//! Source-checked count bounds over original atom coordinates.
//!
//! Guards remain ordered signed occurrences, even when duplicated or
//! contradictory. A guard does not become a fact merely because a bound was
//! recognized. Trivial thresholds and compound contributions stay with the
//! ordinary formula narrower.

use zetesis_algebra::{Quota, Stop};

use super::{
    budget::{Budget, BudgetError},
    conjunction::Conjunction,
    source::{Literal, clause_literal, constraint_body, literal},
    storage::Storage,
    uniform::{BoundSide, UniformBound, uniform_bound},
};
use crate::{AdmissionError, NodeView, Theory};
use zetesis_algebra::words::Words;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    UpperConstraint,
    LowerConstraint,
    Asserted,
    ClauseLower,
    /// A frozen preparation's probed table or clique; never source-checked.
    /// Its `Source` carries no formula coordinates: `root` and `node` hold a
    /// table's index and side, a completing bound's ordinal, or a clique's
    /// two lowest atoms, read only as the comparator's final tiebreak.
    Probed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Source {
    pub kind: Kind,
    pub root: usize,
    pub node: usize,
}

#[derive(Debug)]
pub(super) struct Bound {
    pub support: Words<u64>,
    pub guard: Vec<Literal>,
    pub side: BoundSide,
    pub value: u64,
    pub cardinality: u64,
    pub source: Source,
}

/// Independently read a proposal's original root from its exact admitted owner.
/// Every payload field, including ordered guard occurrences and padded support,
/// must agree with a freshly recognized bound. Temporary readings are released
/// before either successful acceptance or semantic rejection is returned.
pub(super) fn check<Q: Quota + ?Sized>(
    theory: &Theory,
    owner: &Theory,
    candidate: &Bound,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !owner.same_instance(theory) {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    let ordinal = candidate.source.root;
    let mut fresh = read_root(theory, ordinal, storage, budget)?;
    budget.charge(1)?;
    let mut matched = false;
    for actual in &fresh {
        if same_bound(candidate, actual, budget)? {
            budget.charge(1)?;
            matched = true;
            break;
        }
    }
    for bound in fresh.drain(..) {
        release(bound, storage)?;
    }
    storage.release(fresh)?;
    budget.charge(1)?;
    Ok(matched)
}

fn same_bound<Q: Quota + ?Sized>(
    proposed: &Bound,
    actual: &Bound,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if proposed.source.kind != actual.source.kind {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.source.root != actual.source.root {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.source.node != actual.source.node {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.side != actual.side {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.value != actual.value {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.cardinality != actual.cardinality {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.support.len() != actual.support.len() {
        budget.charge(1)?;
        return Ok(false);
    }
    budget.charge(1)?;
    if proposed.guard.len() != actual.guard.len() {
        budget.charge(1)?;
        return Ok(false);
    }
    for (proposed, actual) in proposed.support.iter().zip(&actual.support) {
        budget.charge(1)?;
        if proposed != actual {
            budget.charge(1)?;
            return Ok(false);
        }
    }
    for (proposed, actual) in proposed.guard.iter().zip(&actual.guard) {
        budget.charge(1)?;
        if proposed.atom != actual.atom {
            budget.charge(1)?;
            return Ok(false);
        }
        budget.charge(1)?;
        if proposed.positive != actual.positive {
            budget.charge(1)?;
            return Ok(false);
        }
    }
    budget.charge(1)?;
    Ok(true)
}

/// Propose every admitted count row in this exact asserted root context.
/// The caller independently repeats this reading before accepting a proposal.
pub(super) fn read_root<Q: Quota + ?Sized>(
    theory: &Theory,
    ordinal: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Vec<Bound>, BudgetError<Q::Error>> {
    budget.charge(2)?;
    let root = *theory
        .roots()
        .get(ordinal)
        .ok_or(BudgetError::Source(AdmissionError::Root))?;
    let view = budget.node(theory, root)?;
    let mut bounds = storage.reserve(0, budget)?;
    if let Some(asserted) = asserted_node(theory, root, view, budget)? {
        read_asserted(theory, ordinal, asserted, &mut bounds, storage, budget)?;
    } else if let Some(bound) = read_constraint(theory, (ordinal, root), view, storage, budget)? {
        push_bound(&mut bounds, bound, storage, budget)?;
    }
    budget.charge(1)?;
    Ok(bounds)
}

/// Publish no family prefix: every nested leaf must be an eligible threshold.
/// The local vector is only returned after the complete occurrence walk accepts.
fn read_asserted<'a, Q: Quota + ?Sized>(
    theory: &'a Theory,
    ordinal: usize,
    asserted: (usize, NodeView<'a>),
    bounds: &mut Vec<Bound>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    let mut conjunction = Conjunction::new(storage, budget)?;
    let accepted = conjunction.walk_view(
        theory,
        asserted,
        storage,
        budget,
        |child, view, storage, budget| {
            let Some((node, side)) = asserted_threshold(theory, child, view, budget)? else {
                budget.charge(1)?;
                return Ok(false);
            };
            let guard = storage.reserve(0, budget)?;
            let source = Source {
                kind: Kind::Asserted,
                root: ordinal,
                node,
            };
            let Some(bound) = threshold(theory, node, side, guard, source, storage, budget)? else {
                budget.charge(1)?;
                return Ok(false);
            };
            push_bound(bounds, bound, storage, budget)?;
            budget.charge(1)?;
            Ok(true)
        },
    )?;
    conjunction.release(storage)?;
    budget.charge(1)?;
    if !accepted {
        // These bounds were tentative scratch, never a published family. An
        // unsupported final leaf must not lend authority to its valid prefix.
        for bound in bounds.drain(..) {
            release(bound, storage)?;
        }
    }
    Ok(())
}

fn read_constraint<Q: Quota + ?Sized>(
    theory: &Theory,
    (ordinal, root): (usize, usize),
    view: NodeView<'_>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Bound>, BudgetError<Q::Error>> {
    let Some(body) = constraint_body(theory, view, budget)? else {
        return Ok(None);
    };
    let mut guard = storage.reserve(0, budget)?;
    budget.charge(1)?;
    let mut selected = None;
    budget.charge(1)?;
    let mut clause_only = false;
    let mut conjunction = Conjunction::new(storage, budget)?;
    let accepted = conjunction.walk(
        theory,
        body,
        storage,
        budget,
        |child, child_view, storage, budget| {
            if let Some((node, side)) = constraint_threshold(theory, child, child_view, budget)? {
                budget.charge(1)?;
                if selected.is_some() {
                    budget.charge(1)?;
                    return Ok(false);
                }
                budget.charge(3)?;
                selected = Some((node, side));
            } else if let Some(literal) = literal(theory, child_view, budget)? {
                storage.push(&mut guard, literal, 2, budget)?;
            } else if let Some(value) = clause_literal(theory, child, child_view, budget)? {
                budget.charge(1)?;
                clause_only = true;
                storage.push(&mut guard, value, 2, budget)?;
            } else {
                budget.charge(1)?;
                return Ok(false);
            }
            budget.charge(1)?;
            Ok(true)
        },
    )?;
    conjunction.release(storage)?;
    budget.charge(1)?;
    if !accepted {
        storage.release(guard)?;
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(1)?;
    if selected.is_some() {
        budget.charge(1)?;
        if clause_only {
            storage.release(guard)?;
            budget.charge(1)?;
            return Ok(None);
        }
    }
    budget.charge(1)?;
    if let Some((node, side)) = selected {
        budget.charge(1)?;
        let kind = match side {
            BoundSide::Lower => Kind::LowerConstraint,
            BoundSide::Upper => Kind::UpperConstraint,
        };
        threshold(
            theory,
            node,
            side,
            guard,
            Source {
                kind,
                root: ordinal,
                node,
            },
            storage,
            budget,
        )
    } else {
        clause_lower(
            theory,
            guard,
            Source {
                kind: Kind::ClauseLower,
                root: ordinal,
                node: root,
            },
            storage,
            budget,
        )
    }
}

/// At most one outer double negation. This helper does not flatten formulas.
fn asserted_node<'a, Q: Quota + ?Sized>(
    theory: &'a Theory,
    node: usize,
    view: NodeView<'a>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<(usize, NodeView<'a>)>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if matches!(view, NodeView::Threshold(_) | NodeView::And(_)) {
        budget.charge(2)?;
        return Ok(Some((node, view)));
    }
    let Some(node) = double_negative(theory, view, budget)? else {
        budget.charge(1)?;
        return Ok(None);
    };
    let inside = budget.node(theory, node)?;
    budget.charge(1)?;
    if matches!(inside, NodeView::Threshold(_) | NodeView::And(_)) {
        budget.charge(2)?;
        Ok(Some((node, inside)))
    } else {
        budget.charge(1)?;
        Ok(None)
    }
}

/// One positive or negated threshold leaf; retain the earlier double-negated
/// positive leaf as well. Every negation is authenticated against its False
/// consequent, and no conjunction or further negation is hidden inside a leaf.
fn asserted_threshold<Q: Quota + ?Sized>(
    theory: &Theory,
    node: usize,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<(usize, BoundSide)>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if matches!(view, NodeView::Threshold(_)) {
        budget.charge(3)?;
        return Ok(Some((node, BoundSide::Lower)));
    }
    budget.charge(1)?;
    let NodeView::Implies(inner, right) = view else {
        budget.charge(1)?;
        return Ok(None);
    };
    let right = budget.node(theory, right)?;
    budget.charge(1)?;
    if !matches!(right, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    let inner_view = budget.node(theory, inner)?;
    budget.charge(1)?;
    if matches!(inner_view, NodeView::Threshold(_)) {
        budget.charge(3)?;
        return Ok(Some((inner, BoundSide::Upper)));
    }
    budget.charge(1)?;
    let NodeView::Implies(node, right) = inner_view else {
        budget.charge(1)?;
        return Ok(None);
    };
    let right = budget.node(theory, right)?;
    budget.charge(1)?;
    if !matches!(right, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    let inside = budget.node(theory, node)?;
    budget.charge(1)?;
    if matches!(inside, NodeView::Threshold(_)) {
        budget.charge(3)?;
        Ok(Some((node, BoundSide::Lower)))
    } else {
        budget.charge(1)?;
        Ok(None)
    }
}

fn double_negative<Q: Quota + ?Sized>(
    theory: &Theory,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<usize>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let NodeView::Implies(inner, right) = view else {
        budget.charge(1)?;
        return Ok(None);
    };
    let right = budget.node(theory, right)?;
    budget.charge(1)?;
    if !matches!(right, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    let inner = budget.node(theory, inner)?;
    budget.charge(1)?;
    let NodeView::Implies(node, right) = inner else {
        budget.charge(1)?;
        return Ok(None);
    };
    let right = budget.node(theory, right)?;
    budget.charge(1)?;
    if !matches!(right, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(2)?;
    Ok(Some(node))
}

fn constraint_threshold<Q: Quota + ?Sized>(
    theory: &Theory,
    node: usize,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<(usize, BoundSide)>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if matches!(view, NodeView::Threshold(_)) {
        budget.charge(3)?;
        return Ok(Some((node, BoundSide::Upper)));
    }
    budget.charge(1)?;
    let NodeView::Implies(left, right) = view else {
        budget.charge(1)?;
        return Ok(None);
    };
    let right = budget.node(theory, right)?;
    budget.charge(1)?;
    if !matches!(right, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    let left_view = budget.node(theory, left)?;
    budget.charge(1)?;
    if matches!(left_view, NodeView::Threshold(_)) {
        budget.charge(3)?;
        Ok(Some((left, BoundSide::Lower)))
    } else {
        budget.charge(1)?;
        Ok(None)
    }
}

fn threshold<Q: Quota + ?Sized>(
    theory: &Theory,
    node: usize,
    side: BoundSide,
    guard: Vec<Literal>,
    source: Source,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Bound>, BudgetError<Q::Error>> {
    let view = budget.node(theory, node)?;
    budget.charge(1)?;
    let NodeView::Threshold(row) = view else {
        storage.release(guard)?;
        budget.charge(1)?;
        return Ok(None);
    };
    let mut support =
        Words::new(storage.filled(theory.atom_count().div_ceil(128) * 2, 0_u64, 1, budget)?);
    let decoded = threshold_content(theory, row, &mut support, side, budget)?;
    budget.charge(1)?;
    if let Some((value, cardinality)) = decoded {
        budget.charge(7)?;
        Ok(Some(Bound {
            support,
            guard,
            side,
            value,
            cardinality,
            source,
        }))
    } else {
        storage.release(support.into_storage())?;
        storage.release(guard)?;
        budget.charge(1)?;
        Ok(None)
    }
}

fn threshold_content<Q: Quota + ?Sized>(
    theory: &Theory,
    row: crate::Threshold<'_>,
    support: &mut [u64],
    side: BoundSide,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<(u64, u64)>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if row.conditions.len() != row.weights.len() {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(1)?;
    if row.conditions.is_empty() {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(2)?;
    let weight = row.weights[0];
    if weight == 0 {
        budget.charge(1)?;
        return Ok(None);
    }
    for (condition, value) in row.conditions.iter().zip(row.weights) {
        budget.charge(1)?;
        if *value != weight {
            budget.charge(1)?;
            return Ok(None);
        }
        budget.charge(1)?;
        let condition = budget.node(theory, *condition)?;
        budget.charge(1)?;
        let NodeView::Atom(atom) = condition else {
            budget.charge(1)?;
            return Ok(None);
        };
        budget.charge(1)?;
        if atom >= theory.atom_count() {
            return Err(BudgetError::Source(AdmissionError::Atom));
        }
        budget.charge(5)?;
        let (word, bit) = Words::<u64>::locate(atom);
        if support[word] & bit != 0 {
            budget.charge(1)?;
            return Ok(None);
        }
        budget.charge(1)?;
        support[word] |= bit;
    }
    budget.charge(1)?;
    let cardinality = u64::try_from(row.conditions.len())
        .map_err(|_| BudgetError::Source(AdmissionError::Limit))?;
    let normalized = uniform_bound(row.bound, weight, cardinality, side, &mut |work| {
        budget.charge(work)
    })?;
    budget.charge(1)?;
    match normalized {
        UniformBound::Valid { value } => {
            budget.charge(3)?;
            Ok(Some((value, cardinality)))
        }
        UniformBound::Declined(_) => {
            budget.charge(1)?;
            Ok(None)
        }
    }
}

fn clause_lower<Q: Quota + ?Sized>(
    theory: &Theory,
    literals: Vec<Literal>,
    source: Source,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Bound>, BudgetError<Q::Error>> {
    let mut support =
        Words::new(storage.filled(theory.atom_count().div_ceil(128) * 2, 0_u64, 1, budget)?);
    for literal in &literals {
        budget.charge(1)?;
        if literal.positive {
            storage.release(support.into_storage())?;
            storage.release(literals)?;
            budget.charge(1)?;
            return Ok(None);
        }
        budget.charge(5)?;
        let (word, bit) = Words::<u64>::locate(literal.atom);
        if support[word] & bit != 0 {
            storage.release(support.into_storage())?;
            storage.release(literals)?;
            budget.charge(1)?;
            return Ok(None);
        }
        budget.charge(1)?;
        support[word] |= bit;
    }
    budget.charge(1)?;
    let cardinality =
        u64::try_from(literals.len()).map_err(|_| BudgetError::Source(AdmissionError::Limit))?;
    storage.release(literals)?;
    let guard = storage.reserve(0, budget)?;
    budget.charge(7)?;
    Ok(Some(Bound {
        support,
        guard,
        side: BoundSide::Lower,
        value: 1,
        cardinality,
        source,
    }))
}

pub(super) fn release(bound: Bound, storage: &mut Storage) -> Result<(), Stop> {
    storage.release_embedded(bound.support.into_storage())?;
    storage.release_embedded(bound.guard)
}

/// Payload headers are contained by the outer row allocation once published.
pub(super) fn push_bound<Q: Quota + ?Sized>(
    bounds: &mut Vec<Bound>,
    bound: Bound,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    storage.push(bounds, bound, 12, budget)?;
    storage.embed::<u64>()?;
    storage.embed::<Literal>().map_err(BudgetError::Stopped)
}
