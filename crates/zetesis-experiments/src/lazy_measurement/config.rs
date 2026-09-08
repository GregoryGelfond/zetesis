use std::num::NonZeroUsize;

use clap::{Args, ValueEnum};
use serde::Serialize;
use zetesis_cpu::{Limits, lazy};
use zetesis_wgpu::GpuLimits;

use super::Error;
use crate::Backend;

/// Current-world overlap of positive join inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// Each occurrence selects one value; different worlds can be disjoint.
    Sparse,
    /// Every occurrence selects every value; masks cannot omit the product.
    Dense,
}

/// One bounded relational input and its ordered seed-occurrence family.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Case {
    /// Per-world overlap pattern.
    pub family: Family,
    /// Values available in each of three joined predicates, at most sixteen.
    pub width: NonZeroUsize,
    /// Submitted occurrences, at most 256; sparse patterns repeat after width.
    pub worlds: NonZeroUsize,
}

/// Library-owned finite schedule and independent logical/transport limits.
/// Cases may repeat deliberately; their ordinal distinguishes their populations.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// At most 64 cases, in caller order.
    pub cases: Vec<Case>,
    /// Require physical Metal, or explicitly measure CPU routes only.
    pub backend: Backend,
    /// Warm iterations per case, zero through six.
    pub warmups: usize,
    /// Timed iterations per case, one through sixty.
    pub repetitions: NonZeroUsize,
    /// Explicitly owned Rayon pool threads, one through 64.
    pub workers: NonZeroUsize,
    /// Scalar/Rayon budget per candidate.
    pub cpu_limits: Limits,
    /// Round-source budget per whole batch; shared by portable and Metal routes.
    pub source_limits: lazy::Limits,
    /// Physical transport budget per nonempty chunk.
    pub gpu_limits: GpuLimits,
}

impl Configuration {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.cases.is_empty() || self.cases.len() > 64 {
            return Err(Error::Configuration("require one through 64 cases"));
        }
        if self
            .cases
            .iter()
            .any(|case| case.width.get() > 16 || case.worlds.get() > 256)
        {
            return Err(Error::Configuration(
                "require width at most 16 and at most 256 worlds",
            ));
        }
        if self.warmups > 6 || self.repetitions.get() > 60 || self.workers.get() > 64 {
            return Err(Error::Configuration(
                "require at most six warmups, sixty repetitions and 64 workers",
            ));
        }
        Ok(())
    }
}

/// Command-line view of a bounded matched-source measurement configuration.
#[derive(Clone, Debug, Args)]
pub struct Options {
    /// Require Metal, or explicitly select CPU-only measurement.
    #[arg(long, value_enum, default_value_t)]
    pub backend: Backend,
    /// Values per positive predicate; the union join can contain width cubed rows.
    #[arg(long, value_delimiter = ',', default_value = "4,8")]
    pub widths: Vec<NonZeroUsize>,
    /// Ordered candidate occurrences; sparse seeds repeat after width.
    #[arg(long, value_delimiter = ',', default_value = "1,32,128")]
    pub batches: Vec<NonZeroUsize>,
    /// Sparse and dense controls, each retained as its own population.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "sparse,dense"
    )]
    pub families: Vec<Family>,
    /// Warm iterations, outside the timed population.
    #[arg(long, default_value_t = 2)]
    pub warmups: usize,
    /// Timed iterations; twelve balances both the four- and six-route schedules.
    #[arg(long, default_value = "12")]
    pub repetitions: NonZeroUsize,
    /// Threads in the independently owned scalar-checking Rayon pool.
    #[arg(long, default_value = "4")]
    pub workers: NonZeroUsize,
    /// Maximum source instances in one portable/Metal consequence chunk.
    #[arg(long, default_value = "256")]
    pub chunk_rules: NonZeroUsize,
    /// Work per scalar candidate and, separately, per whole round-source batch.
    #[arg(long, default_value_t = 100_000_000)]
    pub max_work: u64,
}

impl Options {
    /// Build a validated library configuration. Other native ceilings remain
    /// their published defaults and are emitted in the configuration event.
    ///
    /// # Errors
    /// Rejects oversized/empty products before allocating case storage.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        let count = self
            .widths
            .len()
            .checked_mul(self.batches.len())
            .and_then(|count| count.checked_mul(self.families.len()))
            .filter(|count| (1..=64).contains(count))
            .ok_or(Error::Configuration("require one through 64 cases"))?;
        let mut cases = Vec::with_capacity(count);
        for family in &self.families {
            for width in &self.widths {
                for worlds in &self.batches {
                    cases.push(Case {
                        family: *family,
                        width: *width,
                        worlds: *worlds,
                    });
                }
            }
        }
        let configuration = Configuration {
            cases,
            backend: self.backend,
            warmups: self.warmups,
            repetitions: self.repetitions,
            workers: self.workers,
            cpu_limits: Limits {
                max_work: self.max_work,
                ..Limits::default()
            },
            source_limits: lazy::Limits {
                max_source_work: self.max_work,
                max_chunk_rules: self.chunk_rules.get(),
                ..lazy::Limits::default()
            },
            gpu_limits: GpuLimits::default(),
        };
        configuration.validate()?;
        Ok(configuration)
    }
}
