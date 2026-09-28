//! The oracle gate's selection, read from `scripts/check.sh`: each
//! `oracle_test` call is one `cargo test` campaign over the test targets it
//! names, run with `--ignored` for the comparisons the portable gate skips.
//! The gate's list is written by hand; this reading lets the portable gate
//! check it against the sources.

/// One `oracle_test` call of the gate script.
pub(super) struct Campaign {
    /// The script line, from one.
    pub line: usize,
    /// The package `-p` selects.
    pub package: String,
    /// The test targets `--test` selects, in the order written.
    pub targets: Vec<String>,
    /// The test-name filters, in the order written: cargo's positional
    /// argument, then the harness's. With any, the campaign runs only the
    /// tests whose names, module path included, contain one of them.
    pub filters: Vec<String>,
    /// Whether the harness arguments include `--ignored`; without it the
    /// campaign runs no ignored test.
    pub ignored: bool,
}

impl Campaign {
    /// Whether this campaign runs the ignored test `name` of `target` in
    /// `package`, `name` as the target's harness lists it.
    pub(super) fn runs(&self, package: &str, target: &str, name: &str) -> bool {
        self.ignored
            && self.package == package
            && self.targets.iter().any(|named| named == target)
            && (self.filters.is_empty() || self.filters.iter().any(|filter| name.contains(filter)))
    }
}

/// The options of the gate's `cargo test` calls that select nothing.
const NEUTRAL: [&str; 3] = ["--locked", "--no-fail-fast", "--no-default-features"];

/// The harness options the gate's campaigns use; neither selects a test.
const HARNESS: [&str; 2] = ["--ignored", "--nocapture"];

/// The campaigns of `script`, in script order. A line is a campaign when its
/// first word is `oracle_test`; its words up to `--` are `cargo test`'s
/// arguments and the rest are the test harness's. Cargo passes its one
/// positional argument to the harness as a filter, and the harness takes
/// further filters of its own.
///
/// # Errors
/// Returns a description of the first campaign line whose shape the gate
/// does not use: no package, no test target, two packages or two cargo
/// filters, an option other than `-p`, `--test` and the neutral ones, such as
/// `--lib`, or a harness option other than `--ignored` and `--nocapture`,
/// such as `--exact`.
pub(super) fn campaigns(script: &str) -> Result<Vec<Campaign>, String> {
    let mut found = Vec::new();
    for (index, text) in script.lines().enumerate() {
        let line = index + 1;
        let mut words = text.split_whitespace();
        if words.next() != Some("oracle_test") {
            continue;
        }
        let mut package = None;
        let mut targets = Vec::new();
        let mut filters = Vec::new();
        let mut cargo_filter = false;
        let mut ignored = false;
        let mut harness = false;
        while let Some(word) = words.next() {
            if harness {
                if !word.starts_with('-') {
                    filters.push(word.to_owned());
                } else if HARNESS.contains(&word) {
                    ignored |= word == "--ignored";
                } else {
                    return Err(format!(
                        "line {line}: the oracle gate does not pass {word} to the harness"
                    ));
                }
                continue;
            }
            match word {
                "--" => harness = true,
                "-p" | "--package" => {
                    let name = words
                        .next()
                        .ok_or_else(|| format!("line {line}: -p names no package"))?;
                    if package.replace(name.to_owned()).is_some() {
                        return Err(format!("line {line}: two packages"));
                    }
                }
                "--test" => {
                    let name = words
                        .next()
                        .ok_or_else(|| format!("line {line}: --test names no target"))?;
                    targets.push(name.to_owned());
                }
                neutral if NEUTRAL.contains(&neutral) => {}
                option if option.starts_with('-') => {
                    return Err(format!(
                        "line {line}: the oracle gate does not use {option}"
                    ));
                }
                name => {
                    if cargo_filter {
                        return Err(format!("line {line}: two test-name filters before --"));
                    }
                    cargo_filter = true;
                    filters.push(name.to_owned());
                }
            }
        }
        let package = package.ok_or_else(|| format!("line {line}: no package"))?;
        if targets.is_empty() {
            return Err(format!("line {line}: no test target"));
        }
        found.push(Campaign {
            line,
            package,
            targets,
            filters,
            ignored,
        });
    }
    Ok(found)
}
