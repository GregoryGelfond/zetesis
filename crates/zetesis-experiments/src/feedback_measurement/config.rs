use super::Error;
use clap::Args;
use serde::Serialize;

/// Named guard-construction ceilings, all independent of native SAT budgets.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ConstructionLimits {
    /// Inclusive emitted node count per guard, at most 256.
    pub max_nodes: usize,
    /// Inclusive total nodes in retained guards, at most 2,048.
    pub max_total_nodes: usize,
    /// Distinct retained guards, at most eight.
    pub max_guards: usize,
    /// One reservation sequence: entry initialization or a guard's
    /// node/root/map/witness vectors, at most 64 KiB.
    pub max_build_bytes: usize,
    /// Guard-entry capacity plus retained and building vectors, at most 256 KiB.
    /// Excludes original sources, native SAT/Interpretation/evaluation storage,
    /// output, Arc headers and allocator metadata. This is not RSS.
    pub max_retained_bytes: usize,
    /// Cumulative copied/emitted nodes, scans and reservation/publication steps.
    pub max_work: u64,
}
impl Default for ConstructionLimits {
    fn default() -> Self {
        Self {
            max_nodes: 256,
            max_total_nodes: 2048,
            max_guards: 8,
            max_build_bytes: 64 * 1024,
            max_retained_bytes: 256 * 1024,
            max_work: 1_000_000,
        }
    }
}

/// Fixed eight-owner study schedule. Each native call has finite independent
/// limits; construction work is cumulative within one owner replay.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Configuration {
    /// Warmup observations of every route/owner, zero through two.
    pub warmups: usize,
    /// Timed observations of every route/owner, zero through twelve.
    /// Zero performs complete qualification only.
    pub repetitions: usize,
    /// Separate experimental construction ceilings.
    pub construction: ConstructionLimits,
    /// Native per-call or whole iterator search and verification work, at most 1M.
    /// Candidate64/decision100k, CNF4096/16384/65536 and projection64/512/64KiB
    /// ceilings remain fixed. Reduct shape has the same independent CNF ceilings
    /// and a 1 MiB named-storage bound. All appear in each start record.
    pub max_native_work: u64,
    /// Per-call exhaustive reference or guard evaluation work, at most 1M.
    pub max_reference_work: u64,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            warmups: 1,
            repetitions: 3,
            construction: ConstructionLimits::default(),
            max_native_work: 1_000_000,
            max_reference_work: 1_000_000,
        }
    }
}
impl Configuration {
    pub(super) fn validate(self) -> Result<(), Error> {
        let limits = self.construction;
        if self.warmups > 2
            || self.repetitions > 12
            || limits.max_guards > 8
            || limits.max_nodes > 256
            || limits.max_total_nodes > 2048
            || limits.max_build_bytes > 64 * 1024
            || limits.max_retained_bytes > 256 * 1024
            || limits.max_work > 1_000_000
            || self.max_native_work > 1_000_000
            || self.max_reference_work > 1_000_000
        {
            return Err(Error::Configuration(
                "requested limits exceed the fixed finite study",
            ));
        }
        Ok(())
    }
    pub(super) fn native(self) -> zetesis_sat::Limits {
        let admission = zetesis_sat::AdmissionLimits {
            max_variables: 4096,
            max_clauses: 16_384,
            max_literals: 65_536,
        };
        zetesis_sat::Limits {
            admission,
            reduct_admission: admission,
            projections: zetesis_sat::ProjectionLimits {
                max_entries: 64,
                max_nodes: 512,
                max_bytes: 64 * 1024,
            },
            search: zetesis_sat::SearchLimits {
                max_work: self.max_native_work,
                max_decisions: 100_000,
            },
            max_candidates: 64,
            max_verification_work: self.max_native_work,
            max_reduct_bytes: 1024 * 1024,
        }
    }
}

/// Small process adapter; the fixed source population cannot be replaced by input.
#[derive(Clone, Debug, Args)]
pub struct Options {
    /// Perform complete semantic qualification without timed observations.
    #[arg(long)]
    pub check: bool,
    /// Warmup whole-owner observations, zero through two.
    #[arg(long, default_value_t = 1)]
    pub warmups: usize,
    /// Timed whole-owner observations, zero through twelve.
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
}
impl Options {
    /// Compose the fixed finite configuration.
    ///
    /// # Errors
    /// Refuses a population beyond the finite experiment maxima.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        let configuration = Configuration {
            warmups: if self.check { 0 } else { self.warmups },
            repetitions: if self.check { 0 } else { self.repetitions },
            ..Configuration::default()
        };
        configuration.validate()?;
        Ok(configuration)
    }
}
