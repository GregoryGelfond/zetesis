//! Finite profile matrix and immutable scheduling positions.
use std::num::NonZeroUsize;
use std::path::Path;

use serde::Serialize;

use super::super::{Error, Phase};
use crate::selected::{Grounder, NativeExecution};

pub(super) const MAX_CASES: usize = 94;

/// Base source selections from the verified clean corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Suite {
    /// All 94 sealed runnable cases, in manifest order.
    Corpus,
    /// Established SEND, queens02 and task-allocation cases.
    Baseline,
    /// All six curated queens encodings at their default N=8.
    Queens,
}

/// Validated finite campaign configuration; requested profiles never imply execution.
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub(super) suite: Suite,
    pub(super) profiles: Vec<NativeExecution>,
    pub(super) reference_workers: NonZeroUsize,
    pub(super) warmups: usize,
    pub(super) repetitions: usize,
}
impl Plan {
    /// Construct up to eight explicit CPU/Metal eager/lazy profiles. Each worker
    /// count is bounded by 256; every profile retains its batch/scratch ceilings.
    /// Zero through five warmups and one through 41 timed rounds are admitted.
    ///
    /// # Errors
    /// Refuses empty/oversized profile families, automatic grounding or counts
    /// outside these bounds. Repeated profiles are permitted as authored controls.
    pub fn new(
        suite: Suite,
        profiles: Vec<NativeExecution>,
        reference_workers: NonZeroUsize,
        warmups: usize,
        repetitions: usize,
    ) -> Result<Self, Error> {
        super::super::Schedule::new(warmups, repetitions)?;
        if profiles.is_empty()
            || profiles.len() > 8
            || reference_workers.get() > 256
            || profiles.iter().any(|p| {
                p.grounder == Grounder::Auto
                    || p.workers.get() > 256
                    || p.completion_workers.get() > 256
            })
        {
            return Err(Error::Configuration(
                "matrix requires 1..=8 explicit profiles and workers 1..=256",
            ));
        }
        Ok(Self {
            suite,
            profiles,
            reference_workers,
            warmups,
            repetitions,
        })
    }
    /// Ordered requested native profiles, indexed by [`Producer::Native`].
    #[must_use]
    pub fn profiles(&self) -> &[NativeExecution] {
        &self.profiles
    }
    /// Selected base family; explicit workloads must originate in this family.
    #[must_use]
    pub const fn suite(&self) -> Suite {
        self.suite
    }
    /// Complete schedule. Qualification visits the reference first. Later rounds
    /// rotate both case and producer positions, without compacting refused cells.
    ///
    /// # Errors
    /// Refuses a case count outside the sealed corpus maximum.
    pub fn slots(&self, cases: usize) -> Result<Vec<Slot>, Error> {
        if !(1..=MAX_CASES).contains(&cases) {
            return Err(Error::Configuration("matrix cases must be 1..=94"));
        }
        let width = self.profiles.len() + 1;
        let mut slots = Vec::with_capacity(cases * width * (1 + self.warmups + self.repetitions));
        for (phase, rounds) in [
            (Phase::Qualification, 1),
            (Phase::Warmup, self.warmups),
            (Phase::Timed, self.repetitions),
        ] {
            for round in 0..rounds {
                for position in 0..cases {
                    let case = (position + round) % cases;
                    for position in 0..width {
                        let index = if phase == Phase::Qualification {
                            position
                        } else {
                            (position + round + case) % width
                        };
                        slots.push(Slot {
                            case,
                            phase,
                            round,
                            producer: if index == 0 {
                                Producer::Reference
                            } else {
                                Producer::Native { profile: index - 1 }
                            },
                        });
                    }
                }
            }
        }
        Ok(slots)
    }
}
/// Producer at a fixed configuration position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "solver", rename_all = "snake_case")]
pub enum Producer {
    /// Independent clingo, with the configured reference worker count.
    Reference,
    /// Native profile at its zero-based position in [`Plan::profiles`].
    Native {
        /// Profile index.
        profile: usize,
    },
}
impl Producer {
    pub(super) const fn index(self) -> usize {
        match self {
            Self::Reference => 0,
            Self::Native { profile } => profile + 1,
        }
    }
}
/// A requested invocation, retained even when an earlier failure prevents launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Slot {
    /// Zero-based case index in the report's input order.
    pub case: usize,
    /// First-observed, warmup or instrumented timed population.
    pub phase: Phase,
    /// Round within the population.
    pub round: usize,
    /// Requested solver/profile.
    pub producer: Producer,
}
/// Library-owned experiment request, independent of clap and global I/O.
#[derive(Debug)]
pub struct Request<'a> {
    /// Verified clean examples/kr-domains root.
    pub corpus: &'a Path,
    /// Absolute native executable.
    pub native: &'a Path,
    /// Absolute independent clingo executable.
    pub reference: &'a Path,
    /// New no-clobber evidence path in an exclusively owned parent.
    pub report: &'a Path,
    /// Immutable profile/schedule configuration.
    pub plan: Plan,
    /// Existing process, corpus, cumulative capture and publication bounds.
    pub limits: super::super::Limits,
    /// Native typed model/value acceptance bounds.
    pub native_answers: crate::answers::native_json::Limits,
    /// Combined selected-symbol spelling bytes per native report.
    pub max_spelling_bytes: usize,
}
