//! Fixed semantic tasks and independently bounded observation populations.
use super::Error;
use crate::{answers, examples, process};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

/// Pinned CPU inputs, without source rewriting or parameter overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Case {
    /// Unique SEND + MORE = MONEY answer set.
    Send,
    /// All 92 default eight-queens answers in original variant 02.
    /// Serialized as `queens` for compatibility with existing baseline records.
    #[serde(rename = "queens")]
    Queens02,
    /// Complete optimum family of task-allocation variant 04, scenario 05.
    TaskAllocation,
    /// Portable bounded-generation eight-queens baseline, variant 01.
    Queens01,
    /// Eight queens with exact-one row/column aggregates, variant 03.
    Queens03,
    /// Eight queens with diagonal cardinalities, variant 04.
    Queens04,
    /// Eight queens with explicit diagonal predicates, variant 05.
    Queens05,
    /// Eight queens with bounds in generating choices, variant 06.
    Queens06,
}
impl Case {
    /// Complete supported input catalog. Suite order is specified separately.
    pub const ALL: [Self; 8] = [
        Self::Send,
        Self::Queens02,
        Self::TaskAllocation,
        Self::Queens01,
        Self::Queens03,
        Self::Queens04,
        Self::Queens05,
        Self::Queens06,
    ];
    /// Entry path relative to the sealed clean corpus.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Send => "standalone/send-money/send-money.lp",
            Self::Queens02 => "standalone/n-queens/variant-02.lp",
            Self::TaskAllocation => "scenarios/task-allocation/variant-04/05-larger-mix.lp",
            Self::Queens01 => "standalone/n-queens/variant-01.lp",
            Self::Queens03 => "standalone/n-queens/variant-03.lp",
            Self::Queens04 => "standalone/n-queens/variant-04.lp",
            Self::Queens05 => "standalone/n-queens/variant-05.lp",
            Self::Queens06 => "standalone/n-queens/variant-06.lp",
        }
    }
    pub(super) const fn index(self) -> usize {
        match self {
            Self::Send => 0,
            Self::Queens02 => 1,
            Self::TaskAllocation => 2,
            Self::Queens01 => 3,
            Self::Queens03 => 4,
            Self::Queens04 => 5,
            Self::Queens05 => 6,
            Self::Queens06 => 7,
        }
    }
}

/// Finite authored selection, distinct from the complete supported case catalog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Suite {
    /// Established SEND, queens variant 02 and task-allocation comparison.
    #[default]
    Baseline,
    /// All six original eight-queens encodings in variant order.
    Queens,
}
impl Suite {
    /// Cases and execution order; no source constants or encodings are rewritten.
    #[must_use]
    pub const fn cases(self) -> &'static [Case] {
        match self {
            Self::Baseline => &[Case::Send, Case::Queens02, Case::TaskAllocation],
            Self::Queens => &[
                Case::Queens01,
                Case::Queens02,
                Case::Queens03,
                Case::Queens04,
                Case::Queens05,
                Case::Queens06,
            ],
        }
    }
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip callback receives a borrowed field"
    )]
    const fn is_baseline(&self) -> bool {
        matches!(self, Self::Baseline)
    }
}
/// Invocation population; only `Timed` belongs to the wall-time distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// First-observed correctness pair, without a cold-cache claim.
    Qualification,
    /// Retained preparation pairs, excluded from timed summaries.
    Warmup,
    /// Uninstrumented ordinary one-shot process observation.
    Timed,
    /// Separate native `--stats` invocation.
    Diagnostics,
}
/// Producer identity within a paired semantic task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Producer {
    /// Native CPU/eager/auto-oracle execution, one worker of each kind.
    Native,
    /// Independent single-worker clingo with exhaustive optN output.
    Reference,
}
/// One exact authored schedule position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Slot {
    /// Input whose complete selected family is required.
    pub case: Case,
    /// Observation population.
    pub phase: Phase,
    /// Zero-based round within this population.
    pub round: usize,
    /// Solver selected for this position.
    pub producer: Producer,
}
/// Number of paired rounds; bounded construction prevents unbounded schedules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Schedule {
    // Preserve the schema-1 baseline representation and its historical `queens`
    // case identifier. Non-baseline schedules state their suite explicitly.
    #[serde(skip_serializing_if = "Suite::is_baseline")]
    suite: Suite,
    warmups: usize,
    repetitions: usize,
}
impl Default for Schedule {
    fn default() -> Self {
        Self {
            suite: Suite::Baseline,
            warmups: 3,
            repetitions: 21,
        }
    }
}
impl Schedule {
    /// Select the baseline suite with zero through five warmups and one through
    /// 41 timed repetitions.
    ///
    /// # Errors
    /// Refuses a schedule outside these finite measurement bounds.
    pub fn new(warmups: usize, repetitions: usize) -> Result<Self, Error> {
        Self::for_suite(Suite::Baseline, warmups, repetitions)
    }
    /// Select a finite suite with zero through five warmups and one through 41
    /// timed repetitions; every case retains qualification and diagnostics.
    ///
    /// # Errors
    /// Refuses a schedule outside the same bounds as [`Self::new`].
    pub fn for_suite(suite: Suite, warmups: usize, repetitions: usize) -> Result<Self, Error> {
        if warmups > 5 || !(1..=41).contains(&repetitions) {
            return Err(Error::Configuration(
                "warmups must be 0..=5 and repetitions 1..=41",
            ));
        }
        Ok(Self {
            suite,
            warmups,
            repetitions,
        })
    }
    /// Authored source selection; independent of the solver's grounding policy.
    #[must_use]
    pub const fn suite(self) -> Suite {
        self.suite
    }
    /// Number of solve observations, excluding the three executable metadata calls.
    #[must_use]
    pub const fn expected_samples(self) -> usize {
        self.suite.cases().len() * (2 * (1 + self.warmups + self.repetitions) + 1)
    }
    /// Requested warmup pairs per input.
    #[must_use]
    pub const fn warmups(self) -> usize {
        self.warmups
    }
    /// Requested timed pairs per input.
    #[must_use]
    pub const fn repetitions(self) -> usize {
        self.repetitions
    }
    /// Complete deterministic schedule. Case order rotates between rounds; each
    /// case's producer order alternates. Qualification always runs reference first.
    #[must_use]
    pub fn slots(self) -> Vec<Slot> {
        let cases = self.suite.cases();
        let mut slots = Vec::with_capacity(self.expected_samples());
        for (phase, rounds) in [
            (Phase::Qualification, 1),
            (Phase::Warmup, self.warmups),
            (Phase::Timed, self.repetitions),
        ] {
            for round in 0..rounds {
                for position in 0..cases.len() {
                    let index = (position + round) % cases.len();
                    let case = cases[index];
                    let native_first =
                        phase != Phase::Qualification && (round + index).is_multiple_of(2);
                    let producers = if native_first {
                        [Producer::Native, Producer::Reference]
                    } else {
                        [Producer::Reference, Producer::Native]
                    };
                    slots.extend(producers.map(|producer| Slot {
                        case,
                        phase,
                        round,
                        producer,
                    }));
                }
            }
        }
        slots.extend(cases.iter().copied().map(|case| Slot {
            case,
            phase: Phase::Diagnostics,
            round: 0,
            producer: Producer::Native,
        }));
        slots
    }
}
/// Independent input, process, cumulative capture and publication ceilings.
/// These are logical quantities; none denotes peak heap or process RSS.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Clean corpus admission bounds.
    pub corpus: examples::Limits,
    /// Per-invocation capture and cleanup bounds.
    pub process: process::Limits,
    /// Deadline for scheduling the complete campaign, starting before corpus setup.
    pub campaign_timeout: Duration,
    /// Raw captured bytes across version queries and every solve.
    pub max_total_capture_bytes: usize,
    /// Each primary executable's byte-sealing ceiling.
    pub max_executable_bytes: usize,
    /// Per-producer complete reported-answer normalization bounds.
    pub answers: answers::Limits,
    /// Final serialized evidence bytes, including raw captures.
    pub max_report_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            corpus: examples::Limits::default(),
            process: process::Limits {
                timeout: Duration::from_secs(30),
                max_output_bytes: 4_194_304,
                cleanup_timeout: Duration::from_secs(1),
            },
            campaign_timeout: Duration::from_mins(3),
            max_total_capture_bytes: 134_217_728,
            max_executable_bytes: 268_435_456,
            answers: answers::Limits::default(),
            max_report_bytes: 536_870_912,
        }
    }
}
/// All paths and limits needed for a self-contained selected CPU refresh.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// Clean examples root containing the pinned manifest and sources.
    pub corpus: &'a Path,
    /// Absolute native executable; PATH lookup belongs to the CLI.
    pub native: &'a Path,
    /// Absolute independent clingo executable.
    pub reference: &'a Path,
    /// New report path outside the input corpus, in an exclusively owned parent.
    pub report: &'a Path,
    /// Explicit fixed observation populations and execution order.
    pub schedule: Schedule,
    /// Independent authored resource ceilings.
    pub limits: Limits,
}
