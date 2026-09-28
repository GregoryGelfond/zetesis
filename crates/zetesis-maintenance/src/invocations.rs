//! Live documentation names only executables the workspace builds.
//!
//! Every command in a shell code block, and every tool table's first cell,
//! whose first word is a zetesis executable must name a binary or package of
//! the workspace, and `zetesis bench` is not a command. A document carrying
//! [`RECORD`] is a dated measurement record: its commands keep the spellings
//! of the binaries it records, so it is not checked. Prose is not checked; it
//! may name an executable's former spelling on purpose.

use std::collections::BTreeSet;

use crate::{Error, require, workspace};

/// Marks a dated measurement record, whose commands keep the spellings of the
/// binaries it records.
pub const RECORD: &str =
    "<!-- A dated record: its commands keep the spellings of the binaries it records. -->";

/// Code-block languages whose lines are shell commands.
const SHELLS: [&str; 5] = ["sh", "shell", "bash", "zsh", "console"];

/// Check each live document against the workspace's binaries and packages.
///
/// `documents` pairs each document's path, as reported, with its text;
/// `metadata` is the output of `cargo metadata --no-deps --format-version 1`.
///
/// # Errors
/// Returns [`Error::Invalid`] listing every offending line, and [`Error::Json`]
/// for malformed metadata.
pub fn check<'a>(
    documents: impl IntoIterator<Item = (&'a str, &'a str)>,
    metadata: &[u8],
) -> Result<(), Error> {
    let members = workspace::packages(metadata)?;
    let known: BTreeSet<&str> = members
        .iter()
        .flat_map(|(name, package)| {
            std::iter::once(name.as_str()).chain(package.binaries.iter().map(String::as_str))
        })
        .collect();
    let mut stale = Vec::new();
    for (path, text) in documents {
        if text.contains(RECORD) {
            continue;
        }
        for (line, command) in commands(text) {
            if let Some(problem) = problem(command, &known) {
                stale.push(format!("{path}:{line}: {problem}"));
            }
        }
    }
    require(
        stale.is_empty(),
        format!(
            "live documentation names what the workspace does not build:\n{}",
            stale.join("\n")
        ),
    )
}

/// Each command of a shell code block and each tool table's first cell, with
/// its one-based line number.
fn commands(text: &str) -> Vec<(usize, &str)> {
    let mut commands = Vec::new();
    // Outside a fence: None; inside one: whether its lines are shell commands.
    let mut fence = None;
    let mut continued = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```") {
            fence = match fence {
                Some(_) => None,
                None => Some(
                    info.split([',', ' '])
                        .next()
                        .is_some_and(|language| SHELLS.contains(&language)),
                ),
            };
            continued = false;
            continue;
        }
        match fence {
            Some(true) => {
                // A line continuing the previous one carries arguments only.
                let starts = !continued;
                continued = line.trim_end().ends_with('\\');
                if starts && !trimmed.starts_with('#') {
                    commands.extend(
                        trimmed
                            .split([';', '|', '&'])
                            .map(str::trim)
                            .filter(|segment| !segment.is_empty())
                            .map(|segment| (index + 1, segment)),
                    );
                }
            }
            Some(false) => {}
            None => {
                if let Some(cell) = table_cell(trimmed) {
                    commands.push((index + 1, cell));
                }
            }
        }
    }
    commands
}

/// The code span filling a table row's first cell, as in "| `zetesis-bench` |".
fn table_cell(line: &str) -> Option<&str> {
    let cell = line.strip_prefix('|')?.split('|').next()?.trim();
    cell.strip_prefix('`')?.strip_suffix('`')
}

/// Why a command names what the workspace does not build, if it does.
fn problem(command: &str, known: &BTreeSet<&str>) -> Option<String> {
    let mut words = command
        .split_whitespace()
        .skip_while(|word| *word == "$" || assignment(word));
    let first = words.next()?;
    let name = first.rsplit('/').next().unwrap_or(first);
    if name != "zetesis" && !name.starts_with("zetesis-") {
        return None;
    }
    if !known.contains(name) {
        return Some(format!(
            "`{name}` is not a binary or package of this workspace"
        ));
    }
    // The CLI reserves `bench` only to refuse it with a pointer to the tool.
    (name == "zetesis" && words.next() == Some("bench"))
        .then(|| "`zetesis bench` is not a command; benchmarking is `zetesis-bench`".to_owned())
}

/// Whether `word` is a shell variable assignment such as `NAME=value`.
fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}
