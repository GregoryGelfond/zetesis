use std::num::NonZeroUsize;

use clap::{Args, ValueEnum};
use serde::Serialize;

use super::Error;
use crate::Backend;

const MAX_CASES: usize = 24;
const MAX_ATOMS: usize = 256;
const MAX_OCCURRENCES: usize = 256;
const MAX_SUPPORT_ATOMS: usize = 4096;
const MAX_SUPPORT_OCCURRENCES: usize = 1024;
pub(super) const SUPPORT_MULTIPLICITY: usize = 16;
const EXHAUSTIVE_ATOMS: usize = 8;
const MAX_WARMUPS: usize = 6;
const MAX_REPETITIONS: usize = 60;
const MAX_WORKERS: usize = 64;

/// Ranked producer grammar, with one deliberately unsupported carrier atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// A fact followed by ordinary positive implications.
    Normal,
    /// Atomic choices, with the final producer guarded by its predecessor when one exists.
    Choices,
    /// Choice producers duplicated uniformly, sixteen occurrences per head.
    SupportUniform,
    /// The same total producer count, with duplicates concentrated on one head.
    SupportSkewed,
}

/// Command and report view of physical producer-support construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Independent atomic ORs, the ordinary library default.
    #[default]
    Atomic,
    /// One complete local reduction per support word.
    Grouped,
}

impl From<Support> for zetesis_wgpu::TightSupport {
    fn from(support: Support) -> Self {
        match support {
            Support::Atomic => Self::Atomic,
            Support::Grouped => Self::Grouped,
        }
    }
}

/// Independent complete reference instrument, selected before any measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reference {
    /// Enumerate proper subsets with the finite Ferraris kernel; at most eight atoms.
    ExhaustiveReduct,
    /// General original/frozen-reduct search, with no tight certificate enabled.
    GeneralReduct,
}

/// One complete synthetic theory and its ordered unfiltered candidate population.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Case {
    /// Complete producer family.
    pub family: Family,
    /// Semantic atoms, including an unsupported atom: at most 256 for the two
    /// original families, or 4096 for the explicit support-distribution families.
    pub atoms: NonZeroUsize,
    /// Candidate occurrences: at most 256 for the original families, or 1024
    /// for support distributions. Repetitions retain their identity.
    pub candidates: NonZeroUsize,
    /// Complete reference used outside sample timers.
    pub reference: Reference,
}

/// Finite measurement schedule and independent per-operation resource ceilings.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// One through 24 cases, in caller order.
    pub cases: Vec<Case>,
    /// Require physical Metal/Vulkan or explicitly select only the two CPU routes.
    pub backend: Backend,
    /// Physical producer-support construction; CPU references stay identical.
    pub support: Support,
    /// Preparation iterations per route, zero through six.
    pub warmups: usize,
    /// Timed iterations per route, one through sixty.
    pub repetitions: NonZeroUsize,
    /// Size of the independently owned Rayon pool, one through 64.
    pub workers: NonZeroUsize,
    /// Complete-theory certificate construction limits, applied per case.
    pub plan_limits: zetesis_ferraris::TightPlanLimits,
    /// Scalar certificate ceilings per occurrence, including failed attempts.
    pub certificate_limits: zetesis_ferraris::TightCheckLimits,
    /// Exact subset-reference and independent witness-evaluation ceilings per call.
    pub reference_limits: zetesis_ferraris::Limits,
    /// General reduct-reference and timed CPU residual ceilings per occurrence.
    pub residual_limits: zetesis_sat::Limits,
    /// Per-batch physical transport and per-candidate shader work bounds.
    pub gpu_limits: zetesis_wgpu::TightGpuLimits,
}

impl Configuration {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if !(1..=MAX_CASES).contains(&self.cases.len()) {
            return Err(Error::Configuration("require one through 24 cases"));
        }
        for case in &self.cases {
            let (max_atoms, max_occurrences) = match case.family {
                Family::Normal | Family::Choices => (MAX_ATOMS, MAX_OCCURRENCES),
                Family::SupportUniform | Family::SupportSkewed => {
                    (MAX_SUPPORT_ATOMS, MAX_SUPPORT_OCCURRENCES)
                }
            };
            if !(2..=max_atoms).contains(&case.atoms.get())
                || case.candidates.get() > max_occurrences
            {
                return Err(Error::Configuration(
                    "require 2..256 atoms and at most 256 occurrences; support families permit 2..4096 atoms and at most 1024 occurrences",
                ));
            }
            if case.reference == Reference::ExhaustiveReduct && case.atoms.get() > EXHAUSTIVE_ATOMS
            {
                return Err(Error::Configuration(
                    "exhaustive subset references are limited to eight atoms",
                ));
            }
        }
        if self.warmups > MAX_WARMUPS
            || self.repetitions.get() > MAX_REPETITIONS
            || self.workers.get() > MAX_WORKERS
        {
            return Err(Error::Configuration(
                "require at most six warmups, sixty repetitions and 64 workers",
            ));
        }
        Ok(())
    }
}

/// Installed command view of matched certificate execution.
#[derive(Clone, Debug, Args)]
pub struct Options {
    /// Require Metal/Vulkan or explicitly select scalar/Rayon checking only.
    #[arg(long = "device", alias = "backend", value_enum, default_value_t)]
    pub backend: Backend,
    /// Physical support construction; applies only to this primitive experiment.
    #[arg(long, value_enum, default_value_t)]
    pub support: Support,
    /// Total atoms; widths above eight use general exact reduct references.
    #[arg(long, value_delimiter = ',', default_value = "4,64,256")]
    pub atoms: Vec<NonZeroUsize>,
    /// Ordered unfiltered candidate occurrences.
    #[arg(long, value_delimiter = ',', default_value = "1,32,128")]
    pub batches: Vec<NonZeroUsize>,
    /// Complete ranked producer families, including an unsupported carrier atom.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "normal,choices"
    )]
    pub families: Vec<Family>,
    /// Preparation iterations outside the timed population.
    #[arg(long, default_value_t = 2)]
    pub warmups: usize,
    /// Twelve balances both the two- and four-route schedules.
    #[arg(long, default_value = "12")]
    pub repetitions: NonZeroUsize,
    /// Independently owned Rayon workers.
    #[arg(long = "threads", alias = "workers", default_value = "4")]
    pub workers: NonZeroUsize,
    /// Per-operation logical work; different algorithms retain different charges.
    #[arg(long, default_value_t = 100_000_000)]
    pub max_work: u64,
}

impl Options {
    /// Construct the bounded schedule before allocating cases. Native storage,
    /// decision and subset ceilings retain their published defaults.
    ///
    /// # Errors
    /// Refuses empty/oversized products and unsupported dimensions.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        let count = self
            .atoms
            .len()
            .checked_mul(self.batches.len())
            .and_then(|count| count.checked_mul(self.families.len()))
            .filter(|count| (1..=MAX_CASES).contains(count))
            .ok_or(Error::Configuration("require one through 24 cases"))?;
        let mut cases = super::reserve(count)?;
        for &family in &self.families {
            for &atoms in &self.atoms {
                for &candidates in &self.batches {
                    cases.push(Case {
                        family,
                        atoms,
                        candidates,
                        reference: if atoms.get() <= EXHAUSTIVE_ATOMS {
                            Reference::ExhaustiveReduct
                        } else {
                            Reference::GeneralReduct
                        },
                    });
                }
            }
        }
        let configuration = Configuration {
            cases,
            backend: self.backend,
            support: self.support,
            warmups: self.warmups,
            repetitions: self.repetitions,
            workers: self.workers,
            plan_limits: zetesis_ferraris::TightPlanLimits {
                max_work: self.max_work,
                ..Default::default()
            },
            certificate_limits: zetesis_ferraris::TightCheckLimits {
                max_work: self.max_work,
                ..Default::default()
            },
            reference_limits: zetesis_ferraris::Limits {
                max_work: self.max_work,
                ..Default::default()
            },
            gpu_limits: zetesis_wgpu::TightGpuLimits {
                max_work_per_candidate: self.max_work,
                ..Default::default()
            },
            residual_limits: zetesis_sat::Limits {
                search: zetesis_sat::SearchLimits {
                    max_work: self.max_work,
                    ..Default::default()
                },
                max_verification_work: self.max_work,
                ..Default::default()
            },
        };
        configuration.validate()?;
        Ok(configuration)
    }
}
