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
#[path = "release_observations/render.rs"]
mod render;
#[cfg(test)]
#[path = "release_observations/tests.rs"]
mod tests;

use std::error::Error;
use std::io::{self, Write};

const OBSERVATIONS: &str =
    include_str!("../../../docs/book/reference/observations/release-6bebb980-1e5b78ce.json");
const PROVENANCE: &str = include_str!(
    "../../../docs/book/reference/observations/release-6bebb980-1e5b78ce-provenance.json"
);
const TABLES: &str =
    include_str!("../../../docs/book/reference/observations/release-6bebb980-1e5b78ce-tables.md");

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let check = match (args.next(), args.next()) {
        (None, None) => false,
        (Some(arg), None) if arg == "--check" => true,
        _ => return Err("usage: release_observations [--check]".into()),
    };
    let observations = data::load()?;
    let tables = render::tables(&observations)?;
    require(
        tables == TABLES,
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
