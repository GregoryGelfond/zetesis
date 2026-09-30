//! The ignored tests among the maintained sources and the oracle gate's
//! campaigns over them: which ignored tests each campaign's filters select,
//! and whether a campaign's recorded run ran exactly those.
//!
//! The portable gate reads the selection statically: every clingo test is
//! selected by some campaign. The oracle gate then checks each run against it,
//! so that a filter matching nothing, or a selected test its campaign does not
//! compile, fails the run instead of passing it vacuously.

mod campaigns;
mod sources;

pub use campaigns::{Campaign, campaigns};
pub use sources::{Ignored, Target, ignored};

use std::collections::BTreeSet;

use crate::{Error, require};

/// The names of the ignored tests among `tests` that `campaign` runs: the
/// tests of its package and test targets whose names contain one of its
/// filters. A campaign over several targets is compared by test name across
/// them.
#[must_use]
pub fn selected<'a>(campaign: &Campaign, tests: &'a [Ignored]) -> BTreeSet<&'a str> {
    tests
        .iter()
        .filter(|test| {
            matches!(&test.target, Some(Target::Test(target))
                if campaign.runs(&test.package, target, &test.name))
        })
        .map(|test| test.name.as_str())
        .collect()
}

/// The tests a run's harness output reports running: the name of each
/// `test NAME ... ok` or `test NAME ... FAILED` line.
#[must_use]
pub fn ran(output: &str) -> BTreeSet<&str> {
    output
        .lines()
        .filter_map(|line| {
            let (name, outcome) = line.strip_prefix("test ")?.rsplit_once(" ... ")?;
            matches!(outcome, "ok" | "FAILED").then_some(name)
        })
        .collect()
}

/// Check the oracle gate's recorded runs, `outputs` in the order of
/// `campaigns`: each ran at least one test, and exactly the ignored tests
/// among `tests` its filters select.
///
/// # Errors
/// Returns [`Error::Invalid`] naming every campaign that ran no test, each
/// selected test a campaign did not run and each test it ran that its filters
/// do not select, or a count of runs other than the campaigns'.
pub fn check_runs(
    campaigns: &[Campaign],
    tests: &[Ignored],
    outputs: &[String],
) -> Result<(), Error> {
    require(
        outputs.len() == campaigns.len(),
        format!(
            "{} recorded runs for {} oracle campaigns",
            outputs.len(),
            campaigns.len()
        ),
    )?;
    let mut differences = Vec::new();
    for (index, (campaign, output)) in campaigns.iter().zip(outputs).enumerate() {
        let (number, line) = (index + 1, campaign.line);
        let expected = selected(campaign, tests);
        let observed = ran(output);
        if observed.is_empty() {
            differences.push(format!("campaign {number} (line {line}) ran no test"));
            continue;
        }
        differences.extend(
            expected
                .difference(&observed)
                .map(|name| format!("campaign {number} (line {line}) did not run {name}")),
        );
        differences.extend(observed.difference(&expected).map(|name| {
            format!("campaign {number} (line {line}) ran {name}, which its filters do not select")
        }));
    }
    require(
        differences.is_empty(),
        format!(
            "oracle campaigns ran other tests than they select:\n{}",
            differences.join("\n")
        ),
    )
}
