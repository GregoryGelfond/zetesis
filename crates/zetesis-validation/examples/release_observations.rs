//! Reproduce one historical release table from its curated observation receipts.
//!
//! This fixed-data example performs no process execution or measurement. The
//! input is embedded and below one MiB; decoding uses storage bounded by those
//! bytes. Validation uses the maintained performance schedule before examining
//! the three timed and one separate RSS observations per producer/case/block.
//! Recorded qualification is evidence from the original producer, not a new
//! comparison of output streams or hidden interpretations.

#[path = "release_observations/data.rs"]
mod data;
#[path = "release_observations/dataset.rs"]
mod dataset;
#[path = "release_observations/render.rs"]
mod render;
#[cfg(test)]
#[path = "release_observations/tests.rs"]
mod tests;

use std::error::Error;
use std::io::{self, Write};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let check = match (args.next(), args.next()) {
        (None, None) => false,
        (Some(arg), None) if arg == "--check" => true,
        _ => return Err("usage: release_observations [--check]".into()),
    };
    let dataset = &dataset::HISTORICAL;
    let observations = data::load(dataset)?;
    let tables = render::tables(&observations, dataset)?;
    require(
        tables == dataset.tables,
        "rendered values differ from the published tables",
    )?;
    if !check {
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
