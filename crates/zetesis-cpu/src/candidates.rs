//! Incremental seed enumeration with optional completed-support narrowing.
//!
//! The unbounded counter discovers canonical carrier atoms incrementally.
//! Completed narrowing instead supplies the supported gate atoms directly,
//! preserving their full-carrier positions without enumerating excluded tuples.

use std::iter::FusedIterator;
use std::sync::Arc;
use zetesis_core::{
    CarrierAtom, CarrierFailure, GateAtom, GateAtomError, GateAtoms, GateIndex, GateIndexError,
    GateIndexFailure, Model, Program, Seed, SeedSelection, SeedSelectionError,
};

use crate::oracle::restrictions::{Conflict, Restrictions};
use crate::oracle::{
    Bounds, CarrierSet, ClosureWorkspace, Cube, Limits, PreparationLimits, PreparedQueries,
    RegionBounds, Work, definite_closure, model_contains, possible_closure,
};
use crate::regions::{Counting, Narrowing, Region, Traversal, Visit};
use crate::{Cancellation, Stop};

/// Explicit limits for complete seed enumeration. Zero is a real ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateLimits {
    /// Maximum seeds returned successfully, including the empty seed.
    pub max_candidates: u64,
    /// Maximum carrier atoms retained by the incremental binary counter.
    pub max_carrier_atoms: usize,
}

impl Default for CandidateLimits {
    fn default() -> Self {
        Self {
            max_candidates: 1_000_000,
            max_carrier_atoms: 1_000_000,
        }
    }
}

/// Bounds for optional source-certified candidate restriction preparation and
/// traversal. They are cumulative across the entire candidate iterator.
#[derive(Clone, Copy, Debug)]
pub struct CandidateRestrictionLimits {
    /// Source joins, checked coordinate lookup, metadata moves and premise
    /// comparisons across all seeds. No owned atom canonicalization is needed.
    pub max_work: u64,
    /// Fact and forbidden-conjunction coordinate occurrences admitted during preparation.
    pub max_atoms: usize,
    /// Named coordinate, borrowed-row and pattern-handle capacities during
    /// preparation, including old/replacement buffer overlap. Shared Program
    /// storage and allocator/Arc bookkeeping are excluded; local join scratch
    /// shares this allowance while the borrowed query is active.
    pub max_bytes: usize,
}

impl Default for CandidateRestrictionLimits {
    fn default() -> Self {
        Self {
            max_work: 100_000_000,
            max_atoms: 1_000_000,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Candidate storage and optional narrowing/filter work. The original powerset
/// constructor leaves the optional optimization counters zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateStatistics {
    /// Charged construction and traversal operations, retained after interruption.
    pub restriction_work: u64,
    /// Fact and forbidden-conjunction coordinate occurrences in completed preparation.
    /// Zero when preparation failed; `restriction_work` retains failed work.
    pub restriction_atoms: usize,
    /// Retained forbidden-conjunction coordinate and handle capacities.
    /// Preparation facts and borrowed query buffers have already been released.
    pub restriction_bytes: usize,
    /// Peak admitted coordinate and metadata storage during completed
    /// preparation, including temporary facts and pattern partitions.
    /// Shared Program storage is separate; zero on failure.
    pub restriction_peak_bytes: usize,
    /// Positive gate conjunctions obtained from covered fact-side bindings.
    pub restriction_conjunctions: usize,
    /// Certified impossible binary intervals skipped, not individual seeds.
    pub conflicts: u64,
    /// Gate atoms the narrowing cut, outside its upper closure and never
    /// offered to the counter: no rule derives them under any gate
    /// assumption, so no answer set holds them. A completed narrowed root
    /// counts the full complement symbolically; the fallback counter counts
    /// excluded atoms as it encounters them.
    pub cut_gate_atoms: usize,
    /// Gate atoms the narrowing held, in its lower closure and held in
    /// every seed instead of counted: every answer set holds them.
    pub held_gate_atoms: usize,
    /// Why a narrowing stopped early, the root's or any region's below it:
    /// the preparation or a closure stopped on a resource ceiling, and the
    /// counter kept the bounds of the passes that completed, the whole
    /// symbolic carrier when none had. The first stop is kept. `None` when
    /// no bound was requested or every narrowing reached its fixed point.
    pub narrowing_stop: Option<Stop>,
    /// The root's narrowing passes that changed a bound or found the fixed
    /// point, each two closures; a pass that refuted the root or stopped is
    /// not counted.
    pub narrowing_passes: usize,
    /// Checked coordinate lookup, set reads and decision transfer after completed
    /// narrowing closures, including work retained on a stopped transfer. Each
    /// pass has a separate `Limits::max_work` allowance.
    pub narrowing_transfer_work: u64,
    /// Largest admitted named transfer buffer envelope. This excludes completed
    /// Models, closure workspaces, allocator overhead and shared Program storage.
    pub narrowing_transfer_peak_bytes: usize,
    /// The admitted Program's storage, shared by every retained carrier token
    /// and counted once, independently of candidate/closure storage ceilings.
    pub shared_program_bytes: u128,
    /// Retained carrier tuple capacities, counting each shared coordinate owner
    /// once across the root, held selection and current binary counter.
    pub coordinate_bytes: u128,
    /// Named candidate handle, indexed-witness and decision-vector capacities.
    /// Arc counters, traversal history and closure owners are separate.
    pub candidate_metadata_bytes: u128,
    /// A constraint fired in the lower closure of the narrowed root, so no
    /// seed of it is accepted and the counter offered none
    /// (`Bounds.lower_constraint_refutes`).
    pub root_refuted: bool,
    /// Regions the split visited, the narrowed root included.
    pub regions: usize,
    /// Regions a definite constraint refuted in their lower closure: no seed
    /// of them was offered.
    pub regions_refuted: usize,
    /// Regions visited as leaves, their narrowing having decided every gate
    /// atom: one seed each, offered without a count.
    pub regions_leaves: usize,
    /// Regions counted as a flat interval, because their narrowing decided
    /// nothing beyond the split that formed them or a closure stopped inside
    /// them; the count's restrictions still apply within.
    pub regions_counted: usize,
    /// The narrowing passes over the regions below the root that changed a
    /// bound or found a fixed point, each two closures, as
    /// [`Self::narrowing_passes`] counts the root's.
    pub region_passes: usize,
}

enum RestrictionState {
    Disabled,
    Pending(CandidateRestrictionLimits),
    Prepared {
        limits: CandidateRestrictionLimits,
        plan: Restrictions,
    },
}

/// Whether the program's two closures narrow the carrier, and how far that got.
enum NarrowingState {
    Disabled,
    Pending(Limits),
    /// The root is narrowed; its regions are narrowed by the same closures,
    /// held behind one allocation made when the bounds are applied.
    Applied(Box<Closures>),
    /// The carrier is empty: the one seed needs no closure.
    Trivial,
    /// The preparation or a closure stopped on a resource ceiling; the
    /// counter keeps the bounds of the passes that completed.
    Unavailable,
}

/// The closures of the narrowing: the program prepared once for every
/// closure the narrowing computes, one workspace, since one closure is live
/// at a time, and the limits each closure is charged under. Built when the
/// bounds are first applied, it lives with the iterator and retains the
/// preparation's bytes and the workspace's capacity; every closure admits
/// both under `limits.max_closure_bytes` before it runs, and a closure a
/// resource ceiling stops retires the workspace. Its work is one
/// preparation, charged once under `limits.max_work`, and then each
/// closure's, charged as one candidate check's is.
struct Closures {
    prepared: PreparedQueries,
    workspace: ClosureWorkspace,
    limits: Limits,
    cancellation: Cancellation,
    transfer_work: u64,
    transfer_peak_bytes: usize,
}

impl Closures {
    /// Prepare `program` under `limits`.
    ///
    /// # Errors
    /// Returns the preparation's stop: cancellation, a deadline, or its work
    /// or storage ceiling.
    fn new(program: &Program, limits: Limits, cancellation: Cancellation) -> Result<Self, Stop> {
        let prepared = PreparedQueries::new(
            program,
            PreparationLimits {
                max_work: limits.max_work,
                max_bytes: limits.max_closure_bytes,
                max_dense_atoms: PreparationLimits::default().max_dense_atoms,
            },
            &cancellation,
        )?;
        Ok(Self {
            prepared,
            workspace: ClosureWorkspace::default(),
            limits,
            cancellation,
            transfer_work: 0,
            transfer_peak_bytes: 0,
        })
    }

    /// Read both closures from the same immutable pre-pass bounds. Only a
    /// completed lower constraint refutes without computing the upper closure.
    fn enclose(&mut self, bounds: Bounds<'_>) -> Result<Enclosure, Stop> {
        let lower = definite_closure(
            &self.prepared,
            &mut self.workspace,
            bounds,
            self.limits,
            &self.cancellation,
        )?;
        if lower.constraint_violated {
            return Ok(Enclosure::Refuted);
        }
        let upper = possible_closure(
            &self.prepared,
            &mut self.workspace,
            bounds,
            self.limits,
            &self.cancellation,
        )?;
        Ok(Enclosure::Complete {
            lower: lower.atoms,
            upper: upper.atoms,
        })
    }

    /// One transfer has an independent `max_work` allowance after its two
    /// completed closures. Coordinate/handle capacity is bounded separately by
    /// `max_closure_bytes`; it excludes their retained Models, the prepared
    /// workspace, shared Program storage and allocator/Arc bookkeeping.
    fn narrow(&mut self, cube: &mut Cube) -> Result<Pass, Stop> {
        let Enclosure::Complete { lower, upper } = self.enclose((&*cube).into())? else {
            return Ok(Pass::Refuted);
        };
        let mut work = Work::source(&self.cancellation, self.limits.max_work);
        let mut peak = 0;
        let result = transfer_root(
            self.prepared.program(),
            cube,
            &lower,
            &upper,
            self.limits.max_closure_bytes,
            &mut peak,
            &mut work,
        );
        self.transfer_work = self
            .transfer_work
            .saturating_add(work.source_statistics(0).work);
        self.transfer_peak_bytes = self.transfer_peak_bytes.max(peak);
        result
    }

    /// Complete every checked read and stage decisions before changing history.
    /// A stopped transfer leaves this pass's pre-state intact.
    fn narrow_region(
        &mut self,
        held: &CarrierSet,
        root: &[Arc<GateAtom>],
        region: &mut Region,
    ) -> Result<Pass, Stop> {
        let bounds = RegionBounds::new(held, root, region);
        let Enclosure::Complete { lower, upper } = self.enclose(Bounds::Region(&bounds))? else {
            return Ok(Pass::Refuted);
        };
        let mut work = Work::source(&self.cancellation, self.limits.max_work);
        let mut peak = 0;
        let result = (|| {
            if bounds.conflicts(self.prepared.program(), &lower, &upper, &mut work)? {
                return Ok(None);
            }
            let mut decisions = Vec::new();
            for (at, gate) in root.iter().enumerate() {
                work.tick()?;
                if region.is_open(at) {
                    let held = model_contains(&lower, gate.atom(), &mut work)?;
                    if held || !model_contains(&upper, gate.atom(), &mut work)? {
                        reserve(
                            &mut decisions,
                            1,
                            0,
                            self.limits.max_closure_bytes,
                            &mut peak,
                            &mut work,
                        )?;
                        work.tick()?;
                        decisions.push((at, held));
                    }
                }
            }
            // Admission of every commit precedes mutation: no fallible read,
            // cancellation check or resource charge occurs during this commit.
            for _ in &decisions {
                work.tick()?;
            }
            Ok(Some(decisions))
        })();
        self.transfer_work = self
            .transfer_work
            .saturating_add(work.source_statistics(0).work);
        self.transfer_peak_bytes = self.transfer_peak_bytes.max(peak);
        let Some(decisions) = result? else {
            return Ok(Pass::Refuted);
        };
        let changed = !decisions.is_empty();
        for (at, held) in decisions {
            if held {
                region.hold(at);
            } else {
                region.cut(at);
            }
        }
        Ok(if changed { Pass::Changed } else { Pass::Fixed })
    }
}

/// Named buffers for the transfer itself. Models and the immutable Program are
/// retained independently; the latter is reported once on the candidate owner.
fn cube_bytes(cube: &Cube) -> u128 {
    cube.must.handle_bytes()
        + cube.may.as_ref().map_or_else(
            || cube.must.coordinate_bytes(),
            |may| may.handle_bytes() + may.coordinate_bytes(),
        )
}

fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    other: u128,
    limit: usize,
    peak: &mut usize,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(Stop::Allocation)?;
    if required > values.capacity() {
        let target = required.max(values.capacity().saturating_mul(2)).max(4);
        let old = values.capacity() as u128 * size_of::<T>() as u128;
        let replacement = target as u128 * size_of::<T>() as u128;
        admit_transfer(other + old + replacement, limit, peak)?;
        for _ in 0..values.len().max(1) {
            work.tick()?;
        }
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| Stop::Allocation)?;
        admit_transfer(
            other + old + values.capacity() as u128 * size_of::<T>() as u128,
            limit,
            peak,
        )?;
    }
    admit_transfer(
        other + values.capacity() as u128 * size_of::<T>() as u128,
        limit,
        peak,
    )
}

fn admit_transfer(bytes: u128, limit: usize, peak: &mut usize) -> Result<(), Stop> {
    if bytes > limit as u128 {
        return Err(Stop::Allocation);
    }
    *peak = (*peak).max(usize::try_from(bytes).map_err(|_| Stop::Allocation)?);
    Ok(())
}

fn carrier_stop(error: &CarrierFailure<Stop>) -> Stop {
    match error {
        CarrierFailure::Stopped(stop) => *stop,
        CarrierFailure::Storage(zetesis_core::CarrierError::Coordinates) => Stop::InvalidProgram,
        CarrierFailure::Storage(_) => Stop::Allocation,
    }
}

fn transfer_root(
    program: &Program,
    cube: &mut Cube,
    lower: &Model,
    upper: &Model,
    limit: usize,
    peak: &mut usize,
    work: &mut Work<'_>,
) -> Result<Pass, Stop> {
    let old_bytes = cube_bytes(cube);
    admit_transfer(old_bytes, limit, peak)?;
    let mut may = CarrierSet::new();
    let mut must = CarrierSet::new();
    let mut coordinates = 0u128;
    for atom in upper.atoms() {
        work.tick()?;
        let live = old_bytes + may.handle_bytes() + must.handle_bytes() + coordinates;
        let remaining = usize::try_from((limit as u128).checked_sub(live).ok_or(Stop::Allocation)?)
            .map_err(|_| Stop::Allocation)?;
        let Some(carrier) = program
            .locate_atom_with(atom, true, remaining, || work.tick())
            .map_err(|error| carrier_stop(&error))?
        else {
            continue;
        };
        // The just-created tuple is live even if the prior upper bound cuts it.
        let tuple_bytes = carrier.coordinate_bytes();
        admit_transfer(live + tuple_bytes, limit, peak)?;
        if let Some(previous) = &cube.may
            && !previous.contains_atom(atom, work)?
        {
            continue;
        }
        let held = cube.must.contains_atom(atom, work)? || model_contains(lower, atom, work)?;
        coordinates += tuple_bytes;
        may.reserve(
            1,
            old_bytes + must.handle_bytes() + coordinates,
            limit,
            peak,
            work,
        )?;
        if held {
            must.reserve(
                1,
                old_bytes + may.handle_bytes() + coordinates,
                limit,
                peak,
                work,
            )?;
            must.push_ordered(carrier.clone(), work)?;
        }
        may.push_ordered(carrier, work)?;
        admit_transfer(
            old_bytes + may.handle_bytes() + must.handle_bytes() + coordinates,
            limit,
            peak,
        )?;
    }
    for atom in cube.must.iter() {
        if !may.contains_atom(atom.atom(), work)? {
            return Ok(Pass::Refuted);
        }
    }
    for atom in lower.atoms() {
        work.tick()?;
        if program
            .gate_predicates()
            .binary_search_with(atom.predicate(), || work.tick())?
            .is_ok()
            && !may.contains_atom(atom, work)?
        {
            return Ok(Pass::Refuted);
        }
    }
    let before = (cube.must.len(), cube.may.as_ref().map(CarrierSet::len));
    let after = (must.len(), Some(may.len()));
    work.tick()?;
    // Every must handle shares its tuple with may. A resource refusal above
    // drops only staged buffers; no prefix of this pass becomes a bound.
    *cube = Cube {
        must,
        may: Some(may),
    };
    Ok(if before == after {
        Pass::Fixed
    } else {
        Pass::Changed
    })
}

/// Completed lower/upper consequences, or a definite constraint refutation.
/// A stopped closure yields neither case and cannot commit a narrowed bound.
enum Enclosure {
    Refuted,
    Complete { lower: Model, upper: Model },
}

/// How the seeds are offered once the bounds are applied.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Enumeration {
    /// The binary counter over the carrier, pulled lazily.
    Carrier,
    /// The regions of the narrowed root, split until decided or counted.
    Regions,
}

/// The outcome of one narrowing pass, two closures.
#[derive(Debug, PartialEq, Eq)]
enum Pass {
    /// A bound moved; another pass may move it further.
    Changed,
    /// Neither bound moved: the region is the narrowing's fixed point.
    Fixed,
    /// A definite constraint fired: the region holds no accepted seed.
    Refuted,
}

/// Retained termination of the gate-seed enumerator, independent of membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateTermination {
    /// Every seed was returned or excluded by a certified necessary condition.
    Exhausted,
    /// Enumeration stopped before proving complete seed coverage.
    Stopped(Stop),
}

/// Enumerate empty, `{a}`, `{b}`, `{a,b}`, `{c}`, and so on. A new carrier
/// atom is requested only on carry beyond the known binary counter. Creation
/// and the first successful empty seed never request a carrier tuple.
/// Narrowed by the program's closures ([`Self::bounded`]), the counter runs
/// region by region over the gate atoms some seed could derive and not
/// every seed must hold; the others are omitted or held throughout, and the
/// narrowing's preparation and closure workspace are retained with the
/// iterator once the bounds are applied.
/// Each discovered tuple retains only shared integer coordinates and the one
/// Program vocabulary. Arc envelopes remain infallible under the carrier-count
/// bound; coordinate and selected-handle buffers use fallible reservation.
pub struct Candidates<'a> {
    program: &'a Program,
    carrier: GateAtoms<'a>,
    may: Option<CarrierSet>,
    must: Vec<CarrierAtom>,
    refuted: bool,
    atoms: Vec<Arc<GateAtom>>,
    bits: Vec<bool>,
    limits: CandidateLimits,
    cancellation: Cancellation,
    emitted: u64,
    started: bool,
    termination: Option<CandidateTermination>,
    restrictions: RestrictionState,
    narrowing: NarrowingState,
    enumeration: Enumeration,
    /// The open gate atoms of the narrowed root, in carrier order; a
    /// region decides over their indices.
    root: Vec<Arc<GateAtom>>,
    /// The gate atoms every seed holds, from the root's narrowing.
    root_must: CarrierSet,
    /// The coverage tree of the narrowed root, once the bounds are applied.
    traversal: Option<Traversal>,
    /// The counter is running inside a counted region.
    counting: bool,
    statistics: CandidateStatistics,
}

impl<'a> Candidates<'a> {
    /// Create an unstarted iterator without expanding the gate carrier.
    #[must_use]
    pub fn new(program: &'a Program, limits: CandidateLimits, cancellation: Cancellation) -> Self {
        Self {
            program,
            carrier: program.indexed_gate_atoms(),
            may: None,
            must: Vec::new(),
            refuted: false,
            atoms: Vec::new(),
            bits: Vec::new(),
            limits,
            cancellation,
            emitted: 0,
            started: false,
            termination: None,
            restrictions: RestrictionState::Disabled,
            narrowing: NarrowingState::Disabled,
            enumeration: Enumeration::Carrier,
            root: Vec::new(),
            root_must: CarrierSet::new(),
            traversal: None,
            counting: false,
            statistics: CandidateStatistics {
                shared_program_bytes: program.storage_bytes(),
                ..CandidateStatistics::default()
            },
        }
    }

    /// Enumerate a necessary superset of answer-set gate projections using
    /// positive constraints witnessed by actual unconditional facts. Creation
    /// does not expand the carrier; restriction preparation starts on first pull.
    ///
    /// A constraint is eligible when it has no negative gate and its nongate
    /// positive patterns bind every variable. The shared source visitor joins
    /// those patterns with unconditional facts; remaining positive gate atoms
    /// form forbidden conjunctions. Constraints outside this grammar remain for
    /// the complete closure oracle. No possible support is treated as a fact.
    ///
    /// A violated conjunction permits skipping the entire binary interval up to
    /// the next clearing of its least selected bit: all its premises stay true
    /// throughout that interval. Candidate limits count returned seeds; separate
    /// restriction limits bound preparation and skipped-interval checking. Every
    /// returned seed still needs original closure and gate-agreement checking.
    #[must_use]
    pub fn restricted(
        program: &'a Program,
        limits: CandidateLimits,
        restrictions: CandidateRestrictionLimits,
        cancellation: Cancellation,
    ) -> Self {
        let mut candidates = Self::new(program, limits, cancellation);
        candidates.restrictions = RestrictionState::Pending(restrictions);
        candidates
    }

    /// Narrow the carrier before the first pull. The program is prepared
    /// once, under `limits` as one preparation; then, starting from the
    /// region in which nothing is decided, each pass computes the region's
    /// two closures on that preparation in one retained workspace, each
    /// charged under `limits` as one candidate check would be, adds the
    /// lower one's gate atoms to those every seed must hold and cuts the
    /// gate atoms outside the upper one from those a seed may hold, until a
    /// pass changes neither; every accepted seed lies in every pass's region
    /// (`Bounds.narrowed_contains_accepted`). A constraint that fires in a
    /// lower closure refutes the region, and the counter offers no seed. A
    /// preparation or closure stopped by a resource ceiling keeps the bounds
    /// of the completed passes and is retained in the statistics;
    /// cancellation or a deadline stops the pull that met it.
    /// Descendants borrow those completed root owners and their indexed
    /// decisions for both pre-pass gate readings. Each completed pair is followed
    /// by a transfer with its own `max_work` allowance; coordinate and staged
    /// decision buffers are separately bounded by `max_closure_bytes`.
    /// A stopped transfer commits no part of that pass.
    pub fn bounded(&mut self, limits: Limits) {
        debug_assert!(!self.started, "the bound precedes the first pull");
        self.narrowing = NarrowingState::Pending(limits);
    }

    /// Accounted necessary-condition work and named storage through this pull.
    #[must_use]
    pub fn statistics(&self) -> CandidateStatistics {
        let mut statistics = self.statistics;
        let (coordinates, witnesses) = match self.enumeration {
            Enumeration::Regions => (
                self.root_must.coordinate_bytes()
                    + self
                        .root
                        .iter()
                        .map(|gate| gate.carrier().coordinate_bytes())
                        .sum::<u128>(),
                self.root.len(),
            ),
            Enumeration::Carrier => (
                self.may.as_ref().map_or_else(
                    || self.must.iter().map(CarrierAtom::coordinate_bytes).sum(),
                    CarrierSet::coordinate_bytes,
                ) + self
                    .atoms
                    .iter()
                    .map(|gate| gate.carrier().coordinate_bytes())
                    .sum::<u128>(),
                self.atoms.len(),
            ),
        };
        statistics.coordinate_bytes = coordinates;
        statistics.candidate_metadata_bytes = (self.root.capacity() + self.atoms.capacity())
            as u128
            * size_of::<Arc<GateAtom>>() as u128
            + self.must.capacity() as u128 * size_of::<CarrierAtom>() as u128
            + self.root_must.handle_bytes()
            + self.may.as_ref().map_or(0, CarrierSet::handle_bytes)
            + self.bits.capacity() as u128 * size_of::<bool>() as u128
            + witnesses as u128 * size_of::<GateAtom>() as u128;
        if let Some(traversal) = &self.traversal {
            let regions = traversal.statistics();
            statistics.regions = regions.regions;
            statistics.regions_refuted = regions.refuted;
            statistics.regions_leaves = regions.leaves;
            statistics.regions_counted = regions.counted;
        }
        statistics
    }

    /// Carrier atoms successfully retained so far: the narrowed root's
    /// open atoms once the bounds are applied, else the counter's.
    #[must_use]
    pub fn discovered_atoms(&self) -> usize {
        match self.enumeration {
            Enumeration::Regions => self.root.len(),
            Enumeration::Carrier => self.atoms.len(),
        }
    }

    /// Retained stopping outcome, even after the error item has been consumed.
    /// `None` means not yet terminated. Exhausted seed generation alone does not
    /// prove that membership checking completed, or that no stable model exists.
    #[must_use]
    pub const fn termination(&self) -> Option<CandidateTermination> {
        self.termination
    }

    /// Pull the next complete candidate while sharing discovered atom payloads.
    /// The selection owns its lifetime across later carries or iterator drop.
    /// This and the owned Iterator door share one sequence, emitted count and
    /// terminal state; mixing the doors never repeats a candidate.
    ///
    /// Construction allocates only selected handles, validates their carrier
    /// membership and canonicalizes them. It does not build the owned Seed tree.
    /// Errors are returned once and fuse both doors, including reservation,
    /// candidate/carrier/restriction limits, cancellation and deadlines.
    pub fn next_selection(&mut self) -> Option<Result<SeedSelection, Stop>> {
        self.pull(std::convert::identity)
    }

    fn selection(&mut self) -> Result<Option<SeedSelection>, Stop> {
        self.cancellation.poll()?;
        self.prepare_bounds()?;
        if self.refuted {
            return Ok(None);
        }
        self.prepare_restrictions()?;
        if self.enumeration == Enumeration::Regions {
            return self.region_selection();
        }
        if !self.counted_selection()? {
            return Ok(None);
        }
        self.selected()
    }

    /// Advance the counter to its next permitted seed; `false` when the
    /// counter's interval is exhausted or a restriction rules out the rest.
    fn counted_selection(&mut self) -> Result<bool, Stop> {
        if self.started {
            if !self.advance()? {
                return Ok(false);
            }
        } else {
            self.started = true;
        }
        self.seek_permitted()
    }

    /// The seeds of the narrowed root, region by region: a region is
    /// narrowed to its fixed point, refuted by a definite constraint,
    /// decided outright, split on its highest open atom with the out
    /// branch first, or counted when its narrowing decided nothing beyond
    /// the split. The leaves come in the counter's order, and every accepted
    /// seed of the root lies in exactly one region visited
    /// (`Search.CoverageTree.split`, `split_partition`).
    fn region_selection(&mut self) -> Result<Option<SeedSelection>, Stop> {
        loop {
            if self.counting {
                if self.counted_selection()? {
                    return self.selected();
                }
                self.counting = false;
            }
            // The root was narrowed when the bounds were applied, and the
            // traversal starts from it narrowed.
            let mut traversal = self
                .traversal
                .take()
                .expect("regions follow applied bounds");
            let visit = traversal.next(|region, ()| self.narrow_region(region));
            self.traversal = Some(traversal);
            match visit? {
                None => return Ok(None),
                Some(Visit::Leaf(region, ())) => {
                    self.hold(&region, &[])?;
                    return self.selected();
                }
                Some(Visit::Counted(region, ())) => {
                    let open: Vec<usize> = region.open().collect();
                    self.hold(&region, &open)?;
                    self.started = false;
                    self.counting = true;
                }
            }
        }
    }

    /// Hold the region's decided atoms in every seed and offer its open
    /// ones to the counter.
    fn hold(&mut self, region: &Region, open: &[usize]) -> Result<(), Stop> {
        self.must.clear();
        self.atoms.clear();
        self.bits.clear();
        let count = self
            .root_must
            .len()
            .checked_add(region.held().count())
            .ok_or(Stop::Allocation)?;
        let mut held = Vec::new();
        held.try_reserve_exact(count)
            .map_err(|_| Stop::Allocation)?;
        held.extend(self.root_must.iter().cloned());
        for index in region.held() {
            held.push(self.root[index].carrier());
        }
        held.sort_unstable();
        self.must
            .try_reserve(held.len())
            .map_err(|_| Stop::Allocation)?;
        for atom in held {
            self.cancellation.poll()?;
            self.must.push(atom);
        }
        self.atoms
            .try_reserve(open.len())
            .map_err(|_| Stop::Allocation)?;
        self.bits
            .try_reserve(open.len())
            .map_err(|_| Stop::Allocation)?;
        for &index in open {
            self.atoms.push(self.root[index].clone());
            self.bits.push(false);
        }
        Ok(())
    }

    /// Narrow a region below the root to its fixed point, reading each pass's
    /// decisions back into it. A closure stopped on a resource ceiling keeps
    /// the completed passes' decisions and reports no change, so the region
    /// is counted with them; cancellation or a deadline stops the pull.
    fn narrow_region(&mut self, region: &mut Region) -> Result<Narrowing, Stop> {
        let mut changed = false;
        loop {
            let NarrowingState::Applied(closures) = &mut self.narrowing else {
                return Ok(Narrowing::Fixed { changed: false });
            };
            let result = closures.narrow_region(&self.root_must, &self.root, region);
            self.statistics.narrowing_transfer_work = closures.transfer_work;
            self.statistics.narrowing_transfer_peak_bytes = closures.transfer_peak_bytes;
            match result {
                Ok(Pass::Changed) => {
                    self.statistics.region_passes += 1;
                    changed = true;
                }
                Ok(Pass::Fixed) => {
                    self.statistics.region_passes += 1;
                    return Ok(Narrowing::Fixed { changed });
                }
                Ok(Pass::Refuted) => return Ok(Narrowing::Refuted),
                Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
                Err(stop) => {
                    self.statistics.narrowing_stop.get_or_insert(stop);
                    return Ok(Narrowing::Fixed { changed: false });
                }
            }
        }
    }

    /// The selection of the held atoms and the counter's selected ones.
    fn selected(&mut self) -> Result<Option<SeedSelection>, Stop> {
        if self.emitted >= self.limits.max_candidates {
            return Err(Stop::CandidateLimit);
        }
        let seed = SeedSelection::held_and_selected(
            self.program,
            self.must.iter().cloned(),
            self.atoms
                .iter()
                .zip(&self.bits)
                .filter(|(_, selected)| **selected)
                .map(|(atom, _)| atom.clone()),
        )
        .map_err(|error| match error {
            SeedSelectionError::Allocation => Stop::Allocation,
            SeedSelectionError::OutsideCarrier { .. }
            | SeedSelectionError::OutsideGateCarrier { .. }
            | SeedSelectionError::WrongProgram => Stop::InvalidProgram,
        })?;
        Ok(Some(seed))
    }

    fn pull<T>(&mut self, materialize: impl FnOnce(SeedSelection) -> T) -> Option<Result<T, Stop>> {
        if self.termination.is_some() {
            return None;
        }
        let result = self.selection().and_then(|selection| {
            let Some(selection) = selection else {
                return Ok(None);
            };
            let candidate = materialize(selection);
            self.cancellation.poll()?;
            self.emitted += 1;
            Ok(Some(candidate))
        });
        match result {
            Ok(Some(candidate)) => Some(Ok(candidate)),
            Ok(None) => {
                self.termination = Some(CandidateTermination::Exhausted);
                None
            }
            Err(error) => {
                self.termination = Some(CandidateTermination::Stopped(error));
                Some(Err(error))
            }
        }
    }

    fn prepare_bounds(&mut self) -> Result<(), Stop> {
        let NarrowingState::Pending(limits) = &self.narrowing else {
            return Ok(());
        };
        let limits = *limits;
        // An empty carrier has nothing to bound; the one seed costs no closure.
        if self.program.gate_predicates().is_empty() {
            self.narrowing = NarrowingState::Trivial;
            return Ok(());
        }
        let mut closures = match Closures::new(self.program, limits, self.cancellation.clone()) {
            Ok(closures) => closures,
            Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
            Err(stop) => {
                self.statistics.narrowing_stop = Some(stop);
                self.narrowing = NarrowingState::Unavailable;
                return Ok(());
            }
        };
        let mut cube = Cube::all_open();
        // Each pass either refutes, strictly grows `must`, strictly shrinks
        // `may` or is the last; both sets lie within the finite gate atoms
        // of the first upper closure, so the loop ends.
        self.narrowing = loop {
            let result = closures.narrow(&mut cube);
            self.statistics.narrowing_transfer_work = closures.transfer_work;
            self.statistics.narrowing_transfer_peak_bytes = closures.transfer_peak_bytes;
            match result {
                Ok(Pass::Changed) => self.statistics.narrowing_passes += 1,
                Ok(Pass::Fixed) => {
                    self.statistics.narrowing_passes += 1;
                    break NarrowingState::Applied(Box::new(closures));
                }
                Ok(Pass::Refuted) => {
                    self.refuted = true;
                    self.statistics.root_refuted = true;
                    break NarrowingState::Applied(Box::new(closures));
                }
                Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
                Err(stop) => {
                    self.statistics.narrowing_stop = Some(stop);
                    break NarrowingState::Unavailable;
                }
            }
        };
        for atom in cube.must.iter() {
            self.must.try_reserve(1).map_err(|_| Stop::Allocation)?;
            self.must.push(atom.clone());
        }
        self.statistics.held_gate_atoms = self.must.len();
        if self.refuted {
            return Ok(());
        }
        let applied = matches!(self.narrowing, NarrowingState::Applied(_));
        match (applied, cube.may) {
            // Completed may bounds contain every accepted seed; only their
            // open atoms become region coordinates. The original program and
            // semantic gate carrier remain unchanged (Bounds.undecided_bounds_accepted).
            (true, Some(may)) => {
                self.root_must = cube.must;
                self.materialize_root(may)?;
                self.traversal = Some(Traversal::with_narrowed_root(
                    Region::all_open(self.root.len()),
                    Counting::Unchanged,
                    (),
                ));
                self.enumeration = Enumeration::Regions;
            }
            (_, may) => self.may = may,
        }
        Ok(())
    }

    /// Retain completed supported gate atoms outside the held set directly.
    /// `may` is already canonical and confined to this program's gate carrier.
    /// The temporary symbolic index has one fallibly reserved offset per
    /// admitted gate signature; token payloads move from `may` without copies.
    /// Work depends on signatures and supported atoms, not Cartesian tuples.
    /// Existing carrier-position overflow and open-atom count limits still apply.
    fn materialize_root(&mut self, may: CarrierSet) -> Result<(), Stop> {
        self.cancellation.poll()?;
        let index = GateIndex::new(self.program).map_err(gate_index_stop)?;
        self.statistics.cut_gate_atoms = index
            .len()
            .checked_sub(may.len())
            .ok_or(Stop::InvalidProgram)?;
        let NarrowingState::Applied(closures) = &mut self.narrowing else {
            return Err(Stop::InvalidProgram);
        };
        let mut work = Work::source(&self.cancellation, closures.limits.max_work);
        let result = (|| {
            let mut root = Vec::new();
            for atom in may.into_atoms() {
                work.tick()?;
                if !self.root_must.contains_atom(atom.atom(), &mut work)? {
                    if root.len() >= self.limits.max_carrier_atoms {
                        return Err(Stop::CarrierLimit);
                    }
                    if root.len() == root.capacity() {
                        for _ in 0..root.len().max(1) {
                            work.tick()?;
                        }
                    }
                    root.try_reserve(1).map_err(|_| Stop::Allocation)?;
                    let gate =
                        index
                            .locate_carrier_with(atom, || work.tick())
                            .map_err(|failure| match failure {
                                GateIndexFailure::Index(error) => gate_index_stop(error),
                                GateIndexFailure::Stopped(stop) => stop,
                            })?;
                    work.tick()?;
                    root.push(Arc::new(gate));
                }
            }
            Ok(root)
        })();
        closures.transfer_work = closures
            .transfer_work
            .saturating_add(work.source_statistics(0).work);
        self.statistics.narrowing_transfer_work = closures.transfer_work;
        self.root = result?;
        Ok(())
    }

    fn prepare_restrictions(&mut self) -> Result<(), Stop> {
        if let RestrictionState::Pending(limits) = self.restrictions {
            let attempt = Restrictions::compile(self.program, limits, &self.cancellation);
            self.statistics.restriction_work = attempt.work;
            let plan = attempt.result?;
            self.statistics.restriction_atoms = plan.atoms;
            self.statistics.restriction_bytes = plan.bytes;
            self.statistics.restriction_peak_bytes = plan.peak_bytes;
            self.statistics.restriction_conjunctions = plan.conjunctions();
            self.restrictions = RestrictionState::Prepared { limits, plan };
        }
        Ok(())
    }

    fn seek_permitted(&mut self) -> Result<bool, Stop> {
        loop {
            let RestrictionState::Prepared { limits, plan } = &self.restrictions else {
                return Ok(true);
            };
            let remaining = limits.max_work - self.statistics.restriction_work;
            let (result, work) = plan.conflict(
                &self.must,
                &self.atoms,
                &self.bits,
                remaining,
                &self.cancellation,
            );
            self.statistics.restriction_work += work;
            let Some(conflict) = result? else {
                return Ok(true);
            };
            // Each conflict consumed at least one bounded work operation.
            self.statistics.conflicts += 1;
            match conflict {
                Conflict::Unconditional => return Ok(false),
                Conflict::Selected(first) => {
                    for bit in &mut self.bits[..=first] {
                        self.cancellation.poll()?;
                        *bit = false;
                    }
                    if !self.advance_from(first + 1)? {
                        return Ok(false);
                    }
                }
            }
        }
    }

    fn advance(&mut self) -> Result<bool, Stop> {
        self.advance_from(0)
    }

    fn advance_from(&mut self, carry: usize) -> Result<bool, Stop> {
        for bit in &mut self.bits[carry..] {
            self.cancellation.poll()?;
            if !*bit {
                *bit = true;
                return Ok(true);
            }
            *bit = false;
        }
        // A counted region supplies its complete open-atom interval through
        // hold(). Carry beyond that interval returns to the region traversal;
        // it must never discover atoms from the independent symbolic carrier.
        // Direct supported-root construction deliberately leaves that cursor
        // untouched, so cursor exhaustion cannot represent this boundary.
        if self.enumeration == Enumeration::Regions {
            return Ok(false);
        }
        let atom = loop {
            self.cancellation.poll()?;
            let Some(atom) = self.carrier.next() else {
                return Ok(false);
            };
            let atom = atom.map_err(|error| match error {
                GateAtomError::Carrier(_) => Stop::Allocation,
                GateAtomError::OrdinalOverflow => Stop::CarrierLimit,
            })?;
            if self
                .may
                .as_ref()
                .is_some_and(|may| !may.iter().any(|possible| possible.atom() == atom.atom()))
            {
                self.statistics.cut_gate_atoms += 1;
            } else if self
                .must
                .binary_search_by(|necessary| necessary.atom().cmp(&atom.atom()))
                .is_err()
            {
                break atom;
            }
        };
        if self.atoms.len() >= self.limits.max_carrier_atoms {
            return Err(Stop::CarrierLimit);
        }
        self.atoms.try_reserve(1).map_err(|_| Stop::Allocation)?;
        self.bits.try_reserve(1).map_err(|_| Stop::Allocation)?;
        self.atoms.push(Arc::new(atom));
        self.bits.push(true);
        Ok(true)
    }
}

fn gate_index_stop(error: GateIndexError) -> Stop {
    match error {
        GateIndexError::Allocation => Stop::Allocation,
        GateIndexError::OutsideCarrier => Stop::InvalidProgram,
        GateIndexError::OrdinalOverflow => Stop::CarrierLimit,
    }
}

impl Iterator for Candidates<'_> {
    type Item = Result<Seed, Stop>;

    fn next(&mut self) -> Option<Self::Item> {
        self.pull(|selection| selection.to_seed())
    }
}

impl FusedIterator for Candidates<'_> {}

#[cfg(test)]
#[path = "../tests/support/selection_cursor.rs"]
mod selection_tests;
#[cfg(test)]
#[path = "../tests/support/narrowing_closures.rs"]
mod narrowing_tests;
#[cfg(test)]
#[path = "../tests/support/supported_carrier.rs"]
mod supported_carrier_tests;
#[cfg(test)]
#[path = "../tests/support/region_bound_closures.rs"]
mod region_bound_tests;
