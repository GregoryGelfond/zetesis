//! Incremental powerset enumeration whose first seed needs no carrier tuple.

use std::iter::FusedIterator;

use zetesis_core::{Atom, AtomIter, Program, Seed};

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

/// Retained termination of the gate-seed enumerator, independent of membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateTermination {
    /// Every seed in the complete gate carrier was returned.
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
        }
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
        if self.started {
            if !self.advance()? {
                return Ok(None);
            }
        } else {
            self.started = true;
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

    fn advance(&mut self) -> Result<bool, Stop> {
        for bit in &mut self.bits {
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
