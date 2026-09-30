//! Finite profile matrix and immutable scheduling positions.
use std::num::NonZeroUsize;
use std::path::Path;

use serde::Serialize;

use super::super::{Error, Phase};
use crate::selected::NativeExecution;

pub(super) const MAX_CASES: usize = 94;

/// Maintained source populations, including independently sealed authored inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Suite {
    /// All 94 sealed runnable cases, in manifest order.
    Corpus,
    /// Established SEND, queens02 and task-allocation cases.
    Baseline,
    /// All six curated queens encodings at their default N=8.
    Queens,
    /// The fixed cell set of [`super::super::series`]: generated families,
    /// amended queens boards and two unchanged entries, meant for
    /// `run_workloads` with the series' workloads. Its corpus entries are
    /// the series' own list, the queens, SEND and task-allocation cases;
    /// a plain run under this suite measures only those, unchanged.
    Series,
    /// Authored queens/pigeonhole sizes and the three baseline corpus entries,
    /// with optional Einstein. Requires explicit workloads, normally supplied by
    /// [`super::super::scalability::run_with_cancellation`].
    Scalability,
}

/// Which populations include the independent reference solver when one takes part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferencePolicy {
    /// Qualify and measure the reference alongside every native profile.
    AllPhases,
    /// Establish one complete reference census per case; measure native profiles only.
    QualificationOnly,
}

/// The reference policy a report records: the one the campaign ran under, or
/// `clingo_free` when no reference took part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedPolicy {
    /// The reference was qualified and measured alongside every native profile.
    AllPhases,
    /// The reference established one complete census per case; only native
    /// profiles were measured.
    QualificationOnly,
    /// No reference took part: each native family was qualified against its
    /// workload's recorded contract.
    ClingoFree,
}
impl RecordedPolicy {
    /// The policy the campaign ran under; none for a clingo-free campaign.
    #[must_use]
    pub const fn reference(self) -> Option<ReferencePolicy> {
        match self {
            Self::AllPhases => Some(ReferencePolicy::AllPhases),
            Self::QualificationOnly => Some(ReferencePolicy::QualificationOnly),
            Self::ClingoFree => None,
        }
    }
}
impl From<Option<ReferencePolicy>> for RecordedPolicy {
    fn from(policy: Option<ReferencePolicy>) -> Self {
        match policy {
            Some(ReferencePolicy::AllPhases) => Self::AllPhases,
            Some(ReferencePolicy::QualificationOnly) => Self::QualificationOnly,
            None => Self::ClingoFree,
        }
    }
}

/// The independent reference solver, clingo, and what a campaign uses it for.
/// A campaign without one is clingo-free: each native family is qualified
/// against its workload's recorded contract.
#[derive(Clone, Copy, Debug)]
pub struct Reference<'a> {
    /// Absolute clingo executable.
    pub executable: &'a Path,
    /// Whether clingo is measured in every phase or only establishes each census.
    pub policy: ReferencePolicy,
}

/// Validated finite campaign configuration; requested profiles never imply execution.
/// Whether a reference takes part is the request's, not the plan's.
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub(super) suite: Suite,
    pub(super) profiles: Vec<NativeExecution>,
    pub(super) reference_workers: NonZeroUsize,
    pub(super) warmups: usize,
    pub(super) repetitions: usize,
    /// Separate child-resource rounds per producer and case, after the
    /// timed rounds; zero unless requested.
    pub(super) memory_runs: usize,
    /// Relative entry paths of the suite to run, in caller order; the
    /// whole suite when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selection: Option<Vec<String>>,
}
impl Plan {
    /// Construct up to eight profiles, on any backend. Each worker count is bounded
    /// by 256; every profile retains its batch/scratch ceilings. Zero through
    /// five warmups and one through 41 timed rounds are admitted. An automatic
    /// grounding request is admitted; its observations retain the mode taken.
    ///
    /// # Errors
    /// Refuses empty/oversized profile families or counts outside these bounds.
    /// Repeated profiles are permitted as authored controls.
    pub fn new(
        suite: Suite,
        profiles: Vec<NativeExecution>,
        reference_workers: NonZeroUsize,
        warmups: usize,
        repetitions: usize,
    ) -> Result<Self, Error> {
        super::super::Schedule::new(warmups, repetitions)?;
        validate_profiles(&profiles, reference_workers)?;
        Ok(Self {
            suite,
            profiles,
            reference_workers,
            warmups,
            repetitions,
            memory_runs: 0,
            selection: None,
        })
    }

    /// Qualify each complete family once with each native profile, and with
    /// the reference when one takes part. No warmup, timed or memory positions
    /// are scheduled. Profile and worker bounds are identical to [`Self::new`].
    ///
    /// # Errors
    /// Refuses empty/oversized profile families or worker counts above 256.
    pub fn qualification(
        suite: Suite,
        profiles: Vec<NativeExecution>,
        reference_workers: NonZeroUsize,
    ) -> Result<Self, Error> {
        validate_profiles(&profiles, reference_workers)?;
        Ok(Self {
            suite,
            profiles,
            reference_workers,
            warmups: 0,
            repetitions: 0,
            memory_runs: 0,
            selection: None,
        })
    }
    /// Request zero through 41 memory rounds per producer and case: each a
    /// separate invocation through a fresh helper that reports the child's
    /// peak resident set, excluded from the timed population.
    ///
    /// # Errors
    /// Refuses more than 41 rounds, or nonzero rounds on a qualification plan.
    pub fn with_memory(mut self, rounds: usize) -> Result<Self, Error> {
        if rounds > 41 || (self.repetitions == 0 && rounds != 0) {
            return Err(Error::Configuration(
                "memory rounds must be 0..=41 and require a measurement plan",
            ));
        }
        self.memory_runs = rounds;
        Ok(self)
    }
    /// Memory rounds per producer and case.
    #[must_use]
    pub const fn memory_runs(&self) -> usize {
        self.memory_runs
    }
    /// Run exactly `paths`, one through 94 distinct relative entry paths of
    /// the plan's suite, in the order given, instead of the whole suite.
    /// The ordinary matrix runner selects corpus cases. When
    /// [`super::super::command::run`] expands the series, paths name workload
    /// entries, including generated paths, and one entry can select several
    /// amended cells. An unknown path is refused before launching anything.
    ///
    /// # Errors
    /// Refuses empty or oversized selections, duplicates and escaping or empty paths.
    pub fn with_cases(mut self, paths: Vec<String>) -> Result<Self, Error> {
        super::super::config::selection(&paths)?;
        self.selection = Some(paths);
        Ok(self)
    }
    /// The selected cases, in order; none when the whole suite runs.
    #[must_use]
    pub fn selection(&self) -> Option<&[String]> {
        self.selection.as_deref()
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
    /// Complete schedule under `reference`, the policy of the reference that
    /// takes part, or none for a clingo-free campaign. Qualification visits the
    /// reference first. Later rounds rotate both case and producer positions,
    /// without compacting refused cells. The memory rounds follow the timed rounds.
    ///
    /// # Errors
    /// Refuses a case count outside the sealed corpus maximum.
    pub fn slots(
        &self,
        cases: usize,
        reference: Option<ReferencePolicy>,
    ) -> Result<Vec<Slot>, Error> {
        if !(1..=MAX_CASES).contains(&cases) {
            return Err(Error::Configuration("matrix cases must be 1..=94"));
        }
        let width = self.profiles.len() + 1;
        let mut slots = Vec::with_capacity(
            cases * width * (1 + self.warmups + self.repetitions + self.memory_runs),
        );
        for (phase, rounds) in [
            (Phase::Qualification, 1),
            (Phase::Warmup, self.warmups),
            (Phase::Timed, self.repetitions),
            (Phase::Memory, self.memory_runs),
        ] {
            let reference = reference.is_some_and(|policy| {
                phase == Phase::Qualification || policy == ReferencePolicy::AllPhases
            });
            let width = self.profiles.len() + usize::from(reference);
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
                            producer: if reference && index == 0 {
                                Producer::Reference
                            } else {
                                Producer::Native {
                                    profile: index - usize::from(reference),
                                }
                            },
                        });
                    }
                }
            }
        }
        Ok(slots)
    }
}

fn validate_profiles(
    profiles: &[NativeExecution],
    reference_workers: NonZeroUsize,
) -> Result<(), Error> {
    if profiles.is_empty()
        || profiles.len() > 8
        || reference_workers.get() > 256
        || profiles
            .iter()
            .any(|profile| profile.workers.get() > 256 || profile.completion_workers.get() > 256)
    {
        return Err(Error::Configuration(
            "matrix requires 1..=8 profiles and workers 1..=256",
        ));
    }
    Ok(())
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
/// The tool that runs a campaign and writes its report, with its version: a
/// report names what produced it beside the executables it measured, as
/// benchmarking tools' result files do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Tool {
    /// The tool's name, for example `zetesis-bench`.
    pub name: String,
    /// The tool's version.
    pub version: String,
}

/// Library-owned experiment request, independent of clap and global I/O.
#[derive(Debug)]
pub struct Request<'a> {
    /// The tool that runs the campaign; its report names it.
    pub tool: Tool,
    /// Verified clean examples/correctness root.
    pub corpus: &'a Path,
    /// Absolute native executable.
    pub native: &'a Path,
    /// The independent reference solver when one takes part; absent, the
    /// campaign is clingo-free.
    pub reference: Option<Reference<'a>>,
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
    /// Absolute helper executable that runs each memory round's solver as
    /// its child and records the child's peak resident set; required when
    /// the plan has memory rounds, and sealed with the other executables.
    pub helper: Option<&'a Path>,
}
