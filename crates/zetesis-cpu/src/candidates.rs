//! Incremental powerset enumeration whose first seed needs no carrier tuple.

use std::iter::FusedIterator;
use std::sync::Arc;
use zetesis_core::{
    Atom, GateAtom, GateAtomError, GateAtoms, Model, Program, Seed, SeedSelection,
    SeedSelectionError,
};

use crate::oracle::restrictions::{Conflict, Restrictions};
use crate::oracle::{Cube, Limits, definite_closure, possible_closure};
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
    /// Symbolic gate atoms outside the supplied upper closure, never offered to
    /// the counter: no rule derives them under any gate assumption, so no
    /// answer set holds them.
    pub underivable_gate_atoms: usize,
    /// Gate atoms of the supplied lower closure, held in every seed instead of
    /// counted: gate-free rules derive them, so every answer set holds them.
    pub necessary_gate_atoms: usize,
    /// Why the narrowing stopped early: a closure stopped on a resource
    /// ceiling, and the counter kept the bounds of the passes that completed,
    /// the whole symbolic carrier when none had. `None` when no bound was
    /// requested or the narrowing reached its fixed point.
    pub bounds_stop: Option<Stop>,
    /// Completed narrowing passes, each two closures; the last one changed
    /// neither bound, unless a stop or a refutation ended the narrowing.
    pub bounds_passes: usize,
    /// A constraint fired in the lower closure of the narrowed region, so no
    /// seed of it is accepted and the counter offered none
    /// (`Bounds.lower_constraint_refutes`).
    pub bounds_refuted: bool,
    /// Regions the split visited, the narrowed root included.
    pub regions: usize,
    /// Regions a definite constraint refuted in their lower closure: no seed
    /// of them was offered.
    pub regions_refuted: usize,
    /// Regions whose narrowing decided every gate atom: one seed each,
    /// offered without a count.
    pub regions_decided: usize,
    /// Regions counted as a flat interval, because their narrowing decided
    /// nothing beyond the split that formed them or a closure stopped inside
    /// them; the count's restrictions still apply within.
    pub regions_counted: usize,
    /// Completed narrowing passes over the regions below the root, each two
    /// closures.
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

/// Whether the program's closures narrow the carrier, and how far that got.
#[derive(Clone, Copy)]
enum BoundsState {
    Disabled,
    Pending(Limits),
    /// The root is narrowed; its regions are narrowed under the same limits.
    Applied(Limits),
    Unavailable,
}

/// How the seeds are offered once the bounds are applied.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Enumeration {
    /// The binary counter over the carrier, pulled lazily.
    Carrier,
    /// The regions of the narrowed root, split until decided or counted.
    Regions,
}

/// A region of the narrowed root: every root atom is held in, cut out, or
/// undecided. Its seeds are the root's seeds that agree with its decisions;
/// this is the `Cube` of `Search.lean` over the root's undecided atoms.
struct Region {
    decided: Vec<Option<bool>>,
    /// The narrowed root itself, which is always split rather than counted.
    root: bool,
}

/// The outcome of narrowing one region below the root.
enum RegionNarrowing {
    /// A definite constraint fired: no seed of the region is accepted.
    Refuted,
    /// The fixed point, and whether it decided an atom beyond the split.
    Fixed { changed: bool },
}

/// The outcome of one narrowing pass.
enum Narrowing {
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
/// and the first successful empty seed never request a carrier tuple. Between
/// the program's two closures ([`Self::within`], [`Self::requiring`]) the
/// counter runs over the gate atoms some seed could derive and not every
/// seed must hold; the others are omitted or held throughout.
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
    bounds: BoundsState,
    enumeration: Enumeration,
    /// The undecided gate atoms of the narrowed root, in carrier order; a
    /// region decides over their indices.
    root: Vec<Arc<GateAtom>>,
    /// The gate atoms every seed holds, from the root's narrowing.
    root_must: BTreeSet<Atom>,
    /// The regions still to visit, the next on top.
    regions: Vec<Region>,
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
            bounds: BoundsState::Disabled,
            enumeration: Enumeration::Carrier,
            root: Vec::new(),
            root_must: BTreeSet::new(),
            regions: Vec::new(),
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

    /// Narrow the carrier before the first pull. Starting from the region in
    /// which nothing is decided, each pass computes the region's two closures
    /// under `limits`, as one candidate check would be, adds the lower one's
    /// gate atoms to those every seed must hold and cuts the gate atoms
    /// outside the upper one from those a seed may hold, until a pass changes
    /// neither; every accepted seed lies in every pass's region
    /// (`Bounds.narrowed_contains_accepted`). A constraint that fires in a
    /// lower closure refutes the region, and the counter offers no seed. A
    /// closure stopped by a resource ceiling keeps the bounds of the completed
    /// passes and is retained in the statistics; cancellation or a deadline
    /// stops the pull that met it.
    pub fn bounded(&mut self, limits: Limits) {
        debug_assert!(!self.started, "the bound precedes the first pull");
        self.bounds = BoundsState::Pending(limits);
    }

    /// Offer the counter only the gate atoms inside `may`, the program's
    /// upper closure from [`crate::upper_closure`]. A gate atom outside it
    /// belongs to no answer set, so the returned seeds remain a necessary
    /// superset of answer-set gate projections while the carrier shrinks from
    /// the symbolic gate atoms to the derivable ones. The bound precedes the
    /// first pull; carrier positions keep their symbolic order.
    pub fn within(&mut self, may: &Model) {
        debug_assert!(!self.started, "the bound precedes the first pull");
        self.may = Some(may.atoms().iter().cloned().collect());
    }

    /// Hold the gate atoms of `must`, the program's lower closure from
    /// [`crate::lower_closure`], in every seed instead of counting them.
    /// Every answer set holds them, so no seed without them is accepted and
    /// the counter loses no answer set by never clearing them. The bound
    /// precedes the first pull and lies inside any upper bound given.
    ///
    /// # Errors
    /// Returns [`Stop::Allocation`] when the necessary atoms cannot be retained.
    pub fn requiring(&mut self, must: &Model) -> Result<(), Stop> {
        debug_assert!(!self.started, "the bound precedes the first pull");
        for atom in must.atoms() {
            if self.program.contains_gate_atom(atom) {
                self.must.try_reserve(1).map_err(|_| Stop::Allocation)?;
                self.must.push(Arc::new(atom.clone()));
            }
        }
        self.statistics.necessary_gate_atoms = self.must.len();
        Ok(())
    }

    /// Accounted necessary-condition work and copied payload through this pull.
    #[must_use]
    pub const fn statistics(&self) -> CandidateStatistics {
        self.statistics
    }

    /// Carrier atoms successfully retained so far: the narrowed root's
    /// undecided atoms once the bounds are applied, else the counter's.
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
    /// decided outright, split on its highest undecided atom with the out
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
            let Some(mut region) = self.regions.pop() else {
                return Ok(None);
            };
            self.statistics.regions += 1;
            let changed = match self.narrow_region(&mut region)? {
                RegionNarrowing::Refuted => {
                    self.statistics.regions_refuted += 1;
                    continue;
                }
                RegionNarrowing::Fixed { changed } => changed,
            };
            let Some(last) = region.decided.iter().rposition(Option::is_none) else {
                self.statistics.regions_decided += 1;
                self.hold(&region, &[])?;
                return self.selected();
            };
            if region.root || changed {
                let mut held = Region {
                    decided: Vec::new(),
                    root: false,
                };
                held.decided
                    .try_reserve_exact(region.decided.len())
                    .map_err(|_| Stop::Allocation)?;
                held.decided.extend_from_slice(&region.decided);
                held.decided[last] = Some(true);
                let mut cut = region;
                cut.root = false;
                cut.decided[last] = Some(false);
                self.regions.try_reserve(2).map_err(|_| Stop::Allocation)?;
                self.regions.push(held);
                self.regions.push(cut);
            } else {
                self.statistics.regions_counted += 1;
                let undecided: Vec<usize> = region
                    .decided
                    .iter()
                    .enumerate()
                    .filter(|(_, decision)| decision.is_none())
                    .map(|(index, _)| index)
                    .collect();
                self.hold(&region, &undecided)?;
                self.started = false;
                self.counting = true;
            }
        }
    }

    /// Hold the region's decided atoms in every seed and offer its undecided
    /// ones to the counter.
    fn hold(&mut self, region: &Region, undecided: &[usize]) -> Result<(), Stop> {
        self.must.clear();
        self.atoms.clear();
        self.bits.clear();
        let mut held: BTreeSet<&Atom> = self.root_must.iter().collect();
        for (index, decision) in region.decided.iter().enumerate() {
            if *decision == Some(true) {
                held.insert(self.root[index].atom());
            }
        }
        self.must
            .try_reserve(held.len())
            .map_err(|_| Stop::Allocation)?;
        for atom in held {
            self.control.poll()?;
            self.must.push(Arc::new(atom.clone()));
        }
        self.atoms
            .try_reserve(undecided.len())
            .map_err(|_| Stop::Allocation)?;
        self.bits
            .try_reserve(undecided.len())
            .map_err(|_| Stop::Allocation)?;
        for &index in undecided {
            self.atoms.push(self.root[index].clone());
            self.bits.push(false);
        }
        Ok(())
    }

    /// Narrow a region below the root to its fixed point, reading each pass's
    /// decisions back into it. A closure stopped on a resource ceiling keeps
    /// the completed passes' decisions and reports no change, so the region
    /// is counted with them; cancellation or a deadline stops the pull.
    fn narrow_region(&mut self, region: &mut Region) -> Result<RegionNarrowing, Stop> {
        if region.root {
            return Ok(RegionNarrowing::Fixed { changed: true });
        }
        let BoundsState::Applied(limits) = self.bounds else {
            return Ok(RegionNarrowing::Fixed { changed: false });
        };
        let mut cube = self.cube_of(region);
        let mut changed = false;
        loop {
            match self.narrow(&mut cube, limits) {
                Ok(Narrowing::Changed) => {
                    self.statistics.region_passes += 1;
                    changed |= self.decide(region, &cube);
                }
                Ok(Narrowing::Fixed) => {
                    self.statistics.region_passes += 1;
                    return Ok(RegionNarrowing::Fixed { changed });
                }
                Ok(Narrowing::Refuted) => return Ok(RegionNarrowing::Refuted),
                Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
                Err(stop) => {
                    self.statistics.bounds_stop.get_or_insert(stop);
                    return Ok(RegionNarrowing::Fixed { changed: false });
                }
            }
        }
    }

    /// The region as a cube over the root's atoms: its held atoms with the
    /// root's, and every atom not cut out.
    fn cube_of(&self, region: &Region) -> Cube {
        let mut must = self.root_must.clone();
        let mut may = self.root_must.clone();
        for (index, decision) in region.decided.iter().enumerate() {
            let atom = self.root[index].atom();
            match decision {
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
        for (index, decision) in region.decided.iter_mut().enumerate() {
            if decision.is_some() {
                continue;
            }
            let atom = self.root[index].atom();
            if cube.must.contains(atom) {
                *decision = Some(true);
                changed = true;
            } else if cube.may.as_ref().is_some_and(|may| !may.contains(atom)) {
                *decision = Some(false);
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
        let seed = SeedSelection::necessary_and_selected(
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
        let BoundsState::Pending(limits) = self.bounds else {
            return Ok(());
        };
        // An empty carrier has nothing to bound; the one seed costs no closure.
        if self.program.gate_predicates().is_empty() {
            self.bounds = BoundsState::Applied(limits);
            return Ok(());
        }
        let mut cube = Cube::undecided();
        // Each pass either refutes, strictly grows `must`, strictly shrinks
        // `may` or is the last; both sets lie within the finite gate atoms
        // of the first upper closure, so the loop ends.
        self.bounds = loop {
            match self.narrow(&mut cube, limits) {
                Ok(Narrowing::Changed) => self.statistics.bounds_passes += 1,
                Ok(Narrowing::Fixed) => {
                    self.statistics.bounds_passes += 1;
                    break BoundsState::Applied(limits);
                }
                Ok(Narrowing::Refuted) => {
                    self.refuted = true;
                    self.statistics.bounds_refuted = true;
                    break BoundsState::Applied(limits);
                }
                Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
                Err(stop) => {
                    self.statistics.bounds_stop = Some(stop);
                    break BoundsState::Unavailable;
                }
            }
        };
        for atom in &cube.must {
            self.must.try_reserve(1).map_err(|_| Stop::Allocation)?;
            self.must.push(Arc::new(atom.clone()));
        }
        self.statistics.necessary_gate_atoms = self.must.len();
        if self.refuted {
            return Ok(());
        }
        match (self.bounds, cube.may) {
            // The narrowed root's undecided atoms, in carrier order, are the
            // region tree's coordinates; the carrier is read once for them.
            (BoundsState::Applied(_), Some(may)) => {
                self.root_must = cube.must;
                self.materialize_root(&may)?;
                self.regions.push(Region {
                    decided: vec![None; self.root.len()],
                    root: true,
                });
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
                self.statistics.underivable_gate_atoms += 1;
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

    /// One narrowing pass over `cube`, counted when both closures complete.
    fn narrow(&mut self, cube: &mut Cube, limits: Limits) -> Result<Narrowing, Stop> {
        let lower = definite_closure(self.program, cube, limits, &self.control)?;
        if lower.constraint_violated {
            return Ok(Narrowing::Refuted);
        }
        let upper = possible_closure(self.program, cube, limits, &self.control)?;
        let before = (cube.must.len(), cube.may.as_ref().map(BTreeSet::len));
        for atom in lower.atoms.atoms() {
            if self.program.contains_gate_atom(atom) {
                cube.must.insert(atom.clone());
            }
        }
        let derivable: BTreeSet<Atom> = upper
            .atoms
            .atoms()
            .iter()
            .filter(|atom| self.program.contains_gate_atom(atom))
            .cloned()
            .collect();
        cube.may = Some(match cube.may.take() {
            None => derivable,
            Some(may) => may.intersection(&derivable).cloned().collect(),
        });
        // A gate atom every seed of the region holds that no seed of it can
        // derive: the region holds no accepted seed
        // (`Bounds.conflicting_atom_refutes`). From the undecided cube the
        // lower closure lies inside the upper one; a split can part them.
        if cube
            .must
            .iter()
            .any(|atom| cube.may.as_ref().is_some_and(|may| !may.contains(atom)))
        {
            return Ok(Narrowing::Refuted);
        }
        let after = (cube.must.len(), cube.may.as_ref().map(BTreeSet::len));
        Ok(if after == before {
            Narrowing::Fixed
        } else {
            Narrowing::Changed
        })
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
                self.statistics.underivable_gate_atoms += 1;
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
