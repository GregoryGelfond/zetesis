//! Typed experiment bounds; native limits retain their native meanings.

use std::path::PathBuf;

use serde::Serialize;
use zetesis_themelios::{
    BundleAdmissionOptions, BundleLimits, DomainLimits, ExpansionLimits, FormulaLimits,
    GroundingOptions, JoinStrategy,
};

use super::Error;

mod views;

/// Independent retained diagnostic data ceilings, applied before publication.
/// Zero permits no item. These do not replace any native admission/search limit.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct CaptureLimits {
    /// Maximum phase records per detailed admission; exceeding this refuses the run.
    pub max_phase_records: usize,
    /// Maximum stable models retained per complete enumeration.
    pub max_models: usize,
    /// Maximum true-atom indices retained across one enumeration's models.
    pub max_model_atoms: usize,
    /// Maximum UTF-8 bytes across the complete canonical atom catalog.
    pub max_atom_text_bytes: usize,
    /// Maximum native path bytes across the retained source catalog.
    pub max_source_path_bytes: usize,
    /// Maximum framed subject bytes hashed per admission, without retaining them.
    pub max_subject_bytes: usize,
    /// Maximum bytes in the complete serialized report, including its newline.
    pub max_output_bytes: usize,
}

impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            max_phase_records: 4_096,
            max_models: 256,
            max_model_atoms: 65_536,
            max_atom_text_bytes: 1_048_576,
            max_source_path_bytes: 1_048_576,
            max_subject_bytes: 67_108_864,
            max_output_bytes: 16_777_216,
        }
    }
}

/// One fixed configuration shared by every observer condition.
///
/// Native limits are passed through unchanged. Each repetition makes three fresh
/// admissions; one additional unmeasured admission establishes the reference.
/// No limit denotes unlimited work. Search remains potentially exponential and
/// its native work ceiling is not a wall-clock deadline.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Configuration {
    /// Number of rotated three-condition rounds, from one through eleven.
    pub repetitions: usize,
    /// Original source graph loading and parsing ceilings.
    #[serde(with = "views::Bundle")]
    pub bundle: BundleLimits,
    /// Syntax and source-template admission ceilings.
    #[serde(with = "views::Admission")]
    pub admission: BundleAdmissionOptions,
    /// Source normalization ceilings.
    #[serde(with = "views::Expansion")]
    pub expansion: ExpansionLimits,
    /// Actual formula grounding and storage ceilings.
    #[serde(with = "views::Formula")]
    pub formula: FormulaLimits,
    /// Positive-join execution for timed admissions; the reference always uses indexed joins.
    #[serde(with = "views::Grounding")]
    pub grounding: GroundingOptions,
    /// Optional domain analysis in measured admissions, disabled by default.
    /// The independent reference always disables it. Inapplicable or stopped
    /// analysis keeps complete joins. A widened argument contributes no restriction;
    /// other finite arguments may still guard their corresponding rows.
    /// These logical analysis limits are not allocated-byte or deadline bounds.
    #[serde(serialize_with = "views::domain_request")]
    pub domain_analysis: Option<DomainLimits>,
    /// Native complete-model enumeration ceilings, outside admission timing.
    #[serde(with = "views::Search")]
    pub search: zetesis_sat::Limits,
    /// Optional tight-certificate attempt ceilings, outside admission timing.
    #[serde(with = "views::Certificate")]
    pub certificate: zetesis_ferraris::TightPlanLimits,
    /// Retained attribution, model identity and report ceilings.
    pub capture: CaptureLimits,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            repetitions: 3,
            bundle: BundleLimits::default(),
            admission: BundleAdmissionOptions::default(),
            expansion: ExpansionLimits::default(),
            formula: FormulaLimits::default(),
            grounding: GroundingOptions::default(),
            domain_analysis: None,
            search: zetesis_sat::Limits::default(),
            certificate: zetesis_ferraris::TightPlanLimits::default(),
            capture: CaptureLimits::default(),
        }
    }
}

impl Configuration {
    pub(super) fn validate(self) -> Result<Self, Error> {
        if !(1..=11).contains(&self.repetitions) {
            return Err(Error::Configuration("repetitions must be in 1..=11"));
        }
        Ok(self)
    }
}

/// Command adapter for the reusable [`super::profile`] operation.
#[derive(Debug, clap::Args)]
pub struct Options {
    /// Original source entry path; includes are loaded without rewriting bytes.
    pub source: PathBuf,
    /// Rotated rounds of unobserved, boundary-only and detailed admissions (1–11).
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
    /// Completed-support joins in measured admissions; the reference uses indexed joins.
    #[arg(long, value_enum, default_value_t = Joins::Indexed)]
    pub joins: Joins,
    /// Inclusive per-admission record ceiling; refusal preserves the recorded prefix.
    #[arg(long, default_value_t = 4_096)]
    pub max_phase_records: usize,
    /// Inclusive complete-enumeration model ceiling.
    #[arg(long, default_value_t = 256)]
    pub max_models: usize,
    /// Inclusive true-atom index ceiling across one enumeration.
    #[arg(long, default_value_t = 65_536)]
    pub max_model_atoms: usize,
    /// Inclusive UTF-8 byte ceiling across the canonical atom catalog.
    #[arg(long, default_value_t = 1_048_576)]
    pub max_atom_text_bytes: usize,
    /// Inclusive native path byte ceiling across the retained source catalog.
    #[arg(long, default_value_t = 1_048_576)]
    pub max_source_path_bytes: usize,
    /// Inclusive framed execution-subject byte ceiling for cross-binary evidence.
    #[arg(long, default_value_t = 67_108_864)]
    pub max_subject_bytes: usize,
    /// Inclusive complete JSON report byte ceiling.
    #[arg(long, default_value_t = 16_777_216)]
    pub max_output_bytes: usize,
}

impl Options {
    /// Build the library configuration with unchanged native default ceilings.
    #[must_use]
    pub fn configuration(&self) -> Configuration {
        Configuration {
            repetitions: self.repetitions,
            grounding: GroundingOptions {
                joins: match self.joins {
                    Joins::Indexed => JoinStrategy::Indexed,
                    Joins::Table => JoinStrategy::Table,
                },
            },
            capture: CaptureLimits {
                max_phase_records: self.max_phase_records,
                max_models: self.max_models,
                max_model_atoms: self.max_model_atoms,
                max_atom_text_bytes: self.max_atom_text_bytes,
                max_source_path_bytes: self.max_source_path_bytes,
                max_subject_bytes: self.max_subject_bytes,
                max_output_bytes: self.max_output_bytes,
            },
            ..Configuration::default()
        }
    }
}

/// Command spelling for the library's positive-join execution strategy.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Joins {
    /// Probe the existing shortest matching value posting.
    Indexed,
    /// Use prepared support masks for eligible completed-support patterns.
    Table,
}
