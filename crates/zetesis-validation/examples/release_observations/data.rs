//! Fixed observation decoding and schedule/receipt validation.

use serde::{Deserialize, Deserializer, de::Error as _};
use sha2::{Digest, Sha256};
use zetesis_validation::performance::{Phase, Producer, Schedule};

use super::{Result, dataset::Dataset, require};

const MAX_EMBEDDED_OBSERVATION_BYTES: usize = 1_048_576;

const PATHS: [&str; 9] = [
    "standalone/n-queens/variant-01.lp",
    "standalone/n-queens/variant-02.lp",
    "standalone/n-queens/variant-03.lp",
    "standalone/n-queens/variant-04.lp",
    "standalone/n-queens/variant-05.lp",
    "standalone/n-queens/variant-06.lp",
    "standalone/send-money/send-money.lp",
    "scenarios/task-allocation/variant-04/05-larger-mix.lp",
    "scenarios/shortest-path/variant-01/06-layered-dag.lp",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observations {
    schema: u32,
    kind: String,
    provenance_sha256: String,
    pub cases: Vec<Case>,
    pub blocks: Vec<Block>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub path: String,
    pub label: String,
    selected_models: u64,
    cost: Option<Vec<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Block {
    pub label: String,
    source: String,
    binary_sha256: String,
    original_report_sha256: String,
    started_unix_ns: u64,
    finished_unix_ns: u64,
    qualified: bool,
    input_seals_unchanged: bool,
    metadata_qualified: bool,
    fault_count: usize,
    unresolved_count: usize,
    formula_joins: Option<String>,
    pub observations: Vec<Observation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub case: String,
    #[serde(deserialize_with = "phase")]
    pub phase: Phase,
    round: usize,
    #[serde(deserialize_with = "producer")]
    pub producer: Producer,
    decision: String,
    selected_models: u64,
    cost: Option<Vec<i64>>,
    pub capture: Capture,
    pub memory: Option<Memory>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Capture {
    stop: String,
    exit_code: i32,
    signal: Option<i32>,
    pub elapsed_ns: u64,
    invocation_failed: bool,
    cleanup_failed: bool,
    unresolved: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Memory {
    exit_code: i32,
    signal: Option<i32>,
    raw_max_rss: u64,
    raw_unit: String,
    pub peak_rss_bytes: u64,
}

// Only the linked report identities are needed by this table view. The rest of
// the provenance remains public JSON, bound in full by the digest above.
#[derive(Deserialize)]
struct Provenance {
    original_reports: Vec<ReportIdentity>,
}
#[derive(Deserialize)]
struct ReportIdentity {
    label: String,
    sha256: String,
}

pub(super) fn load(dataset: &Dataset<'_>) -> Result<Observations> {
    require(
        dataset.observations.len() <= MAX_EMBEDDED_OBSERVATION_BYTES,
        "embedded observation byte bound exceeded",
    )?;
    let observations: Observations = serde_json::from_str(dataset.observations)?;
    validate(&observations, dataset)?;
    Ok(observations)
}

pub(super) fn validate(data: &Observations, dataset: &Dataset<'_>) -> Result<()> {
    require(
        data.schema == 1 && data.kind == "derived_historical_observation_view",
        "unknown observation schema or scope",
    )?;
    require(
        data.provenance_sha256 == format!("{:x}", Sha256::digest(dataset.provenance)),
        "provenance digest differs",
    )?;
    let provenance: Provenance = serde_json::from_str(dataset.provenance)?;
    require(
        data.cases.len() == PATHS.len() && data.blocks.len() == 4,
        "expected nine cases and four blocks",
    )?;
    for (case, expected) in data.cases.iter().zip(PATHS) {
        require(case.path == expected, "missing, extra or reordered case")?;
        require(
            !case.label.is_empty() && !case.label.contains(['|', '\n', '\r']),
            "invalid table label",
        )?;
    }
    let schedule = Schedule::for_cases(PATHS.iter().map(|path| (*path).into()).collect(), 1, 3)?
        .with_memory(1)?;
    let slots = schedule.slots();
    for (index, (block, label)) in data.blocks.iter().zip(dataset.labels).enumerate() {
        let version = usize::from(index == 1 || index == 2);
        require(
            block.label == label
                && block.source == dataset.sources[version]
                && block.binary_sha256 == dataset.binaries[version],
            "historical block or executable identity differs",
        )?;
        require(
            block.formula_joins.as_deref() == dataset.joins[version],
            "historical join strategy differs",
        )?;
        require(
            provenance
                .original_reports
                .iter()
                .any(|r| r.label == label && r.sha256 == block.original_report_sha256),
            "original report binding differs",
        )?;
        require(
            block.started_unix_ns < block.finished_unix_ns
                && block.qualified
                && block.input_seals_unchanged
                && block.metadata_qualified
                && block.fault_count == 0
                && block.unresolved_count == 0,
            "block does not record complete qualification",
        )?;
        require(
            block.observations.len() == slots.len(),
            "missing or extra observation",
        )?;
        for (sample, slot) in block.observations.iter().zip(&slots) {
            require(
                sample.case == slot.case.path()
                    && sample.phase == slot.phase
                    && sample.round == slot.round
                    && sample.producer == slot.producer,
                "missing, duplicate or reordered schedule position",
            )?;
            let case = data
                .cases
                .iter()
                .find(|case| case.path == sample.case)
                .ok_or("unknown observation case")?;
            validate_sample(sample, case)?;
        }
    }
    Ok(())
}

fn validate_sample(sample: &Observation, case: &Case) -> Result<()> {
    require(
        sample.decision == "pass"
            && sample.selected_models == case.selected_models
            && sample.cost == case.cost,
        "observation lacks recorded display/cost qualification",
    )?;
    let capture = &sample.capture;
    let solver_exit = if sample.producer == Producer::Native {
        0
    } else {
        30
    };
    let capture_exit = if sample.phase == Phase::Memory {
        0
    } else {
        solver_exit
    };
    require(
        capture.stop == "completed"
            && capture.exit_code == capture_exit
            && capture.signal.is_none()
            && !capture.invocation_failed
            && !capture.cleanup_failed
            && !capture.unresolved,
        "observation capture is incomplete",
    )?;
    match (&sample.memory, sample.phase) {
        (Some(memory), Phase::Memory) => require(
            memory.exit_code == solver_exit
                && memory.signal.is_none()
                && memory.raw_unit == "bytes"
                && memory.raw_max_rss == memory.peak_rss_bytes,
            "invalid separate child-RSS receipt",
        ),
        (None, Phase::Memory) | (Some(_), _) => {
            Err("memory receipt belongs only to the separate memory population".into())
        }
        (None, _) => Ok(()),
    }
}

fn phase<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<Phase, D::Error> {
    match String::deserialize(decoder)?.as_str() {
        "qualification" => Ok(Phase::Qualification),
        "warmup" => Ok(Phase::Warmup),
        "timed" => Ok(Phase::Timed),
        "diagnostics" => Ok(Phase::Diagnostics),
        "memory" => Ok(Phase::Memory),
        _ => Err(D::Error::custom("unknown observation phase")),
    }
}

fn producer<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<Producer, D::Error> {
    match String::deserialize(decoder)?.as_str() {
        "native" => Ok(Producer::Native),
        "reference" => Ok(Producer::Reference),
        _ => Err(D::Error::custom("unknown observation producer")),
    }
}
