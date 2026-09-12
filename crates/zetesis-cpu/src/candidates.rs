//! Incremental powerset enumeration whose first seed needs no carrier tuple.

use std::iter::FusedIterator;

use zetesis_core::{Atom, AtomIter, Program, Seed};

use crate::oracle::restrictions::{Conflict, Restrictions};
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
    /// comparisons are unmetered and tree allocation is infallible. Its copied
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
}

enum RestrictionState {
    Disabled,
    Pending(CandidateRestrictionLimits),
    Prepared {
        limits: CandidateRestrictionLimits,
        plan: Restrictions,
    },
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
pub struct Candidates<'a> {
    program: &'a Program,
    carrier: AtomIter<'a>,
    atoms: Vec<Atom>,
    bits: Vec<bool>,
    limits: CandidateLimits,
    control: Control,
    emitted: u64,
    started: bool,
    termination: Option<CandidateTermination>,
    restrictions: RestrictionState,
    statistics: CandidateStatistics,
}

impl<'a> Candidates<'a> {
    /// Create an unstarted iterator without expanding the gate carrier.
    #[must_use]
    pub fn new(program: &'a Program, limits: CandidateLimits, control: Control) -> Self {
        Self {
            program,
            carrier: program.gate_atoms(),
            atoms: Vec::new(),
            bits: Vec::new(),
            limits,
            control,
            emitted: 0,
            started: false,
            termination: None,
            restrictions: RestrictionState::Disabled,
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

    fn next_seed(&mut self) -> Result<Option<Seed>, Stop> {
        self.control.poll()?;
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
        let seed = Seed::new(
            self.program,
            self.atoms
                .iter()
                .zip(&self.bits)
                .filter(|(_, selected)| **selected)
                .map(|(atom, _)| atom.clone()),
        )
        .map_err(|_| Stop::InvalidProgram)?;
        self.control.poll()?;
        self.emitted += 1;
        Ok(Some(seed))
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
            let (result, work) = plan.conflict(&self.atoms, &self.bits, remaining, &self.control);
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
        let Some(atom) = self.carrier.next() else {
            return Ok(false);
        };
        let atom = atom.map_err(|_| Stop::Allocation)?;
        if self.atoms.len() >= self.limits.max_carrier_atoms {
            return Err(Stop::CarrierLimit);
        }
        self.atoms.try_reserve(1).map_err(|_| Stop::Allocation)?;
        self.bits.try_reserve(1).map_err(|_| Stop::Allocation)?;
        self.atoms.push(atom);
        self.bits.push(true);
        Ok(true)
    }
}

impl Iterator for Candidates<'_> {
    type Item = Result<Seed, Stop>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.termination.is_some() {
            return None;
        }
        match self.next_seed() {
            Ok(Some(seed)) => Some(Ok(seed)),
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
}

impl FusedIterator for Candidates<'_> {}
