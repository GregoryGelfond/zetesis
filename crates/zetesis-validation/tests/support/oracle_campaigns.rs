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
    /// A test-name filter written as a positional argument, if any: the
    /// campaign then runs only the tests whose names contain it.
    pub filter: Option<String>,
    /// Whether the harness arguments include `--ignored`; without it the
    /// campaign runs no ignored test.
    pub ignored: bool,
}

impl Campaign {
    /// Whether this campaign runs the ignored test `name` of `target` in
    /// `package`.
    pub(super) fn runs(&self, package: &str, target: &str, name: &str) -> bool {
        self.ignored
            && self.package == package
            && self.targets.iter().any(|named| named == target)
            && self
                .filter
                .as_deref()
                .is_none_or(|filter| name.contains(filter))
    }
}

/// The options of the gate's `cargo test` calls that select nothing.
const NEUTRAL: [&str; 3] = ["--locked", "--no-fail-fast", "--no-default-features"];

/// The campaigns of `script`, in script order. A line is a campaign when its
/// first word is `oracle_test`; its words up to `--` are `cargo test`'s
/// arguments and the rest are the test harness's.
///
/// # Errors
/// Returns a description of the first campaign line whose shape the gate
/// does not use: no package, no test target, two packages or two filters,
/// or an option other than `-p`, `--test` and the neutral ones, such as
/// `--lib`.
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
        let mut filter = None;
        let mut ignored = false;
        let mut harness = false;
        while let Some(word) = words.next() {
            if harness {
                ignored |= word == "--ignored";
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
                    if filter.replace(name.to_owned()).is_some() {
                        return Err(format!("line {line}: two test-name filters"));
                    }
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
            filter,
            ignored,
        });
    }
    Ok(found)
}
