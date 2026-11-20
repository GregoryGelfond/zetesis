//! Set-at-a-time binary Join for interned term bodies.
//!
//! Bodies that are positive atoms whose arguments are canonical catalog
//! terms (numbers or symbols), plus residual comparisons and simple bound
//! default-negation, and that connect by shared variables, are evaluated
//! with the columnar Join in `zetesis-algebra` instead of the row-at-a-time
//! cursor. Join keys are interned `TermId` words from the support snapshot.
//! Range-and-arithmetic rules stay on the Bind path, which is tried first.
//! Cross products, aggregates, non-canonical columns, and anything this
//! compiler cannot name are declined so the Join cursor keeps their
//! admission and work. Recursive predicates are admitted against the
//! current snapshot. The output is sorted, so the CPU path is deterministic
//! at every thread count. Emit assigns each interned term with
//! `Binding::set_term_with`, charging work exactly as the Join cursor does
//! when it binds a support atom.

use std::collections::HashMap;

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_algebra::{Atoms, Bindings, JoinMethod, JoinStats, join};
use zetesis_core::TemplateTerm;
use zetesis_core::catalog::{PredicateRef, TermRef};

use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::{Computation, Counters, Support};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Default realization is sorted merge. A measurement binary may be built
/// with `ZETESIS_JOIN_METHOD=hash` at compile time to compare the hash
/// join; both methods must produce the same bindings, in the same lex order.
fn method() -> JoinMethod {
    match option_env!("ZETESIS_JOIN_METHOD") {
        Some("hash") => JoinMethod::Hash,
        _ => JoinMethod::SortedMerge,
    }
}

/// True when a normal head also occurs as a positive body atom, so this
/// is a recursive producer. Fact-only one-shot joins stay on the expanding
/// Join generators, which own the parallel dispatch contract.
pub(crate) fn body_repeats_head(rule: &RuleIr, support: &Support<'_>) -> bool {
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return false;
    };
    let Some(view) = support.components() else {
        return false;
    };
    let Some(pattern) = head.resolve(view) else {
        return false;
    };
    let predicate = pattern.predicate();
    rule.body.iter().any(|literal| match literal {
        LiteralIr::Atom(DefaultNegation::None, atom) => atom
            .resolve(view)
            .is_some_and(|atom| atom.predicate() == predicate),
        _ => false,
    })
}

/// Columnar bindings together with the interned terms their keys denote.
pub(crate) struct Evaluated<'a> {
    /// Projected body variables, keyed by interned catalog identity.
    pub(crate) bindings: Bindings,
    terms: TermMap<'a>,
}

impl Evaluated<'_> {
    /// Assign one output row through `set_term_with`, charging work as the
    /// Join cursor does when it binds a support atom.
    pub(crate) fn bind_row(
        &self,
        (computation, limits, counters, location): (
            &mut Computation<'_, '_>,
            &FormulaLimits,
            &mut Counters,
            ProgramSite,
        ),
        assignment: &mut Binding<'static>,
        row: usize,
        buffer: &mut [i32],
    ) -> Result<(), FormulaFailure> {
        self.bindings.row_into(row, buffer);
        for (slot, &key) in buffer.iter().enumerate() {
            let term = self
                .terms
                .get(key)
                .expect("join keys are interned from the snapshot");
            assignment.set_term_with(slot, term, computation.read(), location, || {
                counters.work(limits, location)
            })?;
        }
        Ok(())
    }
}

/// Evaluate `rule` to columnar bindings, or `Ok(None)` when declined.
pub(crate) fn evaluate<'a>(
    rule: &'a RuleIr,
    support: &'a Support<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Option<Evaluated<'a>>, FormulaFailure> {
    let Some(plan) = plan(rule, support) else {
        return Ok(None);
    };
    // Default negation is declined until visits can wait for a completed
    // negated relation. An empty snapshot looks stable (`old_rows ==
    // row_count == 0`) and would otherwise walk through later-blocked
    // parents, as observed on stratrec10k.
    if !plan.negated.is_empty() {
        return Ok(None);
    }
    let mut account = |amount: u64| charge(counters, limits, location, amount);
    let mut terms = TermMap::default();
    let recursive = recursive_atoms(rule, support, &plan.atoms);
    let Some(projected) = semi_naive(&plan, &recursive, support, &mut terms, &mut account)? else {
        return Ok(None);
    };
    let Some(filtered) = apply_residuals(&projected, &plan, support, &mut terms, &mut account)?
    else {
        return Ok(None);
    };
    Ok(Some(Evaluated {
        bindings: filtered,
        terms,
    }))
}

fn semi_naive<'a>(
    plan: &Plan<'a>,
    recursive: &[usize],
    support: &'a Support<'_>,
    terms: &mut TermMap<'a>,
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<Bindings>, FormulaFailure> {
    let mut bounds_list = delta_bounds(&plan.atoms, support, recursive);
    if bounds_list.is_empty() {
        bounds_list.push(vec![(0, None); plan.atoms.len()]);
        // No recursive delta and no full fallback: a rec-only visit with
        // no new rows should emit nothing, not rejoin the snapshot.
        if !recursive.is_empty() {
            return Ok(Some(Bindings::empty(plan.width)));
        }
    }
    let mut rows = Vec::new();
    for bounds in &bounds_list {
        let Some((table, slots)) = join_atoms(&plan.atoms, bounds, support, terms, account)? else {
            return Ok(None);
        };
        let Some(projected) = project_slots(&table, &slots, plan.width, account)? else {
            return Ok(None);
        };
        rows.extend(projected.to_rows());
    }
    let mut columns = vec![Vec::with_capacity(rows.len()); plan.width];
    for row in &rows {
        account(1)?;
        for (column, &value) in columns.iter_mut().zip(row.iter()) {
            column.push(value);
        }
    }
    match Bindings::from_columns(columns) {
        Ok(bindings) => Ok(Some(bindings)),
        Err(_) => Ok(None),
    }
}

fn recursive_atoms(rule: &RuleIr, support: &Support<'_>, atoms: &[PlannedAtom<'_>]) -> Vec<usize> {
    let HeadIr::Normal(Some(head)) = &rule.head else {
        return Vec::new();
    };
    let Some(view) = support.components() else {
        return Vec::new();
    };
    let Some(pattern) = head.resolve(view) else {
        return Vec::new();
    };
    let predicate = pattern.predicate();
    atoms
        .iter()
        .enumerate()
        .filter(|(_, atom)| atom.predicate == predicate)
        .map(|(index, _)| index)
        .collect()
}

fn delta_bounds(
    atoms: &[PlannedAtom<'_>],
    support: &Support<'_>,
    recursive: &[usize],
) -> Vec<Vec<(usize, Option<usize>)>> {
    let mut parts = Vec::new();
    for (k, &index) in recursive.iter().enumerate() {
        let old = support.old_rows(atoms[index].predicate);
        let count = support.row_count(atoms[index].predicate);
        if old >= count {
            continue;
        }
        let mut bounds = vec![(0, None); atoms.len()];
        for &previous in recursive.iter().take(k) {
            bounds[previous] = (0, Some(support.old_rows(atoms[previous].predicate)));
        }
        bounds[index] = (old, None);
        parts.push(bounds);
    }
    parts
}

fn join_atoms<'a>(
    atoms: &[PlannedAtom<'a>],
    bounds: &[(usize, Option<usize>)],
    support: &'a Support<'_>,
    terms: &mut TermMap<'a>,
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<(Atoms, Vec<usize>)>, FormulaFailure> {
    let mut current: Option<(Atoms, Vec<usize>)> = None;
    for (atom, &(from_row, until_row)) in atoms.iter().zip(bounds) {
        let Some(table) = term_table(
            support,
            atom.predicate,
            from_row,
            until_row,
            atom.arity,
            terms,
        ) else {
            return Ok(None);
        };
        current = Some(match current {
            None => (table, atom.slots.clone()),
            Some((left, left_slots)) => {
                let Some((left_keys, right_keys)) = key_columns(&left_slots, &atom.slots) else {
                    return Ok(None);
                };
                let mut stats = JoinStats::default();
                let Ok(joined) = join(
                    &left,
                    &table,
                    &left_keys,
                    &right_keys,
                    method(),
                    account,
                    &mut stats,
                )?
                else {
                    return Ok(None);
                };
                let slots = [left_slots.as_slice(), atom.slots.as_slice()].concat();
                (bindings_to_atoms(&joined), slots)
            }
        });
    }
    Ok(current)
}

/// Interned catalog identities used as join keys, with a resolver for emit
/// and for ordered residual comparisons (identity order is not term order).
#[derive(Default)]
struct TermMap<'a> {
    witness: Option<TermRef<'a>>,
    by_key: HashMap<i32, TermRef<'a>>,
}

impl<'a> TermMap<'a> {
    fn insert(&mut self, term: TermRef<'a>) -> Option<i32> {
        let key = term_key(term)?;
        if let Some(witness) = self.witness {
            if witness.same_vocabulary(term) != Some(true) {
                return None;
            }
        } else {
            self.witness = Some(term);
        }
        self.by_key.entry(key).or_insert(term);
        Some(key)
    }

    fn get(&self, key: i32) -> Option<TermRef<'a>> {
        self.by_key.get(&key).copied()
    }
}

/// Bit-preserving identity of a canonical catalog term. Ids above
/// `i32::MAX` stay distinct so a merge join still groups equal keys.
fn term_key(term: TermRef<'_>) -> Option<i32> {
    let position = term.canonical_position()?;
    let word = u32::try_from(position).ok()?;
    Some(i32::from_ne_bytes(word.to_ne_bytes()))
}

struct Plan<'a> {
    width: usize,
    atoms: Vec<PlannedAtom<'a>>,
    compares: Vec<Compare>,
    negated: Vec<Negated<'a>>,
}

#[derive(Clone)]
struct PlannedAtom<'a> {
    predicate: PredicateRef<'a>,
    arity: usize,
    /// Slot index for each column, or `usize::MAX` for a constant already
    /// applied as a filter when the table was loaded (not used: constants
    /// decline in v1).
    slots: Vec<usize>,
}

struct Compare {
    left: usize,
    right: usize,
    relation: Relation,
}

struct Negated<'a> {
    predicate: PredicateRef<'a>,
    slots: Vec<usize>,
}

fn plan<'a>(rule: &'a RuleIr, support: &'a Support<'_>) -> Option<Plan<'a>> {
    if !matches!(rule.head, HeadIr::Normal(Some(_))) {
        return None;
    }
    let view = support.components()?;
    let (atoms, compares, negated) = body_parts(rule, view)?;
    if atoms.len() < 2 || !connected(&atoms) {
        return None;
    }
    // Reorder into a connected left-deep sequence so each binary step has
    // a shared-variable key and is not a cross product.
    let atoms = order_connected(atoms)?;
    residuals_are_bound(&atoms, &compares, &negated)?;
    Some(Plan {
        width: rule.body_variables,
        atoms,
        compares,
        negated,
    })
}

fn body_parts<'a>(
    rule: &'a RuleIr,
    view: zetesis_core::TemplateComponentsRef<'a>,
) -> Option<(Vec<PlannedAtom<'a>>, Vec<Compare>, Vec<Negated<'a>>)> {
    let mut atoms = Vec::new();
    let mut compares = Vec::new();
    let mut negated = Vec::new();
    for literal in &rule.body {
        match literal {
            LiteralIr::Atom(DefaultNegation::None, pattern) => {
                let pattern = pattern.resolve(view)?;
                if pattern.predicate().arity() == 0 {
                    return None;
                }
                let mut slots = Vec::with_capacity(pattern.predicate().arity());
                for index in 0..pattern.predicate().arity() {
                    match pattern.terms().at(index)? {
                        TemplateTerm::Variable(slot) => slots.push(slot),
                        TemplateTerm::Constant(_) => return None,
                    }
                }
                if repeated_slot(&slots) {
                    return None;
                }
                atoms.push(PlannedAtom {
                    predicate: pattern.predicate(),
                    arity: pattern.predicate().arity(),
                    slots,
                });
            }
            LiteralIr::Atom(DefaultNegation::Not, pattern) => {
                let pattern = pattern.resolve(view)?;
                let mut slots = Vec::with_capacity(pattern.predicate().arity());
                for index in 0..pattern.predicate().arity() {
                    match pattern.terms().at(index)? {
                        TemplateTerm::Variable(slot) => slots.push(slot),
                        TemplateTerm::Constant(_) => return None,
                    }
                }
                negated.push(Negated {
                    predicate: pattern.predicate(),
                    slots,
                });
            }
            LiteralIr::Compare(comparison) => {
                let left = slot(comparison.left())?;
                let right = slot(comparison.right())?;
                compares.push(Compare {
                    left,
                    right,
                    relation: comparison.relation(),
                });
            }
            LiteralIr::Atom(..)
            | LiteralIr::PatternAtom(_)
            | LiteralIr::AnonymousAtom(..)
            | LiteralIr::ArgumentCheck(_)
            | LiteralIr::TupleCompare(..)
            | LiteralIr::Guard(_)
            | LiteralIr::HeadGuard(_)
            | LiteralIr::Conditional(_)
            | LiteralIr::Aggregate(_)
            | LiteralIr::Bind { .. }
            | LiteralIr::Range { .. } => return None,
        }
    }
    Some((atoms, compares, negated))
}

fn repeated_slot(slots: &[usize]) -> bool {
    slots
        .iter()
        .enumerate()
        .any(|(index, slot)| slots[..index].contains(slot))
}

fn residuals_are_bound(
    atoms: &[PlannedAtom<'_>],
    compares: &[Compare],
    negated: &[Negated<'_>],
) -> Option<()> {
    let bound: std::collections::BTreeSet<usize> = atoms
        .iter()
        .flat_map(|atom| atom.slots.iter().copied())
        .collect();
    if compares
        .iter()
        .any(|compare| !bound.contains(&compare.left) || !bound.contains(&compare.right))
    {
        return None;
    }
    if negated
        .iter()
        .any(|atom| atom.slots.iter().any(|slot| !bound.contains(slot)))
    {
        return None;
    }
    Some(())
}

fn connected(atoms: &[PlannedAtom<'_>]) -> bool {
    order_connected(atoms.to_vec()).is_some()
}

fn order_connected(mut atoms: Vec<PlannedAtom<'_>>) -> Option<Vec<PlannedAtom<'_>>> {
    if atoms.is_empty() {
        return None;
    }
    let mut ordered = vec![atoms.remove(0)];
    while !atoms.is_empty() {
        let bound: std::collections::BTreeSet<usize> = ordered
            .iter()
            .flat_map(|atom| atom.slots.iter().copied())
            .collect();
        let index = atoms
            .iter()
            .position(|atom| atom.slots.iter().any(|slot| bound.contains(slot)))?;
        ordered.push(atoms.remove(index));
    }
    Some(ordered)
}

fn key_columns(left_slots: &[usize], right_slots: &[usize]) -> Option<(Vec<usize>, Vec<usize>)> {
    let mut left_keys = Vec::new();
    let mut right_keys = Vec::new();
    for (right_col, slot) in right_slots.iter().enumerate() {
        if let Some(left_col) = left_slots.iter().position(|left| left == slot) {
            left_keys.push(left_col);
            right_keys.push(right_col);
        }
    }
    if left_keys.is_empty() {
        None
    } else {
        Some((left_keys, right_keys))
    }
}

fn term_table<'a>(
    support: &'a Support<'_>,
    predicate: PredicateRef<'_>,
    from_row: usize,
    until_row: Option<usize>,
    arity: usize,
    terms: &mut TermMap<'a>,
) -> Option<Atoms> {
    let mut columns = vec![Vec::new(); arity];
    for (index, row) in support.rows(predicate).enumerate() {
        if index < from_row {
            continue;
        }
        if until_row.is_some_and(|end| index >= end) {
            break;
        }
        for (column, values) in columns.iter_mut().enumerate() {
            let key = terms.insert(row.value(column)?)?;
            values.push(key);
        }
    }
    if columns
        .iter()
        .any(|column| column.len() != columns[0].len())
    {
        return None;
    }
    // The snapshot is not required to be sorted in identity order; adopt
    // it as an atom set of interned keys.
    let mut charges = 0u64;
    match Atoms::from_unsorted(&columns, &mut |n| {
        charges = charges.saturating_add(n);
        Ok::<(), ()>(())
    }) {
        Ok(Ok(atoms)) => Some(atoms),
        _ => None,
    }
}

fn bindings_to_atoms(bindings: &Bindings) -> Atoms {
    // A join result may repeat rows only if the inputs did; atom sets are
    // unique. from_unsorted sorts and uniques. We do not have a location
    // here for a charge refusal, so we use an infallible account: the
    // caller already charged the join that produced these rows.
    let columns: Vec<Vec<i32>> = (0..bindings.width())
        .map(|index| bindings.column(index).to_vec())
        .collect();
    match Atoms::from_unsorted(&columns, &mut |_| Ok::<(), ()>(())) {
        Ok(Ok(atoms)) => atoms,
        Ok(Err(_)) | Err(()) => unreachable!("join output columns have equal length"),
    }
}

fn project_slots(
    table: &Atoms,
    slots: &[usize],
    width: usize,
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<Bindings>, FormulaFailure> {
    if width == 0 {
        return Ok(Some(Bindings::empty(0)));
    }
    let mut columns = vec![Vec::with_capacity(table.len()); width];
    let mut map = vec![None; width];
    for (column, slot) in slots.iter().copied().enumerate() {
        if slot >= width {
            return Ok(None);
        }
        if map[slot].is_none() {
            map[slot] = Some(column);
        }
    }
    if map.iter().any(Option::is_none) {
        return Ok(None);
    }
    for row in 0..table.len() {
        account(1)?;
        for slot in 0..width {
            let column = map[slot].expect("checked");
            columns[slot].push(table.column(column)[row]);
        }
    }
    match Bindings::from_columns(columns) {
        Ok(bindings) => Ok(Some(bindings)),
        Err(_) => Ok(None),
    }
}

fn apply_residuals<'a>(
    bindings: &Bindings,
    plan: &Plan<'_>,
    support: &'a Support<'_>,
    terms: &mut TermMap<'a>,
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<Bindings>, FormulaFailure> {
    let mut rows = bindings.to_rows();
    let mut comparable = true;
    rows.retain(|row| {
        plan.compares.iter().all(|compare| {
            if let Some(holds) = residual(compare, row, terms) {
                holds
            } else {
                comparable = false;
                false
            }
        })
    });
    if !comparable {
        return Ok(None);
    }
    for negated in &plan.negated {
        let Some(table) = term_table(
            support,
            negated.predicate,
            0,
            None,
            negated.slots.len(),
            terms,
        ) else {
            return Ok(None);
        };
        rows.retain(|row| {
            let key: Vec<i32> = negated.slots.iter().map(|&slot| row[slot]).collect();
            !table.contains(&key)
        });
    }
    let mut columns = vec![Vec::with_capacity(rows.len()); plan.width];
    for row in &rows {
        account(1)?;
        for (column, &value) in columns.iter_mut().zip(row.iter()) {
            column.push(value);
        }
    }
    match Bindings::from_columns(columns) {
        Ok(bindings) => Ok(Some(bindings)),
        Err(_) => Ok(None),
    }
}

fn residual(compare: &Compare, row: &[i32], terms: &TermMap<'_>) -> Option<bool> {
    let left = row[compare.left];
    let right = row[compare.right];
    match compare.relation {
        Relation::Eq => Some(left == right),
        Relation::Neq => Some(left != right),
        Relation::Lt | Relation::Le | Relation::Gt | Relation::Ge => {
            // Catalog identity order is not term order.
            let ord = terms.get(left)?.cmp(&terms.get(right)?);
            Some(match compare.relation {
                Relation::Lt => ord.is_lt(),
                Relation::Le => ord.is_le(),
                Relation::Gt => ord.is_gt(),
                Relation::Ge => ord.is_ge(),
                Relation::Eq | Relation::Neq => unreachable!("ordered residual"),
            })
        }
    }
}

fn slot(expression: &Expression) -> Option<usize> {
    match expression.nodes.as_slice() {
        [Operation::Variable(slot)] => Some(*slot),
        _ => None,
    }
}

fn charge(
    counters: &mut Counters,
    limits: &FormulaLimits,
    location: ProgramSite,
    amount: u64,
) -> Result<(), FormulaFailure> {
    let mut remaining = amount;
    while remaining > 0 {
        counters.work(limits, location)?;
        remaining -= 1;
    }
    Ok(())
}
