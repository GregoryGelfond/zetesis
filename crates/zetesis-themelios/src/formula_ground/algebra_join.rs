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
//! current snapshot. Join methods still agree on a lex-TermId binding
//! table. Heads are emitted from pre-projected term columns, in
//! `TermRef` semantic order, so the catalog stays in a few ordered
//! stretches for model-construction `checked_sort`. Emit still charges
//! `Binding::set_term_with` as the Join cursor does when it binds a
//! support atom.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use themelios_program::program::{DefaultNegation, Relation};
use zetesis_algebra::{Atoms, Bindings, JoinMethod, JoinStats, join};
use zetesis_core::TemplateTerm;
use zetesis_core::catalog::{CatalogRead, PredicateRef, TermKey, TermRef};

use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_support::{Computation, Counters, Support};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Realized as a sorted merge. Hash join was measured earlier and is not
/// selected here; both methods agree on a lex-TermId binding table.
fn method() -> JoinMethod {
    JoinMethod::SortedMerge
}

/// Persistent interned snapshot columns and transferable term keys for one
/// support-completion run. See `grounding-joins/PRIOR-ART.md`: Soufflé, `FVLog`,
/// and the Vaelii redesign all keep ordinals with the relation and intern only
/// new rows. `TermRef` cannot be stored here, because `Support` is recreated
/// each round; `TermKey` is reborrowed through the current `CatalogRead`.
struct InternCache {
    keys: HashMap<i32, TermKey>,
    bodies: HashMap<usize, CachedBody>,
    /// Keys in `TermRef` order, grown by merging keys first seen at emit.
    sorted_keys: Vec<i32>,
    /// Dense ranks of `sorted_keys`. Relative order is `TermRef` order, so
    /// ranks over a superset of one visit's output keys still sort that
    /// visit correctly.
    rank: HashMap<i32, u32>,
}

struct CachedBody {
    rows: usize,
    columns: Vec<Vec<i32>>,
}

impl InternCache {
    fn new() -> Self {
        Self {
            keys: HashMap::new(),
            bodies: HashMap::new(),
            sorted_keys: Vec::new(),
            rank: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.keys.clear();
        self.bodies.clear();
        self.sorted_keys.clear();
        self.rank.clear();
    }
}

thread_local! {
    static INTERN: RefCell<InternCache> = RefCell::new(InternCache::new());
}

/// Clear the support-run intern cache. Installed around
/// `complete_with_executor` so a later program in the same thread cannot
/// reborrow a foreign `TermKey`.
pub(crate) fn reset_intern_cache() {
    INTERN.with(|cell| cell.borrow_mut().clear());
}

/// Reset on construction and on drop so both early returns and panics leave
/// a clean cache.
pub(crate) struct InternCacheGuard;

impl InternCacheGuard {
    pub(crate) fn new() -> Self {
        reset_intern_cache();
        Self
    }
}

impl Drop for InternCacheGuard {
    fn drop(&mut self) {
        reset_intern_cache();
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

    /// Row indices of the binding table in semantic head order.
    ///
    /// The algebra Join sorts by interned identity, which is not term
    /// order. Inserting heads in `TermRef` comparison order keeps each
    /// recursive stretch nearly sorted, so `ModelOrder::prepare_with`
    /// does far fewer checked comparisons. Ties break by the original
    /// row index so the order is deterministic at every thread count.
    ///
    /// Ranks are maintained incrementally in the support-run intern cache
    /// (only new keys are TermRef-sorted and merged). The decorated row
    /// keys are then radix-sorted as integers.
    pub(crate) fn emission_order(&self, rule: &RuleIr, support: &Support<'_>) -> Vec<usize> {
        let mut rows: Vec<usize> = self.bindings.row_range().collect();
        let HeadIr::Normal(Some(head)) = &rule.head else {
            return rows;
        };
        let Some(view) = support.components() else {
            return rows;
        };
        let Some(pattern) = head.resolve(view) else {
            return rows;
        };
        let width = self.bindings.width();
        let mut slots = Vec::with_capacity(pattern.predicate().arity());
        for index in 0..pattern.predicate().arity() {
            match pattern.terms().at(index) {
                Some(TemplateTerm::Variable(slot)) if slot < width => slots.push(Some(slot)),
                Some(TemplateTerm::Constant(_)) => slots.push(None),
                _ => return rows,
            }
        }
        // Rank interned keys once by TermRef order, then sort rows by
        // integer ranks. Repeated structural compares were dominating
        // support time, especially on sg at many threads.
        let columns: Vec<Option<&[i32]>> = slots
            .iter()
            .map(|slot| slot.map(|slot| self.bindings.column(slot)))
            .collect();
        let ranks = self
            .terms
            .ranks_incremental(columns.iter().copied().flatten(), rows.len());
        // Decorate once, then integer-sort. HashMap lookups inside the
        // comparator were dominating support time (closure ~equal to
        // evaluate; sg even more) because each comparison re-probed ranks.
        let arity = columns.iter().filter(|column| column.is_some()).count();
        if arity > MAX_HEAD {
            rows.sort_by(|&left, &right| {
                for column in &columns {
                    let Some(column) = column else {
                        continue;
                    };
                    let left_rank = ranks
                        .get(&column[left])
                        .expect("join keys are interned from the snapshot");
                    let right_rank = ranks
                        .get(&column[right])
                        .expect("join keys are interned from the snapshot");
                    match left_rank.cmp(right_rank) {
                        std::cmp::Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                left.cmp(&right)
            });
            return rows;
        }
        let mut decorated: Vec<([u32; MAX_HEAD], usize)> = Vec::with_capacity(rows.len());
        for row in rows {
            let mut key = [0u32; MAX_HEAD];
            let mut written = 0;
            for column in &columns {
                let Some(column) = column else {
                    continue;
                };
                key[written] = *ranks
                    .get(&column[row])
                    .expect("join keys are interned from the snapshot");
                written += 1;
            }
            decorated.push((key, row));
        }
        radix_sort_decorated(decorated, arity)
    }
}

/// Evaluate `rule` to columnar bindings, or `Ok(None)` when declined.
pub(crate) fn evaluate<'a>(
    rule: &'a RuleIr,
    support: &'a Support<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
    allow_negation: bool,
    pending: &[usize],
) -> Result<Option<Evaluated<'a>>, FormulaFailure> {
    let Some(plan) = plan(rule, support) else {
        return Ok(None);
    };
    // `allow_negation` is computed by the hook from decided() plus the
    // defining component's reached (N1) or settled (N2) stage. Without
    // that gate, an empty snapshot is not evidence of completeness.
    if !plan.negated.is_empty() && !allow_negation {
        return Ok(None);
    }
    let mut account = |amount: u64| charge(counters, limits, location, amount);
    let mut terms = TermMap::for_support(support);
    let recursive = recursive_atoms(rule, support, &plan.atoms);
    let Some(projected) = semi_naive(&plan, &recursive, support, &mut terms, &mut account)? else {
        return Ok(None);
    };
    let Some(filtered) = apply_residuals(
        &projected,
        &plan,
        support,
        &mut terms,
        pending,
        &mut account,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Evaluated {
        bindings: filtered,
        terms,
    }))
}

/// Snapshot-order interned columns plus semi-naive bounds, all `Sync` so
/// workers can join a delta slice without touching `Support`.
pub(crate) struct JoinTables {
    width: usize,
    atoms: Vec<Vec<usize>>,
    raw: Vec<Vec<Vec<i32>>>,
    recursive: Vec<usize>,
    old_rows: Vec<usize>,
}

/// One semi-naive part: the new-row suffix of one recursive atom occurrence.
#[derive(Clone, Copy)]
pub(crate) struct DeltaPart {
    /// Index into the recursive-occurrence list, used to pin earlier
    /// occurrences to the old prefix.
    pub index: usize,
    /// Index into the planned atom list / raw tables.
    pub atom: usize,
    pub from: usize,
    pub until: usize,
}

impl DeltaPart {
    pub(crate) fn len(self) -> usize {
        self.until.saturating_sub(self.from)
    }
}

/// Fixed body tables for one semi-naive part, built once on the coordinator
/// so workers do not re-sort the non-delta relations 14 ways.
pub(crate) struct PrebuiltPart {
    part: DeltaPart,
    fixed: Vec<Option<Atoms>>,
}

/// Coordinator-loaded join: `Sync` tables for workers, `TermMap` for emit.
pub(crate) struct PreparedJoin<'a> {
    pub tables: JoinTables,
    plan: Plan<'a>,
    terms: TermMap<'a>,
    /// Negated body occurrences whose anti-join must include this
    /// round's pending atoms (`contains_pattern`); skipped in the
    /// snapshot residual and filtered after bind.
    pending: Vec<usize>,
}

impl JoinTables {
    pub(crate) fn delta_parts(&self) -> Vec<DeltaPart> {
        let mut parts = Vec::new();
        for (index, &atom) in self.recursive.iter().enumerate() {
            let old = self.old_rows[atom];
            let count = self.raw[atom].first().map_or(0, Vec::len);
            if old >= count {
                continue;
            }
            parts.push(DeltaPart {
                index,
                atom,
                from: old,
                until: count,
            });
        }
        parts
    }

    fn bounds_for(
        &self,
        part: DeltaPart,
        from: usize,
        until: usize,
    ) -> Vec<(usize, Option<usize>)> {
        let mut bounds = vec![(0, None); self.atoms.len()];
        for &previous in self.recursive.iter().take(part.index) {
            bounds[previous] = (0, Some(self.old_rows[previous]));
        }
        bounds[part.atom] = (from, Some(until));
        bounds
    }

    /// Sort every non-delta body of `part` once, on the coordinator.
    pub(crate) fn prebuild(
        &self,
        part: DeltaPart,
        account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
    ) -> Result<Option<PrebuiltPart>, FormulaFailure> {
        let bounds = self.bounds_for(part, part.from, part.until);
        let mut fixed = Vec::with_capacity(self.atoms.len());
        for (index, &(from_row, until_row)) in bounds.iter().enumerate() {
            if index == part.atom {
                fixed.push(None);
                continue;
            }
            let sliced = slice_raw(&self.raw[index], from_row, until_row);
            match Atoms::from_unsorted(&sliced, account)? {
                Ok(atoms) => fixed.push(Some(atoms)),
                Err(_) => return Ok(None),
            }
        }
        Ok(Some(PrebuiltPart { part, fixed }))
    }

    pub(crate) fn join_range_prebuilt(
        &self,
        prebuilt: &PrebuiltPart,
        from: usize,
        until: usize,
        account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
    ) -> Result<Option<Bindings>, FormulaFailure> {
        if until <= from {
            return Ok(Some(Bindings::empty(self.width)));
        }
        let bounds = self.bounds_for(prebuilt.part, from, until);
        let Some((table, slots)) =
            join_atoms_prebuilt(&self.atoms, &self.raw, &prebuilt.fixed, &bounds, account)?
        else {
            return Ok(None);
        };
        project_slots(&table, &slots, self.width, account)
    }
}

/// Load interned snapshot columns on the coordinator. Declines the same
/// bodies as [`evaluate`].
pub(crate) fn prepare<'a>(
    rule: &'a RuleIr,
    support: &'a Support<'_>,
    allow_negation: bool,
    pending: &[usize],
) -> Option<PreparedJoin<'a>> {
    let plan = plan(rule, support)?;
    if !plan.negated.is_empty() && !allow_negation {
        return None;
    }
    let mut terms = TermMap::for_support(support);
    let mut raw = Vec::with_capacity(plan.atoms.len());
    let mut old_rows = Vec::with_capacity(plan.atoms.len());
    for atom in &plan.atoms {
        raw.push(load_raw(support, atom.predicate, atom.arity, &mut terms)?);
        old_rows.push(support.old_rows(atom.predicate));
    }
    let recursive = recursive_atoms(rule, support, &plan.atoms);
    let tables = JoinTables {
        width: plan.width,
        atoms: plan.atoms.iter().map(|atom| atom.slots.clone()).collect(),
        raw,
        recursive,
        old_rows,
    };
    Some(PreparedJoin {
        tables,
        plan,
        terms,
        pending: pending.to_vec(),
    })
}

/// Concatenate worker (or part) bindings, apply residuals, and hand back
/// an [`Evaluated`] table for TermRef-ordered emission.
pub(crate) fn finish<'a>(
    prepared: PreparedJoin<'a>,
    parts: &[Bindings],
    support: &'a Support<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Option<Evaluated<'a>>, FormulaFailure> {
    let mut account = |amount: u64| charge(counters, limits, location, amount);
    let width = prepared.plan.width;
    let Some(projected) = concat_bindings(width, parts, &mut account)? else {
        return Ok(None);
    };
    let PreparedJoin {
        plan,
        mut terms,
        pending,
        ..
    } = prepared;
    let Some(filtered) = apply_residuals(
        &projected,
        &plan,
        support,
        &mut terms,
        &pending,
        &mut account,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Evaluated {
        bindings: filtered,
        terms,
    }))
}

fn concat_bindings(
    width: usize,
    parts: &[Bindings],
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<Bindings>, FormulaFailure> {
    if width == 0 {
        return Ok(Some(Bindings::empty(0)));
    }
    if parts.is_empty() {
        return Ok(Some(Bindings::empty(width)));
    }
    let mut rows = Vec::new();
    for part in parts {
        rows.extend(part.to_rows());
    }
    let mut columns = vec![Vec::with_capacity(rows.len()); width];
    for row in &rows {
        account(1)?;
        if row.len() != width {
            return Ok(None);
        }
        for (column, &value) in columns.iter_mut().zip(row.iter()) {
            column.push(value);
        }
    }
    match Bindings::from_columns(columns) {
        Ok(bindings) => Ok(Some(bindings)),
        Err(_) => Ok(None),
    }
}

fn load_raw<'a>(
    support: &'a Support<'_>,
    predicate: PredicateRef<'_>,
    arity: usize,
    terms: &mut TermMap<'a>,
) -> Option<Vec<Vec<i32>>> {
    let count = support.row_count(predicate);
    let position = predicate.canonical_position();
    let (from, mut columns) = position
        .and_then(|position| {
            INTERN.with(|cell| {
                let intern = cell.borrow();
                intern.bodies.get(&position).and_then(|cached| {
                    if cached.rows <= count && cached.columns.len() == arity {
                        Some((cached.rows, cached.columns.clone()))
                    } else {
                        None
                    }
                })
            })
        })
        .unwrap_or_else(|| (0, vec![Vec::with_capacity(count); arity]));
    if from == 0 && columns.iter().any(|column| !column.is_empty()) {
        columns = vec![Vec::with_capacity(count); arity];
    }
    for row in support.rows_from(predicate, from) {
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
    if let Some(position) = position {
        INTERN.with(|cell| {
            cell.borrow_mut().bodies.insert(
                position,
                CachedBody {
                    rows: columns.first().map_or(0, Vec::len),
                    columns: columns.clone(),
                },
            );
        });
    }
    Some(columns)
}

fn slice_raw(columns: &[Vec<i32>], from: usize, until: Option<usize>) -> Vec<Vec<i32>> {
    let len = columns.first().map_or(0, Vec::len);
    let until = until.unwrap_or(len).min(len);
    let from = from.min(until);
    columns
        .iter()
        .map(|column| column[from..until].to_vec())
        .collect()
}

fn load_delta(
    raw: &[Vec<i32>],
    from_row: usize,
    until_row: Option<usize>,
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<Atoms>, FormulaFailure> {
    let sliced = slice_raw(raw, from_row, until_row);
    match Atoms::from_unsorted(&sliced, account)? {
        Ok(atoms) => Ok(Some(atoms)),
        Err(_) => Ok(None),
    }
}

fn join_atoms_prebuilt(
    slots: &[Vec<usize>],
    raw: &[Vec<Vec<i32>>],
    fixed: &[Option<Atoms>],
    bounds: &[(usize, Option<usize>)],
    account: &mut impl FnMut(u64) -> Result<(), FormulaFailure>,
) -> Result<Option<(Atoms, Vec<usize>)>, FormulaFailure> {
    if bounds.is_empty() {
        return Ok(None);
    }
    let mut delta: Option<Atoms> = None;
    if fixed[0].is_none() {
        let Some(table) = load_delta(&raw[0], bounds[0].0, bounds[0].1, account)? else {
            return Ok(None);
        };
        delta = Some(table);
    }
    if bounds.len() == 1 {
        let first = match (&fixed[0], &delta) {
            (Some(table), _) | (_, Some(table)) => table.clone(),
            _ => return Ok(None),
        };
        return Ok(Some((first, slots[0].clone())));
    }
    // Left starts as a borrow of the first body (shared or this worker's
    // delta). The first join output is owned; later steps join that.
    let mut current: Option<(Atoms, Vec<usize>)> = None;
    for index in 1..bounds.len() {
        let (from_row, until_row) = bounds[index];
        let mut right_delta: Option<Atoms> = None;
        if fixed[index].is_none() {
            let Some(table) = load_delta(&raw[index], from_row, until_row, account)? else {
                return Ok(None);
            };
            right_delta = Some(table);
        }
        let right: &Atoms = match (&fixed[index], &right_delta) {
            (Some(table), _) | (_, Some(table)) => table,
            _ => return Ok(None),
        };
        let (left, left_slots): (&Atoms, &[usize]) = if let Some((table, left_slots)) = &current {
            (table, left_slots)
        } else {
            let first: &Atoms = match (&fixed[0], &delta) {
                (Some(table), _) | (_, Some(table)) => table,
                _ => return Ok(None),
            };
            (first, slots[0].as_slice())
        };
        let Some((left_keys, right_keys)) = key_columns(left_slots, &slots[index]) else {
            return Ok(None);
        };
        let mut stats = JoinStats::default();
        let Ok(joined) = join(
            left,
            right,
            &left_keys,
            &right_keys,
            method(),
            account,
            &mut stats,
        )?
        else {
            return Ok(None);
        };
        let combined = [left_slots, slots[index].as_slice()].concat();
        current = Some((bindings_to_atoms(&joined), combined));
    }
    Ok(current)
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
///
/// `resolved` holds `TermRef`s for this visit only. Keys that were interned
/// in an earlier visit of the same support run are reborrowed lazily through
/// `CatalogRead::term` from the transferable `TermKey` cache, so prepare does
/// not reconstruct payloads for the whole snapshot.
struct TermMap<'a> {
    witness: Option<TermRef<'a>>,
    read: Option<CatalogRead<'a>>,
    resolved: RefCell<HashMap<i32, TermRef<'a>>>,
}

impl<'a> TermMap<'a> {
    fn for_support(support: &'a Support<'_>) -> Self {
        Self {
            witness: None,
            read: support.catalog_read(),
            resolved: RefCell::new(HashMap::new()),
        }
    }

    fn insert(&mut self, term: TermRef<'a>) -> Option<i32> {
        let key = term_key(term)?;
        if let Some(witness) = self.witness {
            if witness.same_vocabulary(term) != Some(true) {
                return None;
            }
        } else {
            self.witness = Some(term);
        }
        self.resolved.get_mut().entry(key).or_insert(term);
        if let Some(read) = self.read {
            if let Ok(tk) = read.term_key(term) {
                INTERN.with(|cell| {
                    cell.borrow_mut().keys.insert(key, tk);
                });
            } else {
                // A foreign key means the cache still holds a previous
                // program's vocabulary. Drop it; this visit keeps the
                // TermRef in `resolved` and later visits start clean.
                reset_intern_cache();
                if let Ok(tk) = read.term_key(term) {
                    INTERN.with(|cell| {
                        cell.borrow_mut().keys.insert(key, tk);
                    });
                }
            }
        }
        Some(key)
    }

    fn get(&self, key: i32) -> Option<TermRef<'a>> {
        if let Some(term) = self.resolved.borrow().get(&key).copied() {
            return Some(term);
        }
        let term = self.reborrow(key)?;
        self.resolved.borrow_mut().insert(key, term);
        Some(term)
    }

    fn reborrow(&self, key: i32) -> Option<TermRef<'a>> {
        let read = self.read?;
        INTERN.with(|cell| {
            let intern = cell.borrow();
            let key = intern.keys.get(&key)?;
            read.term(key).ok()
        })
    }

    /// Dense semantic ranks for the keys that appear in `columns`.
    ///
    /// Only keys not already in the support-run intern cache are
    /// TermRef-sorted; they are merged into the cached order.
    fn ranks_incremental<'c>(
        &self,
        columns: impl Iterator<Item = &'c [i32]>,
        rows: usize,
    ) -> HashMap<i32, u32> {
        let mut needed: Vec<i32> = Vec::new();
        let mut seen: HashSet<i32> = HashSet::new();
        for column in columns {
            let end = rows.min(column.len());
            for &key in &column[..end] {
                if seen.insert(key) {
                    needed.push(key);
                }
            }
        }
        let (sorted_keys, rank) = INTERN.with(|cell| {
            let intern = cell.borrow();
            (intern.sorted_keys.clone(), intern.rank.clone())
        });
        let mut fresh: Vec<i32> = needed
            .iter()
            .copied()
            .filter(|key| !rank.contains_key(key))
            .collect();
        if fresh.is_empty() {
            return rank;
        }
        fresh.sort_by(|left, right| self.get(*left).cmp(&self.get(*right)).then(left.cmp(right)));
        let mut merged = Vec::with_capacity(sorted_keys.len() + fresh.len());
        let mut old = sorted_keys.iter().copied().peekable();
        let mut new = fresh.iter().copied().peekable();
        while let (Some(&left), Some(&right)) = (old.peek(), new.peek()) {
            if self.get(left).cmp(&self.get(right)).then(left.cmp(&right))
                == std::cmp::Ordering::Greater
            {
                merged.push(right);
                new.next();
            } else {
                merged.push(left);
                old.next();
            }
        }
        merged.extend(old);
        merged.extend(new);
        let mut new_rank = HashMap::with_capacity(merged.len());
        for (index, key) in merged.iter().copied().enumerate() {
            new_rank.insert(
                key,
                u32::try_from(index).expect("intern-cache rank fits in u32"),
            );
        }
        INTERN.with(|cell| {
            let mut intern = cell.borrow_mut();
            intern.sorted_keys = merged;
            intern.rank.clone_from(&new_rank);
        });
        new_rank
    }
}

/// Bit-preserving identity of a canonical catalog term. Ids above
/// `i32::MAX` stay distinct so a merge join still groups equal keys.
fn term_key(term: TermRef<'_>) -> Option<i32> {
    let position = term.canonical_position()?;
    let word = u32::try_from(position).ok()?;
    Some(i32::from_ne_bytes(word.to_ne_bytes()))
}

const MAX_HEAD: usize = 8;

/// LSD radix sort of decorated head-rank keys, then the original row index,
/// so the order matches `sort_unstable` on `([u32; MAX_HEAD], usize)`.
fn radix_sort_decorated(mut decorated: Vec<([u32; MAX_HEAD], usize)>, arity: usize) -> Vec<usize> {
    let len = decorated.len();
    if len <= 1 {
        return decorated.into_iter().map(|(_, row)| row).collect();
    }
    let mut scratch = vec![([0u32; MAX_HEAD], 0usize); len];
    // Row index first (least significant), then rank lanes from last to first.
    counting_sort_usize(&mut decorated, &mut scratch, |_, row| row);
    for lane in (0..arity).rev() {
        counting_sort_u32(&mut decorated, &mut scratch, move |key, _| key[lane]);
    }
    decorated.into_iter().map(|(_, row)| row).collect()
}

fn counting_sort_u32(
    src: &mut [([u32; MAX_HEAD], usize)],
    dst: &mut [([u32; MAX_HEAD], usize)],
    digit: impl Fn(&[u32; MAX_HEAD], usize) -> u32,
) {
    for shift in [0u32, 8, 16, 24] {
        let mut counts = [0usize; 256];
        for (key, row) in src.iter() {
            counts[((digit(key, *row) >> shift) & 0xff) as usize] += 1;
        }
        let mut sum = 0;
        for count in &mut counts {
            let next = sum + *count;
            *count = sum;
            sum = next;
        }
        for entry in src.iter() {
            let bucket = ((digit(&entry.0, entry.1) >> shift) & 0xff) as usize;
            dst[counts[bucket]] = *entry;
            counts[bucket] += 1;
        }
        src.swap_with_slice(dst);
    }
}

fn counting_sort_usize(
    src: &mut [([u32; MAX_HEAD], usize)],
    dst: &mut [([u32; MAX_HEAD], usize)],
    digit: impl Fn(&[u32; MAX_HEAD], usize) -> usize,
) {
    let passes = usize::BITS / 8;
    for pass in 0..passes {
        let shift = pass * 8;
        let mut counts = [0usize; 256];
        for (key, row) in src.iter() {
            counts[(digit(key, *row) >> shift) & 0xff] += 1;
        }
        let mut sum = 0;
        for count in &mut counts {
            let next = sum + *count;
            *count = sum;
            sum = next;
        }
        for entry in src.iter() {
            let bucket = (digit(&entry.0, entry.1) >> shift) & 0xff;
            dst[counts[bucket]] = *entry;
            counts[bucket] += 1;
        }
        src.swap_with_slice(dst);
    }
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
    occurrence: usize,
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
    for (occurrence, literal) in rule.body.iter().enumerate() {
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
                    occurrence,
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
    let raw = load_raw(support, predicate, arity, terms)?;
    let columns = slice_raw(&raw, from_row, until_row);
    if columns
        .iter()
        .any(|column| column.len() != columns.first().map_or(0, Vec::len))
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
    pending: &[usize],
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
        if pending.contains(&negated.occurrence) {
            // Settled, not reached: snapshot is incomplete. The hook
            // anti-joins against snapshot ∪ pending after bind.
            continue;
        }
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

pub(crate) fn charge(
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
