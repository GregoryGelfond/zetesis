//! Synchronous scalar delta rounds and complete ordered source traversal.

use std::collections::BTreeSet;

use zetesis_core::{
    Atom, AtomKey, AtomPattern, Filter, Model, ModelAtoms, Program, Seed, SeedView, Template, Term,
    Value,
};

use crate::{Control, Stop};

mod window;
mod relations;
mod prepared;
pub mod bounds;
pub use prepared::{ClosureWorkspace, PreparationLimits, PreparationStatistics, PreparedQueries};
use relations::{
    Block, Catalogs, Dense, Layouts, PendingRows, Relational, Relations, Row, RowSet, Rows, Slot,
};
pub(crate) mod restrictions;
mod row_steps;
use row_steps::RowSteps;
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
    /// the preparation's retained bytes and assignment/cursor/undo capacities.
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
    /// Derived heads recorded as a bit of a dense relation's pending rows, so
    /// that no atom was built for them before the model was assembled. The
    /// closure's other heads were built as atoms when first derived.
    pub dense_heads: u64,
    /// Blocks of body rows joined into a head's pending rows a word at a
    /// time, each in place of binding its rows one by one. The rows of such a
    /// block are counted in `bindings` and not in `tuple_probes`: none is
    /// offered to the row matcher.
    pub row_steps: u64,
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
    let prepared = PreparedQueries::prepare(
        program,
        PreparationLimits::default().max_dense_atoms,
        &mut work,
    )?;
    prepared.check_with(seed, &mut ClosureWorkspace::default(), &mut work)
}

// Construction establishes complete source coverage through bootstrap and
// completed round history, not constraint satisfaction or seed agreement.
pub(crate) struct CompletedClosure {
    pub(crate) atoms: Model,
    /// A constraint fired in the completed closure: under a frozen seed, the
    /// candidate is rejected; under the definite reading of a cube, no seed
    /// of the cube is accepted (`Bounds.lower_constraint_refutes`).
    pub(crate) constraint_violated: bool,
}

/// A region of seeds: the gate atoms every seed of it holds and the gate
/// atoms some seed of it may hold. `None` for `may` is the whole symbolic
/// carrier, which is never materialized. This is the `Cube` of
/// `Bounds.lean` with `undecided` as its origin.
pub(crate) struct Cube {
    pub(crate) must: BTreeSet<Atom>,
    pub(crate) may: Option<BTreeSet<Atom>>,
}
impl Cube {
    /// Nothing decided: every seed lies in this cube.
    pub(crate) fn undecided() -> Self {
        Self {
            must: BTreeSet::new(),
            may: None,
        }
    }
    fn must_hold(&self, key: &zetesis_core::AtomKey<'_>) -> bool {
        key.get(&self.must).is_some()
    }
    fn may_hold(&self, key: &zetesis_core::AtomKey<'_>) -> bool {
        self.may.as_ref().is_none_or(|may| key.get(may).is_some())
    }
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
    let prepared =
        PreparedQueries::prepare(program, PreparationLimits::default().max_dense_atoms, work)?;
    prepared.closure_with(
        Gates::Frozen(seed),
        &mut ClosureWorkspace::default(),
        Schedule::Delta,
        work,
    )
}

/// How a rule's gates are read while a closure is computed. The two cube
/// readings are `MustGate` and `MayGate` of `Bounds.lean`.
#[derive(Clone, Copy)]
enum Gates<'a> {
    /// The frozen seed decides every gate: a candidate's closure.
    Frozen(SeedView<'a>),
    /// A gate holds only if it holds under every seed of the cube, so the
    /// closure lies inside every answer set the cube contains: its lower
    /// closure.
    Definite(&'a Cube),
    /// A gate holds if it holds under some seed of the cube, so every answer
    /// set the cube contains lies inside the closure: its upper closure.
    Possible(&'a Cube),
}
impl Gates<'_> {
    /// Whether a rule can fire under this reading before its gates are
    /// bound: a definite rule needs each `not not` gate in `must` and each
    /// `not` gate outside `may`, which an empty `must` or an unbounded `may`
    /// rules out for the whole template.
    fn admits(self, template: &Template) -> bool {
        match self {
            Self::Frozen(_) | Self::Possible(_) => true,
            Self::Definite(cube) => {
                (template.gate_true().is_empty() || !cube.must.is_empty())
                    && (template.gate_false().is_empty() || cube.may.is_some())
            }
        }
    }
    /// Whether the gate `pattern` bound to `key` holds under this reading;
    /// `required` is true for a `not not` gate and false for a `not` gate.
    fn holds(self, key: &zetesis_core::AtomKey<'_>, required: bool) -> bool {
        match (self, required) {
            (Self::Frozen(seed), _) => seed.contains_key(key) == required,
            (Self::Definite(cube), true) => cube.must_hold(key),
            (Self::Definite(cube), false) => !cube.may_hold(key),
            (Self::Possible(cube), true) => cube.may_hold(key),
            (Self::Possible(cube), false) => !cube.must_hold(key),
        }
    }
}

/// The lower closure of `cube` over `prepared`, computed in `workspace`:
/// rules fire only under gates every seed of the cube satisfies, so the
/// closure lies inside every answer set of the cube and its gate atoms
/// inside every accepted seed (`Bounds.undecided_bounds_accepted` from the
/// undecided cube). A fired constraint refutes the whole cube.
///
/// # Errors
/// Returns [`Stop`] for cancellation, deadlines and the closure budgets of
/// `limits`, charged as one candidate check would be; the workspace is then
/// retired and no partial closure is returned.
pub(crate) fn definite_closure(
    prepared: &PreparedQueries,
    workspace: &mut ClosureWorkspace,
    cube: &Cube,
    limits: Limits,
    control: &Control,
) -> Result<CompletedClosure, Stop> {
    prepared.closure_of(Gates::Definite(cube), workspace, limits, control)
}

/// The upper closure of `cube` over `prepared`, computed in `workspace`:
/// rules fire under gates some seed of the cube satisfies, so every answer
/// set of the cube lies inside it and every accepted seed inside its gate
/// atoms; a gate atom outside it belongs to no answer set of the cube. Its
/// constraint verdict says nothing about any seed.
///
/// # Errors
/// As [`definite_closure`].
pub(crate) fn possible_closure(
    prepared: &PreparedQueries,
    workspace: &mut ClosureWorkspace,
    cube: &Cube,
    limits: Limits,
    control: &Control,
) -> Result<CompletedClosure, Stop> {
    prepared.closure_of(Gates::Possible(cube), workspace, limits, control)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Schedule {
    Delta,
    #[cfg(test)]
    Full,
}

#[derive(Clone, Copy)]
enum Selection {
    All,
    FirstNew(usize),
}

impl Selection {
    fn rows(self, occurrence: usize) -> RowSet {
        match self {
            Self::FirstNew(pivot) if occurrence < pivot => RowSet::Old,
            Self::FirstNew(pivot) if occurrence == pivot => RowSet::New,
            Self::All | Self::FirstNew(_) => RowSet::Current,
        }
    }

    /// The source occurrence visited at a join depth. An incremental round
    /// visits its new rows first: they are the fewest, and the variables they
    /// bind turn every other occurrence into a bound-prefix window instead of
    /// a scan. The remaining occurrences keep their body order.
    fn occurrence(self, depth: usize) -> usize {
        match self {
            Self::All => depth,
            Self::FirstNew(pivot) => match depth {
                0 => pivot,
                depth if depth <= pivot => depth - 1,
                depth => depth,
            },
        }
    }
}

struct RoundWorkspace<'a> {
    buffers: &'a mut prepared::Buffers,
    dimensions: &'a prepared::Dimensions,
    rules: &'a prepared::Rules,
    layouts: &'a Layouts,
    row_steps: &'a RowSteps,
    pending: &'a mut PendingRows,
    overhead: u128,
}

fn least_closure_with(
    program: &Program,
    gates: Gates<'_>,
    closure: &mut Catalogs,
    workspace: RoundWorkspace<'_>,
    schedule: Schedule,
    work: &mut Work<'_>,
) -> Result<CompletedClosure, Stop> {
    let RoundWorkspace {
        buffers,
        dimensions,
        rules,
        layouts,
        row_steps,
        pending,
        overhead,
    } = workspace;
    let mut constraint_violated = false;
    closure.create_dense_relations(layouts, work)?;
    loop {
        work.tick()?;
        let incremental = schedule == Schedule::Delta && work.statistics.rounds != 0;
        if incremental {
            closure.prepare_delta(work)?;
        } else {
            closure.prepare(work)?;
        }
        // Bindings borrow this round's immutable catalog extent only. The
        // reference-free cursor/undo buffers survive after these bindings drop.
        let consequences = {
            let (mut assignment, bytes) = prepared::assignment(
                dimensions,
                closure
                    .owned_bytes()
                    .checked_add(overhead)
                    .ok_or(Stop::StorageLimit)?,
                work,
            )?;
            closure.set_overhead(overhead.checked_add(bytes).ok_or(Stop::StorageLimit)?, work)?;
            visit_round(
                program,
                gates,
                closure,
                incremental.then_some(rules),
                DenseHeads {
                    layouts,
                    row_steps,
                    pending: &mut *pending,
                },
                Frame {
                    assignment: &mut assignment,
                    buffers: &mut *buffers,
                    selection: Selection::All,
                },
                work,
            )?
        };
        let RoundConsequences {
            atoms: delta,
            bytes: mut pending_bytes,
            constraint_violated: triggered,
        } = consequences;
        constraint_violated |= triggered;
        closure.set_overhead(overhead, work)?;
        work.statistics.rounds += 1;
        if delta.is_empty() && pending.is_empty() {
            break;
        }
        if schedule == Schedule::Delta {
            closure.advance(work)?;
        }
        for atom in delta {
            let bytes = relations::atom_bytes(&atom, work)?;
            pending_bytes = pending_bytes
                .checked_sub(bytes)
                .ok_or(Stop::InvalidProgram)?;
            closure.insert(atom, pending_bytes, work)?;
        }
        closure.absorb(pending, layouts, work)?;
    }
    Ok(CompletedClosure {
        atoms: closure.take_model(work)?,
        constraint_violated,
    })
}

/// What a round derived for the tree relations, and whether a constraint
/// fired. Its dense heads are the marks left in the pending rows.
struct RoundConsequences {
    atoms: BTreeSet<Atom>,
    bytes: u128,
    constraint_violated: bool,
}

/// Record a derived head unless the closure or the round already holds it: a
/// head of a dense relation as a pending bit, any other as a pending atom. The
/// atoms the closure holds, the round's pending atoms and its pending bits are
/// disjoint, so their sum is the count the derived-atom limit bounds.
fn record_head(
    key: AtomKey<'_>,
    dense_head: Option<(usize, &Dense)>,
    closure: &Catalogs,
    result: &mut RoundConsequences,
    pending: &mut PendingRows,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    let held = closure
        .len()
        .checked_add(result.atoms.len())
        .and_then(|atoms| atoms.checked_add(pending.len()))
        .ok_or(Stop::DerivedAtomLimit)?;
    if let Some((slot, dense)) = dense_head {
        // The bounds cover every derivable head: a key without a position
        // violates the admitted program's invariant.
        work.charge(key.predicate().arity())?;
        let position = dense.position(&key).ok_or(Stop::InvalidProgram)?;
        if !dense.contains(position) && pending.mark(slot, position) {
            if held >= work.limits.max_derived_atoms {
                return Err(Stop::DerivedAtomLimit);
            }
            work.statistics.dense_heads += 1;
        }
    } else if !closure.contains(&key, result.bytes, work)? && key.get(&result.atoms).is_none() {
        if held >= work.limits.max_derived_atoms {
            return Err(Stop::DerivedAtomLimit);
        }
        let (atom, bytes) = closure.pending(key, result.bytes, work)?;
        result.atoms.insert(atom);
        result.bytes = result.bytes.checked_add(bytes).ok_or(Stop::StorageLimit)?;
    }
    Ok(())
}

/// Where a round records a head of a dense relation: the layouts name the
/// head's slot, and the pending rows take its position.
struct DenseHeads<'a> {
    layouts: &'a Layouts,
    row_steps: &'a RowSteps,
    pending: &'a mut PendingRows,
}

/// What a join reports to: each complete binding, and, where the consumer
/// can take them whole, the block of rows its innermost depth would bind one
/// by one. A closure is a consumer of bindings alone.
trait Sink<'source, E> {
    fn binding(
        &mut self,
        assignment: &[Option<&'source Value>],
        work: &mut Work<'_>,
    ) -> Result<(), E>;

    /// Whether the rows of this occurrence, visited innermost, are taken by
    /// [`Self::rows`] instead of bound singly.
    fn steps_by_rows(&self, _occurrence: usize) -> bool {
        false
    }

    /// Take the block of rows matching the assignment, which binds every
    /// variable of the rule but the occurrence's last.
    fn rows(
        &mut self,
        _rows: Block<'source>,
        _assignment: &[Option<&'source Value>],
        _work: &mut Work<'_>,
    ) -> Result<(), E> {
        Ok(())
    }
}

impl<'source, E, F> Sink<'source, E> for F
where
    F: FnMut(&[Option<&'source Value>], &mut Work<'_>) -> Result<(), E>,
{
    fn binding(
        &mut self,
        assignment: &[Option<&'source Value>],
        work: &mut Work<'_>,
    ) -> Result<(), E> {
        self(assignment, work)
    }
}

/// The consumer of one template's joins in a closure round.
struct RoundSink<'a, 'source> {
    template: &'a Template,
    closure: &'source Catalogs,
    /// The head's pending-row slot and relation, when the head is laid out.
    dense_head: Option<(usize, &'source Dense)>,
    /// The template's row-step plan, by positive occurrence.
    row_steps: &'a [bool],
    result: &'a mut RoundConsequences,
    pending: &'a mut PendingRows,
}

impl<'source> Sink<'source, Stop> for RoundSink<'_, 'source> {
    fn binding(
        &mut self,
        assignment: &[Option<&'source Value>],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.tick()?;
        if let Some(head) = self.template.head() {
            work.charge(head.terms().len())?;
            let key = head.key(assignment).map_err(|_| Stop::InvalidProgram)?;
            record_head(
                key,
                self.dense_head,
                self.closure,
                self.result,
                self.pending,
                work,
            )?;
        } else {
            self.result.constraint_violated = true;
        }
        Ok(())
    }

    fn steps_by_rows(&self, occurrence: usize) -> bool {
        self.row_steps.get(occurrence).copied().unwrap_or(false)
    }

    // The plan admitted the occurrence, so the head is laid out, ends in the
    // occurrence's last variable and lists its values as the occurrence does:
    // the heads of the block are the block of the head's bound prefix, place
    // for place, and each row of the block is one binding.
    fn rows(
        &mut self,
        rows: Block<'source>,
        assignment: &[Option<&'source Value>],
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        let (Some(head), Some((slot, dense))) = (self.template.head(), self.dense_head) else {
            return Err(Stop::InvalidProgram);
        };
        let bound = head.terms().len().saturating_sub(1);
        work.charge(bound)?;
        let heads = dense.layout().prefix_range(
            head.terms()[..bound]
                .iter()
                .map_while(|term| resolve(term, assignment)),
        );
        if heads.len() != rows.len {
            // The head's bounds cover every derivable head: a head prefix
            // outside them derives nothing, so the block holds no row.
            return if rows.is_empty(work)? {
                Ok(())
            } else {
                Err(Stop::InvalidProgram)
            };
        }
        let derived = self
            .closure
            .len()
            .checked_add(self.result.atoms.len())
            .and_then(|atoms| atoms.checked_add(self.pending.len()))
            .ok_or(Stop::DerivedAtomLimit)?;
        let joined = self
            .pending
            .join_row(slot, dense, heads.start, rows, work)?;
        if derived
            .checked_add(joined.marked)
            .is_none_or(|atoms| atoms > work.limits.max_derived_atoms)
        {
            return Err(Stop::DerivedAtomLimit);
        }
        let count = |n: usize| u64::try_from(n).map_err(|_| Stop::InvalidProgram);
        work.statistics.bindings += count(joined.offered)?;
        work.statistics.dense_heads += count(joined.marked)?;
        work.statistics.row_steps += 1;
        Ok(())
    }
}

// Complete the disjoint source family before publishing either its history or
// its consequences. A constraint latch never skips another source occurrence.
/// Visit every template in the bootstrap round, or, given the rule index of
/// an incremental round, only the templates whose body names a predicate
/// with new rows: no other template can bind anew.
fn visit_round<'source>(
    program: &Program,
    gates: Gates<'_>,
    closure: &'source Catalogs,
    incremental: Option<&prepared::Rules>,
    dense_heads: DenseHeads<'_>,
    frame: Frame<'_, 'source>,
    work: &mut Work<'_>,
) -> Result<RoundConsequences, Stop> {
    let DenseHeads {
        layouts,
        row_steps,
        pending,
    } = dense_heads;
    let Frame {
        assignment,
        buffers,
        ..
    } = frame;
    let mut result = RoundConsequences {
        atoms: BTreeSet::new(),
        bytes: 0,
        constraint_violated: false,
    };
    let visited = match incremental {
        None => program.templates().len(),
        Some(rules) => {
            buffers.rules.clear();
            for predicate in closure.predicates_with_new() {
                work.tick()?;
                for &index in rules.naming(predicate) {
                    work.tick()?;
                    buffers.rules.push(index);
                }
            }
            work.charge(buffers.rules.len())?;
            buffers.rules.sort_unstable();
            buffers.rules.dedup();
            buffers.rules.len()
        }
    };
    for position in 0..visited {
        let index = match incremental {
            None => position,
            Some(_) => buffers.rules[position],
        };
        let template = &program.templates()[index];
        work.tick()?;
        if !gates.admits(template) {
            continue;
        }
        // A laid-out head has its relation from the start of the closure, so
        // both are resolved once for the template, not once per binding.
        let dense_head = match template
            .head()
            .and_then(|head| layouts.slot(head.predicate()).zip(Some(head.predicate())))
        {
            Some((slot, predicate)) => {
                Some((slot, closure.dense(predicate).ok_or(Stop::InvalidProgram)?))
            }
            None => None,
        };
        let mut emit = RoundSink {
            template,
            closure,
            dense_head,
            row_steps: row_steps.of(index),
            result: &mut result,
            pending: &mut *pending,
        };
        if incremental.is_some() {
            // Repeated predicates retain distinct occurrences. Earlier Old,
            // this New and later Current rows select the unique first new row.
            for (pivot, pattern) in template.positive().iter().enumerate() {
                if closure.has_new(pattern.predicate(), work)? {
                    visit_with(
                        template,
                        closure,
                        gates,
                        None,
                        work,
                        &mut emit,
                        Frame {
                            assignment: &mut *assignment,
                            buffers: &mut *buffers,
                            selection: Selection::FirstNew(pivot),
                        },
                    )?;
                }
            }
        } else {
            // Bootstrap includes every zero-positive head and constraint under
            // the frozen gates. The test reference repeats this complete scan.
            visit_with(
                template,
                closure,
                gates,
                None,
                work,
                &mut emit,
                Frame {
                    assignment: &mut *assignment,
                    buffers: &mut *buffers,
                    selection: Selection::All,
                },
            )?;
        }
    }
    Ok(result)
}

// Establish closure ∩ gate_carrier = seed in both directions by one merge:
// the closure's rows of each gate predicate, a contiguous range of the model,
// walked against the seed's atoms, both in canonical order. Charges one unit
// per gate predicate, per closure gate row and per seed atom left unmatched,
// and keeps charging after a mismatch; rejection does not bypass work limits.
// Positive-only atoms do not belong to this comparison's projected carrier.
fn gate_agreement(
    program: &Program,
    seed: SeedView<'_>,
    closure: ModelAtoms<'_>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    let mut agreement = true;
    let mut seed = seed.atoms().peekable();
    for predicate in program.gate_predicates() {
        work.tick()?;
        for atom in closure.of_predicate(predicate) {
            work.tick()?;
            while seed.peek().is_some_and(|pending| *pending < atom) {
                // A seed atom the closure never derived.
                work.tick()?;
                seed.next();
                agreement = false;
            }
            if seed.peek() == Some(&atom) {
                seed.next();
            } else {
                // A derived gate atom the seed does not hold.
                agreement = false;
            }
        }
    }
    for _ in seed {
        work.tick()?;
        agreement = false;
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
    gates: Gates<'_>,
    membership: Option<&mut worlds::Join<'_>>,
    work: &mut Work<'_>,
    mut emit: impl FnMut(&[Option<&'source Value>], &mut Work<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let mut assignment = vec![None; template.variable_count()];
    let mut buffers = prepared::Buffers::local(template.positive().len());
    visit_with(
        template,
        relations,
        gates,
        membership,
        work,
        &mut emit,
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
    gates: Gates<'_>,
    mut membership: Option<&mut worlds::Join<'_>>,
    work: &mut Work<'_>,
    emit: &mut impl Sink<'source, E>,
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
    if !guards(template, assignment, gates, work)? {
        return Ok(());
    }
    let count = template.positive().len();
    if count == 0 {
        if !assignment.is_empty() {
            return Err(Stop::InvalidProgram.into());
        }
        work.statistics.bindings += 1;
        return emit.binding(assignment, work);
    }
    if let Some(membership) = membership.as_mut() {
        membership.reset(template, work)?;
    }
    // None means this depth has not yet been opened for the current parent
    // assignment. A retained range advances in the original relation order.
    let cursors = &mut buffers.cursors[..count];
    let slots = &mut buffers.slots[..count];
    let undo = &mut buffers.undo[..count];
    work.charge(count)?;
    cursors.fill(None);
    slots.fill(Slot::Unresolved);
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
            emit.binding(assignment, work)?;
            depth -= 1;
            clear(assignment, &mut undo[depth]);
            continue;
        }
        let occurrence = selection.occurrence(depth);
        let pattern = &template.positive()[occurrence];
        // The relation is fixed for the depth: resolve it on first entry and
        // index it on every later probe.
        if slots[depth] == Slot::Unresolved {
            work.tick()?;
            slots[depth] = relations
                .resolve(pattern.predicate())
                .map_or(Slot::Absent, Slot::At);
        }
        let tuples = match slots[depth] {
            Slot::At(handle) => relations.rows_at(handle, selection.rows(occurrence))?,
            Slot::Absent | Slot::Unresolved => Rows::Borrowed(&[]),
        };
        let cursor = &mut cursors[depth];
        // The innermost depth, entered for this parent assignment. A
        // membership join follows single rows, so it keeps them.
        if depth + 1 == count
            && cursor.is_none()
            && membership.is_none()
            && step_by_rows(pattern, tuples, occurrence, assignment, emit, work)?
        {
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            clear(assignment, &mut undo[depth]);
            continue;
        }
        let window = match cursor {
            Some(window) => window,
            None => cursor.insert(window::Window::open(pattern, tuples, assignment, work)?),
        };
        let Some((run, index)) = window.next(pattern, tuples, assignment, work)? else {
            *cursor = None;
            if depth == 0 {
                return Ok(());
            }
            depth -= 1;
            clear(assignment, &mut undo[depth]);
            continue;
        };
        let row = tuples.get(run, index).ok_or(Stop::InvalidProgram)?;
        // This iteration already charged one join step and offers at most one
        // row. The cumulative probe count therefore cannot exceed charged work.
        work.statistics.tuple_probes += 1;
        if bind(pattern, row, assignment, &mut undo[depth], work)?
            && guards(template, assignment, gates, work)?
            && match membership.as_mut() {
                Some(membership) => membership.extend(occurrence, index, work)?,
                None => true,
            }
        {
            depth += 1;
        } else {
            clear(assignment, &mut undo[depth]);
        }
    }
}

/// Give a consumer that steps by rows the whole block of an occurrence's rows
/// matching the assignment, in place of binding them one by one; whether it
/// was given. The block is that of the bound leading terms, which for such an
/// occurrence are all but its last.
fn step_by_rows<'source, E: From<Stop>>(
    pattern: &AtomPattern,
    tuples: Rows<'source>,
    occurrence: usize,
    assignment: &[Option<&'source Value>],
    emit: &mut impl Sink<'source, E>,
    work: &mut Work<'_>,
) -> Result<bool, E> {
    let Rows::Dense { relation, set } = tuples else {
        return Ok(false);
    };
    if !emit.steps_by_rows(occurrence) {
        return Ok(false);
    }
    // A bound value outside its argument's bound matches no position: the
    // block is empty and there is nothing to take.
    let block = window::matching_prefix(pattern, tuples, 0, assignment, work)?;
    if !block.is_empty() {
        emit.rows(
            Block {
                relation,
                set,
                start: block.start,
                len: block.len(),
            },
            assignment,
            work,
        )?;
    }
    Ok(true)
}

fn bind<'source>(
    pattern: &AtomPattern,
    row: Row<'source>,
    assignment: &mut [Option<&'source Value>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Stop> {
    for (term, value) in pattern.terms().iter().zip(row.values()) {
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
    gates: Gates<'_>,
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
    for (patterns, required) in [(template.gate_true(), true), (template.gate_false(), false)] {
        for pattern in patterns {
            work.tick()?;
            work.charge(pattern.terms().len())?;
            // An absent referenced slot defers this gate until a later join
            // supplies it. A complete key borrows the exact lookup tuple.
            if let Ok(key) = pattern.key(assignment)
                && !gates.holds(&key, required)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
