use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::Error;
use crate::{
    Backend,
    relation_fixtures::{Family, Payload},
};

pub(super) const MAX_BYTES: usize = 128 * 1024 * 1024;

/// One finite source/query population and its repeated execution schedule.
#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    /// Generic equality distribution; independent of an ASP encoding name.
    pub family: Family,
    /// Numeric or structural typed values.
    pub payload: Payload,
    /// Original row occurrences.
    pub rows: usize,
    /// Ordered query occurrences, including repeats and empty/missing controls.
    pub queries: usize,
    /// Requested physical backend, or CPU-only measurement.
    pub backend: Backend,
    /// Independently owned Rayon worker count.
    pub workers: usize,
    /// Retained warmup batches per route, separate from timed repetitions.
    pub warmups: usize,
    /// Timed batches per route; one initial batch is always also retained.
    pub repetitions: usize,
    /// Inclusive simultaneous authored storage ceiling, at most 128 MiB.
    /// Counts the fixture, relation/query/input views, masks, CPU selections and
    /// authored device transport. Driver allocations, allocator metadata, thread
    /// stacks, fixed report/hash metadata and consumer storage are outside it.
    pub max_bytes: usize,
}

impl Configuration {
    pub(super) fn validate(self) -> Result<Self, Error> {
        if !(1..=65_536).contains(&self.rows) || self.queries > 256 {
            return Err(Error::Configuration(
                "rows/queries exceed finite fixture scope",
            ));
        }
        if !(1..=64).contains(&self.workers)
            || self.warmups > 2
            || !(1..=6).contains(&self.repetitions)
        {
            return Err(Error::Configuration(
                "workers/warmups/repetitions exceed finite schedule",
            ));
        }
        if !(1..=MAX_BYTES).contains(&self.max_bytes) {
            return Err(Error::Configuration(
                "authored byte ceiling must be within 1..=128 MiB",
            ));
        }
        Ok(self)
    }
}

/// Command view for a single explicitly bounded relation experiment.
#[derive(Clone, Debug, clap::Args)]
pub struct Options {
    /// Equality distribution in the shared deterministic fixture.
    #[arg(long, value_enum, default_value = "independent")]
    pub family: Family,
    /// Whole-value logical payload.
    #[arg(long, value_enum, default_value = "numeric")]
    pub payload: Payload,
    /// Original source row occurrences (1..=65536).
    #[arg(long, default_value_t = 256)]
    pub rows: usize,
    /// Ordered query occurrences (0..=256).
    #[arg(long, default_value_t = 8)]
    pub queries: usize,
    /// CPU-only (the default), or a required physical GPU (gpu, metal or vulkan) with no fallback.
    #[arg(long, value_parser = zetesis_backend::BackendParser, default_value = "cpu")]
    pub backend: Backend,
    /// Owned Rayon pool size (1..=64).
    #[arg(long = "threads", alias = "workers", default_value_t = 4)]
    pub workers: usize,
    /// Retained warmup batches per route (0..=2).
    #[arg(long, default_value_t = 1)]
    pub warmups: usize,
    /// Timed batches per route (1..=6).
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
}

impl Options {
    /// Construct a validated finite library configuration.
    ///
    /// # Errors
    /// Refuses dimensions, workers or sample counts outside the declared scope.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        Configuration {
            family: self.family,
            payload: self.payload,
            rows: self.rows,
            queries: self.queries,
            backend: self.backend,
            workers: self.workers,
            warmups: self.warmups,
            repetitions: self.repetitions,
            max_bytes: MAX_BYTES,
        }
        .validate()
    }
}

pub(super) const fn family_name(value: Family) -> &'static str {
    match value {
        Family::Single => "single",
        Family::Independent => "independent",
        Family::Correlated => "correlated",
        Family::Skewed => "skewed",
    }
}
pub(super) const fn payload_name(value: Payload) -> &'static str {
    match value {
        Payload::Numeric => "numeric",
        Payload::Tuple => "tuple",
    }
}
impl Serialize for Configuration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("RelationConfiguration", 9)?;
        state.serialize_field("family", family_name(self.family))?;
        state.serialize_field("payload", payload_name(self.payload))?;
        state.serialize_field("rows", &self.rows)?;
        state.serialize_field("queries", &self.queries)?;
        state.serialize_field("backend", crate::backend::label(self.backend))?;
        state.serialize_field("workers", &self.workers)?;
        state.serialize_field("warmups", &self.warmups)?;
        state.serialize_field("repetitions", &self.repetitions)?;
        state.serialize_field("max_bytes", &self.max_bytes)?;
        state.end()
    }
}
