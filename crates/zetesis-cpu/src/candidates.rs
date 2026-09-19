//! Incremental powerset enumeration whose first seed needs no carrier tuple.

use std::iter::FusedIterator;
use std::sync::Arc;
use zetesis_core::{
    Atom, GateAtom, GateAtomError, GateAtoms, Program, Seed, SeedSelection, SeedSelectionError,
};

use crate::oracle::restrictions::{Conflict, Restrictions};
use crate::oracle::{
    ClosureWorkspace, Cube, Limits, PreparationLimits, PreparedQueries, definite_closure,
    possible_closure,
};
use crate::regions::{Counting, Narrowing, Region, Traversal, Visit};
use crate::{Control, Stop};
use std::collections::BTreeSet;

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
    /// Source joins, checked copies and premise comparisons across all seeds.
    /// Fact canonicalization uses [`zetesis_core::Model::new`]: its `O(n log n)`
    /// comparisons and catalog-size traversal are unmetered; vector/Arc allocation
    /// is infallible. Its copied
    /// inputs remain bounded by `max_atoms` and `max_bytes`.
    pub max_work: u64,
    /// Fact and forbidden-conjunction atom occurrences copied during preparation.
    pub max_atoms: usize,
    /// Logical copied atom, template and value payload during preparation.
    /// Allocator slack, tree/index metadata and caller-owned source are excluded.
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

/// Work performed by the optional necessary-condition filter. The original
/// powerset constructor leaves all fields zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateStatistics {
    /// Charged construction and traversal operations, retained after interruption.
    /// Fact canonicalization's comparisons and allocation are not included.
    pub restriction_work: u64,
    /// Fact and forbidden-conjunction atom occurrences in completed preparation.
    /// Zero when preparation failed; `restriction_work` retains failed work.
    pub restriction_atoms: usize,
    /// Copied fact and conjunction payload in completed preparation.
    /// Temporary templates and allocator/index overhead are not included.
    pub restriction_bytes: usize,
    /// Peak copied payload during completed preparation, including its temporary
    /// template. Zero when preparation failed.
    pub restriction_peak_bytes: usize,
    /// Positive gate conjunctions obtained from covered fact-side bindings.
    pub restriction_conjunctions: usize,
    /// Certified impossible binary intervals skipped, not individual seeds.
    pub conflicts: u64,
    /// Gate atoms the narrowing cut, outside its upper closure and never
    /// offered to the counter: no rule derives them under any gate
    /// assumption, so no answer set holds them.
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
    control: Control,
}

impl Closures {
    /// Prepare `program` under `limits`.
    ///
    /// # Errors
    /// Returns the preparation's stop: cancellation, a deadline, or its work
    /// or storage ceiling.
    fn new(program: &Program, limits: Limits, control: Control) -> Result<Self, Stop> {
        let prepared = PreparedQueries::new(
            program,
            PreparationLimits {
                max_work: limits.max_work,
                max_bytes: limits.max_closure_bytes,
                max_dense_atoms: PreparationLimits::default().max_dense_atoms,
            },
            &control,
        )?;
        Ok(Self {
            prepared,
            workspace: ClosureWorkspace::default(),
            limits,
            control,
        })
    }

    /// One narrowing pass over `cube`: its lower closure, then its upper one.
    fn narrow(&mut self, cube: &mut Cube) -> Result<Pass, Stop> {
        let lower = definite_closure(
            &self.prepared,
            &mut self.workspace,
            cube,
            self.limits,
            &self.control,
        )?;
        if lower.constraint_violated {
            return Ok(Pass::Refuted);
        }
        let upper = possible_closure(
            &self.prepared,
            &mut self.workspace,
            cube,
            self.limits,
            &self.control,
        )?;
        let program = self.prepared.program();
        let before = (cube.must.len(), cube.may.as_ref().map(BTreeSet::len));
        for atom in lower.atoms.atoms() {
            if program.contains_gate_atom(atom) {
                cube.must.insert(atom.clone());
            }
        }
        let derivable: BTreeSet<Atom> = upper
            .atoms
            .atoms()
            .iter()
            .filter(|atom| program.contains_gate_atom(atom))
            .cloned()
            .collect();
        cube.may = Some(match cube.may.take() {
            None => derivable,
            Some(may) => may.intersection(&derivable).cloned().collect(),
        });
        // A gate atom every seed of the region holds that no seed of it can
        // derive: the region holds no accepted seed
        // (`Bounds.conflicting_atom_refutes`). From the open cube the
        // lower closure lies inside the upper one; a split can part them.
        if cube
            .must
            .iter()
            .any(|atom| cube.may.as_ref().is_some_and(|may| !may.contains(atom)))
        {
            return Ok(Pass::Refuted);
        }
        let after = (cube.must.len(), cube.may.as_ref().map(BTreeSet::len));
        Ok(if after == before {
            Pass::Fixed
        } else {
            Pass::Changed
        })
    }
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
/// Each discovered atom payload is retained once in an immutable shared owner.
/// Creating that Arc uses infallible allocation under the carrier-count bound;
/// carrier and selected-handle vectors use typed fallible reservation.
pub struct Candidates<'a> {
    program: &'a Program,
    carrier: GateAtoms<'a>,
    may: Option<BTreeSet<Atom>>,
    must: Vec<Arc<Atom>>,
    refuted: bool,
    atoms: Vec<Arc<GateAtom>>,
    bits: Vec<bool>,
    limits: CandidateLimits,
    control: Control,
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
    root_must: BTreeSet<Atom>,
    /// The coverage tree of the narrowed root, once the bounds are applied.
    traversal: Option<Traversal>,
    /// The counter is running inside a counted region.
    counting: bool,
    statistics: CandidateStatistics,
}

impl<'a> Candidates<'a> {
    /// Create an unstarted iterator without expanding the gate carrier.
    #[must_use]
    pub fn new(program: &'a Program, limits: CandidateLimits, control: Control) -> Self {
        Self {
            program,
            carrier: program.indexed_gate_atoms(),
            may: None,
            must: Vec::new(),
            refuted: false,
            atoms: Vec::new(),
            bits: Vec::new(),
            limits,
            control,
            emitted: 0,
            started: false,
            termination: None,
            restrictions: RestrictionState::Disabled,
            narrowing: NarrowingState::Disabled,
            enumeration: Enumeration::Carrier,
            root: Vec::new(),
            root_must: BTreeSet::new(),
            traversal: None,
            counting: false,
            statistics: CandidateStatistics::default(),
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
        control: Control,
    ) -> Self {
        let mut candidates = Self::new(program, limits, control);
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
    pub fn bounded(&mut self, limits: Limits) {
        debug_assert!(!self.started, "the bound precedes the first pull");
        self.narrowing = NarrowingState::Pending(limits);
    }

    /// Accounted necessary-condition work and copied payload through this pull.
    #[must_use]
    pub fn statistics(&self) -> CandidateStatistics {
        let mut statistics = self.statistics;
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
        self.control.poll()?;
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
        let mut held: BTreeSet<&Atom> = self.root_must.iter().collect();
        for index in region.held() {
            held.insert(self.root[index].atom());
        }
        self.must
            .try_reserve(held.len())
            .map_err(|_| Stop::Allocation)?;
        for atom in held {
            self.control.poll()?;
            self.must.push(Arc::new(atom.clone()));
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
        let mut cube = self.cube_of(region);
        let mut changed = false;
        loop {
            let NarrowingState::Applied(closures) = &mut self.narrowing else {
                return Ok(Narrowing::Fixed { changed: false });
            };
            match closures.narrow(&mut cube) {
                Ok(Pass::Changed) => {
                    self.statistics.region_passes += 1;
                    changed |= self.decide(region, &cube);
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

    /// The region as a cube over the root's atoms: its held atoms with the
    /// root's, and every atom not cut out.
    fn cube_of(&self, region: &Region) -> Cube {
        let mut must = self.root_must.clone();
        let mut may = self.root_must.clone();
        for (index, gate) in self.root.iter().enumerate() {
            let atom = gate.atom();
            match region.decision(index) {
                Some(true) => {
                    must.insert(atom.clone());
                    may.insert(atom.clone());
                }
                Some(false) => {}
                None => {
                    may.insert(atom.clone());
                }
            }
        }
        Cube {
            must,
            may: Some(may),
        }
    }

    /// Read a narrowed cube's decisions into the region; whether one was new.
    fn decide(&self, region: &mut Region, cube: &Cube) -> bool {
        let mut changed = false;
        for (index, gate) in self.root.iter().enumerate() {
            if !region.is_open(index) {
                continue;
            }
            let atom = gate.atom();
            if cube.must.contains(atom) {
                region.hold(index);
                changed = true;
            } else if cube.may.as_ref().is_some_and(|may| !may.contains(atom)) {
                region.cut(index);
                changed = true;
            }
        }
        changed
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
            SeedSelectionError::OutsideCarrier { .. } | SeedSelectionError::WrongProgram => {
                Stop::InvalidProgram
            }
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
            self.control.poll()?;
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
        let mut closures = match Closures::new(self.program, limits, self.control.clone()) {
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
            match closures.narrow(&mut cube) {
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
        for atom in &cube.must {
            self.must.try_reserve(1).map_err(|_| Stop::Allocation)?;
            self.must.push(Arc::new(atom.clone()));
        }
        self.statistics.held_gate_atoms = self.must.len();
        if self.refuted {
            return Ok(());
        }
        let applied = matches!(self.narrowing, NarrowingState::Applied(_));
        match (applied, cube.may) {
            // The narrowed root's open atoms, in carrier order, are the
            // region tree's coordinates; the carrier is read once for them.
            (true, Some(may)) => {
                self.root_must = cube.must;
                self.materialize_root(&may)?;
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

    /// Retain the carrier's gate atoms inside `may` and outside the held set.
    fn materialize_root(&mut self, may: &BTreeSet<Atom>) -> Result<(), Stop> {
        for atom in self.carrier.by_ref() {
            self.control.poll()?;
            let atom = atom.map_err(|error| match error {
                GateAtomError::Carrier(_) => Stop::Allocation,
                GateAtomError::OrdinalOverflow => Stop::CarrierLimit,
            })?;
            if !may.contains(atom.atom()) {
                self.statistics.cut_gate_atoms += 1;
            } else if !self.root_must.contains(atom.atom()) {
                if self.root.len() >= self.limits.max_carrier_atoms {
                    return Err(Stop::CarrierLimit);
                }
                self.root.try_reserve(1).map_err(|_| Stop::Allocation)?;
                self.root.push(Arc::new(atom));
            }
        }
        Ok(())
    }

    fn prepare_restrictions(&mut self) -> Result<(), Stop> {
        if let RestrictionState::Pending(limits) = self.restrictions {
            let attempt = Restrictions::compile(self.program, limits, &self.control);
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
                &self.control,
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
                        self.control.poll()?;
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
            self.control.poll()?;
            if !*bit {
                *bit = true;
                return Ok(true);
            }
            *bit = false;
        }
        let atom = loop {
            self.control.poll()?;
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
                .is_some_and(|may| !may.contains(atom.atom()))
            {
                self.statistics.cut_gate_atoms += 1;
            } else if self
                .must
                .binary_search_by(|necessary| necessary.as_ref().cmp(atom.atom()))
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
