use serde::Serialize;

use super::Error;

pub(super) const MAX_ROWS: usize = 8_192;
pub(super) const MAX_QUERIES: usize = 128;
pub(super) const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_RETAINED_BYTES: usize = 128 * 1024 * 1024;
pub(super) const MAX_WORK: u64 = 100_000_000;
pub(super) const MAX_VARIABLES: usize = 4;
pub(super) const DOMAIN_VALUES: usize = 16;
pub(super) const DICTIONARY_VALUES: usize = DOMAIN_VALUES + 1;

/// Fixed generic distributions, unrelated to any ASP domain encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Case {
    /// Two equal numeric columns with sixteen values.
    Correlated,
    /// Four independently varying columns with sixteen mixed typed values.
    Independent,
    /// Three columns, two variables and coherent/incoherent repeated-variable rows.
    Aliased,
}

/// Explicit finite subject and schedule. Counters are per operation, not RSS.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Configuration {
    /// Deterministic typed row distribution.
    pub case: Case,
    /// Original row occurrences, including repeated source indices (1..=8192).
    pub rows: usize,
    /// Ordered domains, repeating a fixed seven-query restoration sequence (1..=128).
    pub queries: usize,
    /// Actual size requested for the independently owned Rayon pool (1..=8).
    pub workers: usize,
    /// Retained warmup batches per route (0..=2).
    pub warmups: usize,
    /// Retained timed batches per route (1..=5).
    pub repetitions: usize,
    /// Per-table operation live-capacity ceiling, including relation/table inputs.
    /// Fixture, prepared scan domains and other retained query outputs are separate.
    pub max_table_bytes: usize,
    /// Per-table operation work ceiling; scan comparison counts use distinct units.
    pub max_table_work: u64,
}
impl Configuration {
    pub(super) fn validate(self) -> Result<Self, Error> {
        if !(1..=MAX_ROWS).contains(&self.rows) || !(1..=MAX_QUERIES).contains(&self.queries) {
            return Err(Error::Configuration(
                "rows or queries exceed finite fixture scope",
            ));
        }
        if !(1..=8).contains(&self.workers)
            || self.warmups > 2
            || !(1..=5).contains(&self.repetitions)
        {
            return Err(Error::Configuration(
                "worker or repetition schedule exceeds scope",
            ));
        }
        if self.max_table_bytes > MAX_RETAINED_BYTES || self.max_table_work > MAX_WORK {
            return Err(Error::Configuration(
                "table operation ceilings exceed scope",
            ));
        }
        // At most four projected variables with seventeen possible typed values.
        // Twice the requested cells covers ordinary geometric result growth;
        // runtime receipts also check the actual retained vector capacities.
        let output_bound = self.queries
            * (2 * (self.rows + MAX_VARIABLES * DICTIONARY_VALUES) * std::mem::size_of::<usize>()
                + std::mem::size_of::<super::Outcome>()
                + MAX_VARIABLES * std::mem::size_of::<Vec<usize>>());
        if output_bound > MAX_RETAINED_BYTES {
            return Err(Error::Configuration("result envelope exceeds 128 MiB"));
        }
        Ok(self)
    }
}

/// CLI view of one complete finite-table measurement population.
#[derive(Clone, Debug, clap::Args)]
pub struct Options {
    /// Generic relation distribution.
    #[arg(long, value_enum, default_value = "independent")]
    pub case: Case,
    /// Original row occurrences (1..=8192).
    #[arg(long, default_value_t = 1024)]
    pub rows: usize,
    /// Ordered domain occurrences (1..=128).
    #[arg(long, default_value_t = 32)]
    pub queries: usize,
    /// Owned Rayon pool size (1..=8).
    #[arg(long, default_value_t = 4)]
    pub workers: usize,
    /// Retained warmups per route (0..=2).
    #[arg(long, default_value_t = 1)]
    pub warmups: usize,
    /// Timed batches per route (1..=5).
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
    /// Finite-table per-operation capacity limit, excluding caller-owned fixtures/results.
    #[arg(long, default_value_t = MAX_RETAINED_BYTES)]
    pub max_table_bytes: usize,
    /// Finite-table per-operation logical work limit.
    #[arg(long, default_value_t = MAX_WORK)]
    pub max_table_work: u64,
}
impl Options {
    /// Validate the command's finite library configuration before acquisition.
    ///
    /// # Errors
    /// Refuses dimensions, sample counts and ceilings outside the documented scope.
    pub fn configuration(&self) -> Result<Configuration, Error> {
        Configuration {
            case: self.case,
            rows: self.rows,
            queries: self.queries,
            workers: self.workers,
            warmups: self.warmups,
            repetitions: self.repetitions,
            max_table_bytes: self.max_table_bytes,
            max_table_work: self.max_table_work,
        }
        .validate()
    }
}
