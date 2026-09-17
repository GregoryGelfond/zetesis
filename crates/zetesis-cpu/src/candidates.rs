//! Incremental powerset enumeration whose first seed needs no carrier tuple.

use std::iter::FusedIterator;
use std::sync::Arc;
use zetesis_core::{
    Atom, GateAtom, GateAtomError, GateAtoms, Model, Program, Seed, SeedSelection,
    SeedSelectionError,
};

use crate::oracle::restrictions::{Conflict, Restrictions};
use crate::oracle::{Limits, lower_closure, upper_closure};
use crate::{Control, Stop};

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
    /// Why the requested carrier bounds were not applied: a closure stopped on
    /// a resource ceiling before the first pull, and the counter ran over the
    /// whole symbolic carrier instead. `None` when no bound was requested or
    /// both closures completed.
    pub bounds_stop: Option<Stop>,
}

enum RestrictionState {
    Disabled,
    Pending(CandidateRestrictionLimits),
    Prepared {
        limits: CandidateRestrictionLimits,
        plan: Restrictions,
    },
}

/// Whether the program's two closures bound the carrier, and how far that got.
enum BoundsState {
    Disabled,
    Pending(Limits),
    Applied,
    Unavailable,
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
    may: Option<Model>,
    must: Vec<Arc<Atom>>,
    atoms: Vec<Arc<GateAtom>>,
    bits: Vec<bool>,
    limits: CandidateLimits,
    control: Control,
    emitted: u64,
    started: bool,
    termination: Option<CandidateTermination>,
    restrictions: RestrictionState,
    bounds: BoundsState,
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
            atoms: Vec::new(),
            bits: Vec::new(),
            limits,
            control,
            emitted: 0,
            started: false,
            termination: None,
            restrictions: RestrictionState::Disabled,
            bounds: BoundsState::Disabled,
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

    /// Bound the carrier by the program's two closures before the first pull:
    /// [`crate::upper_closure`] through [`Self::within`] and
    /// [`crate::lower_closure`] through [`Self::requiring`], each computed
    /// under `limits` as one candidate check would be. A closure stopped by a
    /// resource ceiling leaves the counter over the whole symbolic carrier and
    /// is retained in the statistics; cancellation or a deadline stops the
    /// pull that met it.
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
    pub fn within(&mut self, may: Model) {
        debug_assert!(!self.started, "the bound precedes the first pull");
        self.may = Some(may);
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

    /// Carrier atoms successfully retained so far.
    #[must_use]
    pub fn discovered_atoms(&self) -> usize {
        self.atoms.len()
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
        self.prepare_restrictions()?;
        if self.started {
            if !self.advance()? {
                return Ok(None);
            }
        } else {
            self.started = true;
        }
        if !self.seek_permitted()? {
            return Ok(None);
        }
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
            self.bounds = BoundsState::Applied;
            return Ok(());
        }
        let closures = upper_closure(self.program, limits, &self.control).and_then(|may| {
            let must = lower_closure(self.program, limits, &self.control)?;
            Ok((may, must))
        });
        self.bounds = match closures {
            Ok((may, must)) => {
                self.within(may);
                self.requiring(&must)?;
                BoundsState::Applied
            }
            Err(stop @ (Stop::Cancelled | Stop::Deadline)) => return Err(stop),
            Err(stop) => {
                self.statistics.bounds_stop = Some(stop);
                BoundsState::Unavailable
            }
        };
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
