//! Coverage policy and exact physical-test evidence, independent of instrumentation.
//!
//! The workspace and CPU-only CLI floors remain separate. These functions check
//! observed version strings, authored selections and retained output; Cargo and
//! LLVM execution, freshness, exclusive ownership and ordered status publication
//! remain the orchestration layer's responsibility.
mod physical;
mod toolchain;
pub use physical::{Group, Selection, physical_result, selection};
pub use toolchain::{Metadata, Observation, Physical, Tool, executable_identity, metadata};

use crate::{Error, json, require};
use serde_json::Value;

/// Inclusive retained policy, version-observation or physical-log input ceiling.
pub const MAX_INPUT_BYTES: usize = 16_777_216;

/// The filename filter every coverage report and floor adds to
/// cargo-llvm-cov's defaults: the four test-support crates' sources, which hold
/// test code, as the defaults already leave out `tests/` directories. The
/// receipt records it, and [`metadata`] refuses any other.
pub const SUPPORT_SOURCES: &str = "/crates/zetesis-(test|theory|clingo|reference)-support/";

pub(super) fn input_bytes(bytes: usize) -> Result<(), Error> {
    if bytes <= MAX_INPUT_BYTES {
        Ok(())
    } else {
        Err(Error::Limit {
            resource: "coverage input bytes",
            limit: MAX_INPUT_BYTES,
        })
    }
}

/// Whether an observation establishes a baseline or must meet the committed floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Record fresh profiles without making a gating claim.
    Baseline,
    /// Require both independently measured profiles to meet the floor.
    Gate,
}
impl Mode {
    /// Parse the two supported orchestration modes.
    /// # Errors
    /// Refuses any other spelling.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "gate" => Ok(Self::Gate),
            _ => Err(Error::Invalid(
                "coverage mode must be gate or baseline".into(),
            )),
        }
    }
    /// Stable serialized spelling.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Gate => "gate",
        }
    }
}
/// A declared coverage floor; absence of measurement is not zero percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Floor {
    percentage: Option<u8>,
}
impl Floor {
    /// Parse an ASCII integer percentage or `UNMEASURED`.
    /// # Errors
    /// Refuses signs, non-ASCII digits, fractions and percentages above 100.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let value = value.trim();
        if value == "UNMEASURED" {
            return Ok(Self { percentage: None });
        }
        require(
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
            "coverage floor must be UNMEASURED or an integer percentage from 0 to 100",
        )?;
        let number: u8 = value
            .parse()
            .map_err(|_| Error::Invalid("invalid coverage percentage".into()))?;
        require(number <= 100, "coverage floor exceeds 100")?;
        Ok(Self {
            percentage: Some(number),
        })
    }
    /// Check whether the declared floor can be used in the requested mode.
    /// # Errors
    /// A gate requires an already measured floor.
    pub fn admit(self, mode: Mode) -> Result<(), Error> {
        require(
            mode == Mode::Baseline || self.percentage.is_some(),
            "coverage floor is unmeasured; review a baseline first",
        )
    }
    /// Canonical spelling for command views and metadata.
    #[must_use]
    pub fn label(self) -> String {
        match self.percentage {
            None => "UNMEASURED".into(),
            Some(value) => value.to_string(),
        }
    }
}
/// Preserve or raise a previously measured floor.
/// # Errors
/// Refuses a decrease or replacement of a measured floor by `UNMEASURED`.
pub fn ratchet(previous: Floor, current: Floor) -> Result<(), Error> {
    match previous.percentage {
        None => Ok(()),
        Some(old) => require(
            current.percentage.is_some_and(|new| new >= old),
            "coverage floor cannot decrease",
        ),
    }
}
/// Select the previous commit from a GitHub push or pull-request event.
/// The all-zero push SHA and absent previous identity require no history lookup.
/// # Errors
/// Refuses malformed JSON, duplicate keys and a nonempty invalid commit identity.
pub fn previous_revision(event: &[u8]) -> Result<Option<String>, Error> {
    input_bytes(event.len())?;
    let event = json::parse(event)?;
    json::object(&event, "GitHub event")?;
    let value = event
        .pointer("/pull_request/base/sha")
        .filter(|value| !value.is_null() && value.as_str() != Some(""))
        .unwrap_or(&event["before"]);
    if value.is_null() {
        return Ok(None);
    }
    let previous = json::string(value, "previous revision")?;
    if previous.is_empty() || previous == "0".repeat(40) {
        return Ok(None);
    }
    require(
        previous.len() == 40
            && previous
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid previous revision for coverage floor comparison",
    )?;
    Ok(Some(previous.into()))
}
/// Verify a serialized metadata value can be represented without JSON errors.
/// # Errors
/// Refuses a JSON serialization failure.
pub fn render(value: &Value) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(Error::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}
