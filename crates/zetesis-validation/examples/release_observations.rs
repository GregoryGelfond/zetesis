//! Reproduce a selected fixed comparison from its curated observation receipts.
//!
//! This fixed-data example performs no process execution or measurement. The
//! input is embedded and below one MiB; decoding uses storage bounded by those
//! bytes. Validation uses the maintained performance schedule before examining
//! the three timed and one separate RSS observations per producer/case/block.
//! Recorded qualification is evidence from the original producer, not a new
//! comparison of output streams or hidden interpretations.

#[path = "release_observations/data.rs"]
mod data;
#[path = "release_observations/catalog_dataset.rs"]
mod catalog_dataset;
#[path = "release_observations/dataset.rs"]
mod dataset;
#[path = "release_observations/prepared_dataset.rs"]
mod prepared_dataset;
#[path = "release_observations/render.rs"]
mod render;
#[cfg(test)]
#[path = "release_observations/tests.rs"]
mod tests;

use std::error::Error;
use std::io::{self, Write};

use clap::{Parser, ValueEnum};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Parser)]
#[command(about = "Render checked tables from one embedded comparison")]
struct Options {
    /// Fixed source comparison to reproduce; never an input file path.
    #[arg(long, value_enum, default_value = "table-grounding")]
    dataset: Comparison,
    /// Validate the recorded observations and published tables without printing.
    #[arg(long)]
    check: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Comparison {
    TableGrounding,
    AtomCatalog,
    #[value(name = "release-ca10a5e7-679ca856")]
    PreparedGrounding,
    #[value(name = "release-f56a5a24-679ca856")]
    PreparedAlgorithms,
}

impl Comparison {
    fn dataset(self) -> &'static dataset::Dataset<'static> {
        match self {
            Self::TableGrounding => &dataset::HISTORICAL,
            Self::AtomCatalog => &catalog_dataset::ATOM_CATALOG,
            Self::PreparedGrounding => &prepared_dataset::PREPARED_GROUNDING,
            Self::PreparedAlgorithms => &prepared_dataset::PREPARED_ALGORITHMS,
        }
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    let dataset = options.dataset.dataset();
    let observations = data::load(dataset)?;
    let tables = render::tables(&observations, dataset)?;
    require(
        tables == dataset.tables,
        "rendered values differ from the published tables",
    )?;
    if !options.check {
        io::stdout().lock().write_all(tables.as_bytes())?;
    }
    Ok(())
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
