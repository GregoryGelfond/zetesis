//! Synchronous scalar delta rounds and complete ordered source traversal.

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
use relations::{Catalogs, Relational, RowSet};
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
    /// delta-view preparation and output operations.
    /// Lazy joins also charge bound-prefix inspections and ordered comparisons,
    /// including both compared values' referenced payload bytes. Work counts can
    /// change with the execution algorithm; they are not ground-instance counts.
    /// A key charges its complete argument span even when an absent slot defers
    /// a gate. The bounded construction itself polls only at that boundary.
    pub max_work: u64,
    /// Maximum distinct derived atoms, including pending round outputs.
    pub max_derived_atoms: usize,
    /// Named capacity per scalar closure: predicate/catalog cells and names,
    /// tuple/index/column/prepared-order and old/new ID buffers, nested tuple payload, pending
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
    /// catalogs and their old/new ID views. Source joins and final interpretation
    /// assembly are separate.
    pub catalog_work: u64,
    /// Largest admitted or actually reserved named scalar closure envelope,
    /// under [`Limits::max_closure_bytes`]. Refused unallocated proposals do not
    /// increase this maximum. It excludes final output ownership and is not RSS.
    pub peak_closure_bytes: usize,
    /// Fully matched enabled/filter-valid bindings visited by the selected round
    /// schedule. Old bindings are not revisited by later scalar delta rounds.
    pub bindings: u64,
    /// Source rows offered to the whole-row matcher, including rejected rows.
    /// Excludes prefix-search comparisons and catalog membership lookups. Each
    /// probe belongs to an already charged join-loop step, so it is bounded by work.
    pub tuple_probes: u64,
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
/// Bootstrap checks every template against empty truth, including zero-positive
/// rules and constraints. Later rounds visit each binding at its first newly
/// derived source occurrence. Completed earlier scans account for old bindings;
/// the final no-change round completes source coverage before the full
/// gate-carrier seed comparison. Shared source traversal has its own full schedule.
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

// Construction establishes complete source coverage through bootstrap and
// completed round history, not constraint satisfaction or seed agreement.
struct CompletedClosure {
    atoms: Model,
    constraint_violated: bool,
}

// Positive bodies, pure equality filters and frozen gates make old enabled
// bindings persist. Complete bootstrap plus disjoint first-new scans therefore
// cover the same inflationary step as full rescanning. Constraints latch only
// after a complete selected family; they never truncate another occurrence.
// Every growing round adds an atom and has admitted its pending publication.
// No resource stop can produce CompletedClosure or publish a reusable frontier.
#[cfg(test)]
fn least_closure(
    program: &Program,
    seed: SeedView<'_>,
    work: &mut Work<'_>,
) -> Result<CompletedClosure, Stop> {
    let prepared = PreparedQueries::prepare(program, work)?;
    prepared.closure_with(seed, &mut ClosureWorkspace::default(), Schedule::Delta, work)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Schedule {
    Delta,
    #[cfg(test)]
    Full,
}

#[derive(Clone, Copy)]
enum Selection { All, FirstNew(usize) }

impl Selection {
    fn rows(self, occurrence: usize) -> RowSet {
        match self {
            Self::All => RowSet::Current,
            Self::FirstNew(pivot) if occurrence < pivot => RowSet::Old,
            Self::FirstNew(pivot) if occurrence == pivot => RowSet::New,
            Self::FirstNew(_) => RowSet::Current,
        }
    }
}

struct RoundWorkspace<'a> {
    buffers: &'a mut prepared::Buffers,
    dimensions: &'a prepared::Dimensions,
    overhead: u128,
}

fn least_closure_with(
    program: &Program,
    seed: SeedView<'_>,
    closure: &mut Catalogs,
    workspace: RoundWorkspace<'_>,
    schedule: Schedule,
    work: &mut Work<'_>,
) -> Result<CompletedClosure, Stop> {
    let RoundWorkspace { buffers, dimensions, overhead } = workspace;
    let mut constraint_violated = false;
    loop {
        work.tick()?;
        let incremental = schedule == Schedule::Delta && work.statistics.rounds != 0;
        if incremental { closure.prepare_delta(work)?; }
        else { closure.prepare(work)?; }
        // Bindings borrow this round's immutable catalog extent only. The
        // reference-free cursor/undo buffers survive after these bindings drop.
        let consequences = {
            let (mut assignment, bytes) = prepared::assignment(
                dimensions,
                closure.owned_bytes().checked_add(overhead).ok_or(Stop::StorageLimit)?,
                work,
            )?;
            closure.set_overhead(overhead.checked_add(bytes).ok_or(Stop::StorageLimit)?, work)?;
            visit_round(program, seed, closure, incremental,
                Frame { assignment: &mut assignment, buffers: &mut *buffers, selection: Selection::All },
                work)?
        };
        let RoundConsequences { atoms: delta, bytes: mut pending_bytes, constraint_violated: triggered } = consequences;
        constraint_violated |= triggered;
        closure.set_overhead(overhead, work)?;
        work.statistics.rounds += 1;
        if delta.is_empty() {
            break;
        }
        if schedule == Schedule::Delta { closure.advance(work)?; }
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

struct RoundConsequences {
    atoms: BTreeSet<Atom>,
    bytes: u128,
    constraint_violated: bool,
}

// Complete the disjoint source family before publishing either its history or
// its consequences. A constraint latch never skips another source occurrence.
fn visit_round<'source>(
    program: &Program,
    seed: SeedView<'_>,
    closure: &'source Catalogs,
    incremental: bool,
    frame: Frame<'_, 'source>,
    work: &mut Work<'_>,
) -> Result<RoundConsequences, Stop> {
    let Frame { assignment, buffers, .. } = frame;
    let mut result = RoundConsequences {
        atoms: BTreeSet::new(), bytes: 0, constraint_violated: false,
    };
    for template in program.templates() {
        work.tick()?;
        let mut emit = |assignment: &[Option<&Value>], work: &mut Work<'_>| -> Result<(), Stop> {
            work.tick()?;
            if let Some(head) = template.head() {
                work.charge(head.terms().len())?;
                let key = head.key(assignment).map_err(|_| Stop::InvalidProgram)?;
                if !closure.contains(&key, result.bytes, work)? && key.get(&result.atoms).is_none() {
                    if closure.len().checked_add(result.atoms.len()).ok_or(Stop::DerivedAtomLimit)?
                        >= work.limits.max_derived_atoms
                    { return Err(Stop::DerivedAtomLimit); }
                    let (atom, bytes) = closure.pending(key, result.bytes, work)?;
                    result.atoms.insert(atom);
                    result.bytes = result.bytes.checked_add(bytes).ok_or(Stop::StorageLimit)?;
                }
            } else {
                result.constraint_violated = true;
            }
            Ok(())
        };
        if incremental {
            // Repeated predicates retain distinct occurrences. Earlier Old,
            // this New and later Current rows select the unique first new row.
            for (pivot, pattern) in template.positive().iter().enumerate() {
                if closure.has_new(pattern.predicate(), work)? {
                    visit_with(template, closure, Some(seed), None, work, &mut emit,
                        Frame { assignment: &mut *assignment, buffers: &mut *buffers,
                            selection: Selection::FirstNew(pivot) })?;
                }
            }
        } else {
            // Bootstrap includes every zero-positive head and constraint under
            // the frozen gates. The test reference repeats this complete scan.
            visit_with(template, closure, Some(seed), None, work, &mut emit,
                Frame { assignment: &mut *assignment, buffers: &mut *buffers,
                    selection: Selection::All })?;
        }
    }
    Ok(result)
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
    selection: Selection,
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
    visit_with(
        template,
        relations,
        seed,
        membership,
        work,
        emit,
        Frame {
            assignment: &mut assignment,
            buffers: &mut buffers,
            selection: Selection::All,
        },
    )
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
    let Frame {
        assignment,
        buffers,
        selection,
    } = frame;
    let assignment = &mut assignment[..template.variable_count()];
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
    let cursors = &mut buffers.cursors[..count];
    let undo = &mut buffers.undo[..count];
    work.charge(count)?;
    cursors.fill(None);
    for row in undo.iter_mut() {
        work.tick()?;
        row.clear();
    }
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
        let tuples = relations.selected(pattern.predicate(), selection.rows(depth))?;
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
        // This iteration already charged one join step and offers at most one
        // row. The cumulative probe count therefore cannot exceed charged work.
        work.statistics.tuple_probes += 1;
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
