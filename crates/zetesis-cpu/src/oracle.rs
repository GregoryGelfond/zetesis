//! Synchronous lazy source rounds with iterative, backtracking relational joins.

use std::collections::{BTreeMap, BTreeSet};

use zetesis_core::{
    Atom, AtomPattern, Filter, Model, ModelAtoms, Predicate, Program, Seed, SeedView, Template,
    Term, Value,
};

use crate::{Control, Stop};

mod window;
mod relations;
mod prepared;
pub use prepared::{ClosureWorkspace, PreparationLimits, PreparationStatistics, PreparedQueries};
use relations::{Catalogs, Relational};
pub(crate) mod restrictions;
pub mod source;
pub(crate) mod worlds;
#[cfg(test)]
#[path = "oracle/closure_tests.rs"]
mod closure_tests;
#[cfg(test)]
#[path = "oracle/work_tests.rs"]
mod work_tests;

/// Exact checking budgets, applied before the next charged operation/insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum charged template, tuple-probe, atom-key argument span, gate/filter,
    /// and output operations.
    /// Lazy joins also charge bound-prefix inspections and ordered comparisons,
    /// including both compared values' referenced payload bytes. Work counts can
    /// change with the execution algorithm; they are not ground-instance counts.
    /// A key charges its complete argument span even when an absent slot defers
    /// a gate. The bounded construction itself polls only at that boundary.
    pub max_work: u64,
    /// Maximum distinct derived atoms, including pending round outputs.
    pub max_derived_atoms: usize,
    /// Named capacity per scalar closure: predicate/catalog cells and names,
    /// tuple/index/column/prepared-order buffers, nested tuple payload, pending
    /// tuples and operation scratch/growth overlap. Shared structural buffers
    /// are counted per occurrence. Tree-container allocations (including vacant
    /// slots), allocator metadata and Arc-counter overhead,
    /// and final `Model` retention are excluded. Prepared scalar checks include
    /// query-owner headers and assignment/cursor/undo capacities.
    /// Actual allocator slack can exceed the proposed reservation before refusal;
    /// only completed checks publish their observed peak statistics.
    /// This is an independent finite allowance, not a process RSS ceiling.
    pub max_closure_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_work: 10_000_000,
            max_derived_atoms: 1_000_000,
            max_closure_bytes: 134_217_728,
        }
    }
}

/// Counters for a completed oracle invocation; these are execution work counts,
/// not a device performance estimate or a count of conceptual ground instances.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Completed source rounds, including the final no-delta round. The empty
    /// program needs no rounds because its source coverage is vacuous.
    pub rounds: u64,
    /// Charged operations, as described by [`Limits::max_work`].
    pub work: u64,
    /// Subset of work spent constructing, extending and probing retained typed
    /// catalogs. Source joins and final interpretation assembly are separate.
    pub catalog_work: u64,
    /// Largest admitted or actually reserved named scalar closure envelope,
    /// under [`Limits::max_closure_bytes`]. Refused unallocated proposals do not
    /// increase this maximum. It excludes final output ownership and is not RSS.
    pub peak_closure_bytes: usize,
    /// Fully matched enabled/filter-valid bindings visited across all rounds.
    pub bindings: u64,
    /// Distinct atoms in the final least consequence closure.
    pub derived_atoms: usize,
}

/// Exact closure and rejection reasons after a fully covered completion round.
/// A resource stop never constructs this value.
#[derive(Clone, Debug)]
pub struct Check {
    program: Program,
    closure: Model,
    constraint_violated: bool,
    seed_mismatch: bool,
    statistics: Statistics,
}

impl Check {
    /// The immutable instance whose reduct closure was checked. Constant time.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Exact least reduct closure, including rejected candidates. Constant-time
    /// borrow; the raw interpretation alone carries no stable-membership claim.
    #[must_use]
    pub const fn interpretation(&self) -> &zetesis_core::Interpretation {
        &self.closure
    }

    /// Transfer an accepted closure into an instance-bound stable receipt without
    /// rechecking membership, cloning atoms, or allocating.
    ///
    /// # Errors
    /// A rejected check is returned intact, retaining both rejection conditions.
    pub fn into_stable_interpretation(self) -> Result<crate::StableInterpretation, Self> {
        if self.accepted() {
            Ok(crate::StableInterpretation::new(self.program, self.closure))
        } else {
            Err(self)
        }
    }

    /// Whether the reconstructed closure is stable for the supplied seed.
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.constraint_violated && !self.seed_mismatch
    }
    /// The exact least closure of the frozen reduct, even for rejected seeds.
    #[must_use]
    pub fn closure(&self) -> &Model {
        &self.closure
    }
    /// An enabled constraint has a true positive body in the final closure.
    #[must_use]
    pub fn constraint_violated(&self) -> bool {
        self.constraint_violated
    }
    /// The closure's gate-carrier projection differs from the frozen seed.
    #[must_use]
    pub fn seed_mismatch(&self) -> bool {
        self.seed_mismatch
    }
    /// Completed invocation's counters.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }
}

pub(crate) struct Work<'a> {
    control: &'a Control,
    limits: Limits,
    statistics: Statistics,
    mask_words: u64,
    pruned_prefixes: u64,
    mask_bytes: usize,
}

impl Work<'_> {
    pub(crate) fn tick(&mut self) -> Result<(), Stop> {
        self.charge(1)
    }

    /// Charge bookkeeping units before one indivisible operation, without
    /// iterating over its payload. With unchanged control, a refusal records
    /// the same admitted prefix as repeated ticks. Control is polled before any
    /// nonzero charge; zero preserves the old empty-loop behavior and neither
    /// polls nor changes work. No real operation happens between these units.
    fn charge(&mut self, amount: usize) -> Result<(), Stop> {
        if amount == 0 {
            return Ok(());
        }
        self.control.poll()?;
        let remaining = self.limits.max_work.saturating_sub(self.statistics.work);
        if let Ok(amount) = u64::try_from(amount)
            && amount <= remaining
        {
            self.statistics.work += amount;
            Ok(())
        } else {
            self.statistics.work += remaining;
            Err(Stop::WorkLimit)
        }
    }

    pub(crate) fn source(control: &Control, max_work: u64) -> Work<'_> {
        Work {
            control,
            limits: Limits {
                max_work,
                max_derived_atoms: 0,
                max_closure_bytes: Limits::default().max_closure_bytes,
            },
            statistics: Statistics::default(),
            mask_words: 0,
            pruned_prefixes: 0,
            mask_bytes: 0,
        }
    }

    pub(crate) fn source_statistics(&self, bindings: u64) -> source::ScanStatistics {
        source::ScanStatistics {
            work: self.statistics.work,
            bindings,
            mask_words: self.mask_words,
            pruned_prefixes: self.pruned_prefixes,
            mask_bytes: self.mask_bytes,
        }
    }

    fn mask_word(&mut self) -> Result<(), Stop> {
        self.tick()?;
        self.mask_words += 1;
        Ok(())
    }
}

// The closure's Atom order groups signatures, then orders each relation by the
// tuple's Value storage order. Window lookup relies on this construction order;
// ASP term comparison is a different order and must not be used here.
type Relations<'a> = BTreeMap<&'a Predicate, Vec<&'a Atom>>;

/// Compute the exact least positive closure selected by a sparse frozen seed.
/// Joins read only current derived relations. A no-delta round checks every
/// template and constraint before the final gate-carrier seed comparison.
///
/// # Errors
/// Returns [`Stop`] for cancellation, deadlines, budgets, foreign seed identity,
/// or a violated admitted-program invariant; no partial result is accepted.
pub fn check(
    program: &Program,
    seed: &Seed,
    limits: Limits,
    control: &Control,
) -> Result<Check, Stop> {
    check_view(program, seed.view(), limits, control)
}

/// Check a borrowed owned seed or shared selection without copying its true
/// atoms. This is the same reduct computation, identity validation and operation
/// accounting as [`check`]; derived consequences still own their output payload.
///
/// # Errors
/// Returns the same typed stops as [`check`], without accepting partial results.
pub fn check_view(
    program: &Program,
    seed: SeedView<'_>,
    limits: Limits,
    control: &Control,
) -> Result<Check, Stop> {
    control.poll()?;
    if !program.same_instance(seed.program()) {
        return Err(Stop::WrongProgram);
    }
    if program.templates().is_empty() {
        return Ok(Check {
            program: program.clone(),
            closure: Model::default(),
            constraint_violated: false,
            seed_mismatch: false,
            statistics: Statistics::default(),
        });
    }
    let mut work = Work {
        control,
        limits,
        statistics: Statistics::default(),
        mask_words: 0,
        pruned_prefixes: 0,
        mask_bytes: 0,
    };
    let prepared = PreparedQueries::prepare(program, &mut work)?;
    prepared.check_with(seed, &mut ClosureWorkspace::default(), &mut work)

}

// Construction establishes a complete no-delta source scan, not constraint
// satisfaction or agreement with the seed. Those are separate acceptance facts.
struct CompletedClosure {
    atoms: Model,
    constraint_violated: bool,
}

// The caller has established program/seed identity. Each round reads only the
// preceding closure and visits every template, including constraints. Positive
// bodies and the fixed gate seed make derived atoms and constraint violations
// monotone. A no-delta round therefore establishes leastness and source coverage.
// Every pending insertion is charged and bounded before publication; a stop
// cannot construct CompletedClosure. Constraint failure never truncates a scan.
// Each growing round adds an atom, so the derived-atom ceiling bounds growth.
#[cfg(test)]
fn least_closure(
    program: &Program,
    seed: SeedView<'_>,
    work: &mut Work<'_>,
) -> Result<CompletedClosure, Stop> {
    let prepared = PreparedQueries::prepare(program, work)?;
    let check = prepared.check_with(seed, &mut ClosureWorkspace::default(), work)?;
    Ok(CompletedClosure { atoms: check.closure, constraint_violated: check.constraint_violated })
}

fn least_closure_with(
    program: &Program,
    seed: SeedView<'_>,
    closure: &mut Catalogs,
    buffers: &mut prepared::Buffers,
    dimensions: &prepared::Dimensions,
    overhead: u128,
    work: &mut Work<'_>,
) -> Result<CompletedClosure, Stop> {
    let mut constraint_violated = false;
    loop {
        work.tick()?;
        closure.prepare(work)?;
        let mut delta = BTreeSet::new();
        let mut pending_bytes = 0_u128;
        // Bindings borrow this round's immutable catalog extent only. The
        // reference-free cursor/undo buffers survive after these bindings drop.
        {
        let (mut assignment, bytes) = prepared::assignment(dimensions,
            closure.owned_bytes().checked_add(overhead).ok_or(Stop::StorageLimit)?, work)?;
        closure.set_overhead(overhead.checked_add(bytes).ok_or(Stop::StorageLimit)?, work)?;
        for template in program.templates() {
            work.tick()?;
            visit_with(
                template,
                &*closure,
                Some(seed),
                None,
                work,
                |assignment, work| {
                    work.tick()?;
                    if let Some(head) = template.head() {
                        work.charge(head.terms().len())?;
                        let key = head.key(assignment).map_err(|_| Stop::InvalidProgram)?;
                        if !closure.contains(&key, pending_bytes, work)?
                            && key.get(&delta).is_none()
                        {
                            if closure
                                .len()
                                .checked_add(delta.len())
                                .ok_or(Stop::DerivedAtomLimit)?
                                >= work.limits.max_derived_atoms
                            {
                                return Err(Stop::DerivedAtomLimit);
                            }
                            let (atom, bytes) = closure.pending(key, pending_bytes, work)?;
                            delta.insert(atom);
                            pending_bytes =
                                pending_bytes.checked_add(bytes).ok_or(Stop::StorageLimit)?;
                        }
                    } else {
                        constraint_violated = true;
                    }
                    Ok(())
                },
                Frame { assignment: &mut assignment, buffers: &mut *buffers },
            )?;
        }
        }
        closure.set_overhead(overhead, work)?;
        work.statistics.rounds += 1;
        if delta.is_empty() {
            break;
        }
        for atom in delta {
            let bytes = relations::atom_bytes(&atom, work)?;
            pending_bytes = pending_bytes
                .checked_sub(bytes)
                .ok_or(Stop::InvalidProgram)?;
            closure.insert(atom, pending_bytes, work)?;
        }
    }
    Ok(CompletedClosure {
        atoms: closure.take_model(work)?,
        constraint_violated,
    })
}

// Establish closure ∩ gate_carrier = seed in both directions. Continue charging
// both complete scans after a mismatch; rejection does not bypass work limits.
// Positive-only atoms do not belong to this comparison's projected carrier.
fn gate_agreement(
    program: &Program,
    seed: SeedView<'_>,
    closure: ModelAtoms<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let mut agreement = true;
    for atom in closure {
        work.tick()?;
        if program.contains_gate_atom(atom) && !seed.contains(atom) {
            agreement = false;
        }
    }
    for atom in seed.atoms() {
        work.tick()?;
        if !closure.contains(atom) {
            agreement = false;
        }
    }
    Ok(agreement)
}

struct Frame<'frame, 'source> {
    assignment: &'frame mut [Option<&'source Value>],
    buffers: &'frame mut prepared::Buffers,
}

fn visit<'source, E: From<Stop>>(
    template: &Template,
    relations: &'source impl Relational,
    seed: Option<SeedView<'_>>,
    membership: Option<&mut worlds::Join<'_>>,
    work: &mut Work<'_>,
    emit: impl FnMut(&[Option<&'source Value>], &mut Work<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let mut assignment = vec![None; template.variable_count()];
    let mut buffers = prepared::Buffers::local(template.positive().len());
    visit_with(template, relations, seed, membership, work, emit,
        Frame { assignment: &mut assignment, buffers: &mut buffers })
}

fn visit_with<'source, E: From<Stop>>(
    template: &Template,
    relations: &'source impl Relational,
    seed: Option<SeedView<'_>>,
    mut membership: Option<&mut worlds::Join<'_>>,
    work: &mut Work<'_>,
    mut emit: impl FnMut(&[Option<&'source Value>], &mut Work<'_>) -> Result<(), E>,
    frame: Frame<'_, 'source>,
) -> Result<(), E> {
    // Reset all query state before this source occurrence. No binding survives
    // template reuse; positive matching still returns original borrowed values.
    let assignment = &mut frame.assignment[..template.variable_count()];
    work.charge(assignment.len())?;
    assignment.fill(None);
    if !guards(template, assignment, seed, work)? {
        return Ok(());
    }
    let count = template.positive().len();
    if count == 0 {
        if !assignment.is_empty() {
            return Err(Stop::InvalidProgram.into());
        }
        work.statistics.bindings += 1;
        return emit(assignment, work);
    }
    if let Some(membership) = membership.as_mut() {
        membership.reset(template, work)?;
    }
    // None means this depth has not yet been opened for the current parent
    // assignment. A retained range advances in the original relation order.
    let cursors = &mut frame.buffers.cursors[..count];
    let undo = &mut frame.buffers.undo[..count];
    work.charge(count)?;
    cursors.fill(None);
    for row in undo.iter_mut() { work.tick()?; row.clear(); }
    let mut depth = 0;
    loop {
        work.tick()?;
        if depth == count {
            if assignment.iter().any(Option::is_none) {
                return Err(Stop::InvalidProgram.into());
            }
            work.statistics.bindings += 1;
            emit(assignment, work)?;
            depth -= 1;
            clear(assignment, &mut undo[depth]);
            continue;
        }
        let pattern = &template.positive()[depth];
        let tuples = relations.rows(pattern.predicate());
        let cursor = &mut cursors[depth];
        if cursor.is_none() {
            *cursor = Some(window::matching_prefix(pattern, tuples, assignment, work)?);
        }
        let Some(index) = cursor.as_mut().and_then(Iterator::next) else {
            *cursor = None;
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            clear(assignment, &mut undo[depth]);
            continue;
        };
        let atom = tuples.get(index).ok_or(Stop::InvalidProgram)?;
        if bind(pattern, atom, assignment, &mut undo[depth], work)?
            && guards(template, assignment, seed, work)?
            && match membership.as_mut() {
                Some(membership) => membership.extend(depth, index, work)?,
                None => true,
            }
        {
            depth += 1;
        } else {
            clear(assignment, &mut undo[depth]);
        }
    }
}

fn bind<'source>(
    pattern: &AtomPattern,
    atom: &'source Atom,
    assignment: &mut [Option<&'source Value>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for (term, value) in pattern.terms().iter().zip(atom.values()) {
        structural_work(value, work)?;
        match term {
            Term::Constant(expected) if expected != value => return Ok(false),
            Term::Variable(variable) => match assignment[*variable] {
                Some(expected) if expected != value => return Ok(false),
                Some(_) => {}
                None => {
                    assignment[*variable] = Some(value);
                    undo.push(*variable);
                }
            },
            Term::Constant(_) => {}
        }
    }
    Ok(true)
}

fn structural_work(value: &Value, work: &mut Work<'_>) -> Result<(), Stop> {
    if let Value::Structured(value) = value {
        work.charge(value.payload_bytes())?;
    }
    Ok(())
}

fn clear(assignment: &mut [Option<&Value>], undo: &mut Vec<usize>) {
    for variable in undo.drain(..) {
        assignment[variable] = None;
    }
}

fn resolve<'a>(term: &'a Term, assignment: &[Option<&'a Value>]) -> Option<&'a Value> {
    match term {
        Term::Constant(value) => Some(value),
        Term::Variable(variable) => assignment[*variable],
    }
}

fn guards(
    template: &Template,
    assignment: &[Option<&Value>],
    seed: Option<SeedView<'_>>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for filter in template.filters() {
        work.tick()?;
        let (left, right) = filter.terms();
        if let (Some(left), Some(right)) = (resolve(left, assignment), resolve(right, assignment)) {
            structural_work(left, work)?;
            structural_work(right, work)?;
            let equal = left == right;
            if matches!(filter, Filter::Eq(..)) != equal {
                return Ok(false);
            }
        }
    }
    let Some(seed) = seed else {
        return Ok(true);
    };
    for (patterns, required) in [(template.gate_true(), true), (template.gate_false(), false)] {
        for pattern in patterns {
            work.tick()?;
            work.charge(pattern.terms().len())?;
            // An absent referenced slot defers this gate until a later join
            // supplies it. A complete key borrows the exact seed lookup tuple.
            if let Ok(key) = pattern.key(assignment)
                && seed.contains_key(&key) != required
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
