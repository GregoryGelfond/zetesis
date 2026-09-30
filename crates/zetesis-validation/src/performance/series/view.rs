//! Derived comparison of published matrix reports over the same cells.
//!
//! The view reads the retained records and computes exact integer medians of
//! the timed native and reference intervals per cell and profile, and the
//! counters the native records carry; its tables divide those medians between
//! reports in the order given and against the reference, exactly, and its JSON
//! publishes the medians alone. For each report and profile it also keeps a
//! scoreboard against the reference solver: the cells where both passed,
//! which of them the native solver decided faster, and each cell's time
//! split into grounding, candidate proposal and membership on the native
//! side and grounding and solving on the reference's, with the peak
//! resident set of each when the campaign ran memory rounds. It never pools
//! reports, never averages a cell that did not pass, and retains each
//! report's native executable seal so that a published comparison names
//! what it compared.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde_json::Value;

use super::super::matrix::Qualification;

/// A published report under the name it will carry in the comparison.
#[derive(Clone, Copy, Debug)]
pub struct Labelled<'a> {
    /// Column name, for example `main`, `before` or `after`.
    pub label: &'a str,
    /// The published matrix report document (`{"passed", "accounted", "report"}`).
    pub report: &'a Value,
}

/// A refused comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// No report was given.
    Empty,
    /// Two reports carry the same label.
    Label {
        /// The repeated label.
        label: String,
    },
    /// The reports do not measure the same cells in the same order.
    Cells {
        /// The label whose cells differ from the first report's.
        label: String,
    },
    /// The reports do not request the same profiles in the same order.
    Profiles {
        /// The label whose profiles differ from the first report's.
        label: String,
    },
    /// A report's profiles do not request the same search method, so it
    /// has no one method to stand on its scoreboards.
    Methods {
        /// The label of the report.
        label: String,
    },
    /// Qualification-only evidence has no native timed population to compare.
    NoTimedPopulation {
        /// The label of the report.
        label: String,
    },
    /// A report lacks a field the view reads.
    Malformed {
        /// The label of the report.
        label: String,
        /// The missing or ill-typed field.
        field: &'static str,
    },
}
impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no reports to compare"),
            Self::Label { label } => write!(f, "report label {label:?} is repeated"),
            Self::Cells { label } => write!(f, "report {label:?} measures different cells"),
            Self::Profiles { label } => {
                write!(f, "report {label:?} requests different profiles")
            }
            Self::Methods { label } => {
                write!(f, "report {label:?} requests different search methods")
            }
            Self::NoTimedPopulation { label } => {
                write!(
                    f,
                    "report {label:?} has no native timed population to compare"
                )
            }
            Self::Malformed { label, field } => {
                write!(f, "report {label:?} lacks a readable {field}")
            }
        }
    }
}
impl std::error::Error for ViewError {}

/// Exact summary of one population of timed intervals, in nanoseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Timing {
    /// Number of timed intervals.
    pub samples: usize,
    /// Smallest interval.
    pub minimum_ns: u64,
    /// Exact median: the middle interval, or the mean of the two middle ones
    /// rounded down.
    pub median_ns: u64,
    /// Largest interval.
    pub maximum_ns: u64,
}

/// The reference solver's record for one cell in one report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reference {
    /// Timed wall intervals.
    #[serde(flatten)]
    pub timing: Timing,
    /// Median of the reference's own total less its solving time, from the
    /// report it prints: grounding and preprocessing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_ns: Option<u64>,
    /// Median of the reference's own solving time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solving_ns: Option<u64>,
    /// Median peak resident set over the memory rounds, bytes; absent when
    /// the campaign ran none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_rss_bytes: Option<u64>,
}

/// The native intervals summed into the three parts a comparison against
/// the reference reads, each the median over the timed records that
/// measured it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Breakdown {
    /// The grounding stage, nanoseconds; absent under lazy grounding, which
    /// grounds within membership, and when no record measured it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding: Option<u64>,
    /// Candidate setup and generation, nanoseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<u64>,
    /// Membership, nanoseconds: certificate setup and checks, closure and
    /// exact reduct membership, reduct preparation, original validation and
    /// the device's host oracle, and full-answer reconstruction when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership: Option<u64>,
}

/// The phases each part of the breakdown sums.
const PROPOSAL_PHASES: [&str; 2] = ["candidate_setup", "candidate_generation"];
const MEMBERSHIP_PHASES: [&str; 8] = [
    "certificate_setup",
    "certified_membership",
    "closure_membership",
    "exact_reduct_membership",
    "reduct_preparation",
    "original_validation",
    "gpu_host_oracle",
    "answer_reconstruction",
];

/// One native profile's record for one cell in one report.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Native {
    /// Every timed position passed.
    Passed(Passed),
    /// At least one timed position did not pass: the decisions and their counts.
    NotPassed {
        /// Decision name to number of positions.
        decisions: BTreeMap<String, usize>,
        /// Retained causes, including the attempted phase and original blocker.
        reasons: BTreeMap<String, usize>,
    },
}

/// A passed cell's timing and the counters its retained records carry.
#[derive(Clone, Debug, Serialize)]
pub struct Passed {
    /// Timed wall intervals.
    #[serde(flatten)]
    pub timing: Timing,
    /// Median of the native driver interval, when the record carries it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_median_ns: Option<u64>,
    /// Models the solver published, from the first timed record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_models: Option<u64>,
    /// Candidates the solver examined, from the first timed record's text
    /// statistics; absent when the route does not report them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_examined: Option<u64>,
    /// Charged work, from the first timed record's typed statistics: the
    /// formula route's search work, or the independent closure route's work
    /// summed over its completed checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_work: Option<u64>,
    /// Median of each measured phase's interval.
    pub phases: BTreeMap<String, PhaseTiming>,
    /// The intervals summed into grounding, proposal and membership.
    pub breakdown: Breakdown,
    /// Median peak resident set over the memory rounds, bytes; absent when
    /// the campaign ran none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_rss_bytes: Option<u64>,
    /// Device memory the first timed record's formula execution accounted,
    /// bytes; absent when no device formula execution reported it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_bytes: Option<u64>,
}

/// One configuration's standing against the reference solver: the cells
/// where the native profile and the reference both passed, and which of
/// them the native solver decided faster.
#[derive(Clone, Debug, Serialize)]
pub struct Scoreboard {
    /// The report's label.
    pub report: String,
    /// The requested profile's index.
    pub profile: usize,
    /// The report's formula search method, the same on each of its
    /// profiles, or `default`.
    pub method: String,
    /// Cells where both passed.
    pub compared: usize,
    /// Compared cells whose native median is below the reference median.
    pub wins: usize,
    /// Every compared cell, fastest ratio first.
    pub verdicts: Vec<Verdict>,
}

/// One compared cell on a scoreboard.
#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    /// The cell's label.
    pub cell: String,
    /// Native median wall interval.
    pub native_ns: u64,
    /// Reference median wall interval.
    pub reference_ns: u64,
    /// The native intervals by part.
    pub native: Breakdown,
    /// The reference's own grounding and preprocessing time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_grounding_ns: Option<u64>,
    /// The reference's own solving time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_solving_ns: Option<u64>,
    /// Native peak resident set, bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_peak_rss_bytes: Option<u64>,
    /// Reference peak resident set, bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_peak_rss_bytes: Option<u64>,
    /// Device memory the native execution accounted, bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_bytes: Option<u64>,
}

/// Median interval of one named phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PhaseTiming {
    /// Exact median over the timed records that measured the phase.
    pub median_ns: u64,
}

/// One profile's records across the reports.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileRow {
    /// The requested profile, as the reports serialize it.
    pub profile: Value,
    /// Records by report label.
    pub reports: BTreeMap<String, Native>,
}

/// One cell across the reports.
#[derive(Clone, Debug, Serialize)]
pub struct Cell {
    /// The workload's entry path.
    pub entry: String,
    /// A short distinct name: the generated program's stem, or the entry's
    /// last two path components with the recorded constant edit of an
    /// amended entry.
    pub label: String,
    /// One row per requested native profile.
    pub profiles: Vec<ProfileRow>,
    /// The reference solver's timing and its own split by report label,
    /// where it passed.
    pub reference: BTreeMap<String, Reference>,
    /// What qualified the cell's answer family in each report, by label:
    /// clingo's census where clingo took part, the workload's recorded
    /// contract in a clingo-free report, or nothing where that report's
    /// campaign needed clingo for the cell.
    pub qualification: BTreeMap<String, Qualification>,
    /// Non-pass samples across all scheduled phases, grouped by report and
    /// producer, phase and retained reason. Native profile indices are zero-based.
    pub failure_reasons: BTreeMap<String, BTreeMap<String, usize>>,
}

/// What a report is: its native seal and its own verdicts.
#[derive(Clone, Debug, Serialize)]
pub struct Provenance {
    /// SHA-256 of the native executable the report sealed.
    pub native_sha256: String,
    /// SHA-256 of the reference executable the report sealed; none for a
    /// clingo-free campaign, which seals no reference.
    pub reference_sha256: Option<String>,
    /// SHA-256 of the corpus manifest the report sealed.
    pub manifest_sha256: String,
    /// When the campaign started, Unix nanoseconds.
    pub started_unix_ns: u64,
    /// When the campaign finished, Unix nanoseconds; absent if it did not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_unix_ns: Option<u64>,
    /// The report's own `passed` verdict.
    pub passed: bool,
    /// The report's own `accounted` verdict.
    pub accounted: bool,
}

/// The derived comparison, serializable as a retained observation.
#[derive(Clone, Debug, Serialize)]
pub struct Comparison {
    /// Report labels in the order given.
    pub labels: Vec<String>,
    /// Cells in schedule order.
    pub cells: Vec<Cell>,
    /// Per-report provenance.
    pub provenance: BTreeMap<String, Provenance>,
    /// The formula search method each report requested, by label, where a
    /// report requested one; the executable's default otherwise. Reports may
    /// differ in this field alone, since the methods are compared on the
    /// same cells by design. A report written before the `search` field
    /// carries the method as `candidates`, and one campaign's reports name
    /// the region workers beside it as `region_workers`; both are read as
    /// the method and left out of the profile comparison.
    pub methods: BTreeMap<String, String>,
    /// Each report's and profile's standing against the reference, in report
    /// order and then profile order; a report whose campaign ran without
    /// clingo has none.
    pub scoreboards: Vec<Scoreboard>,
}

/// Compare published reports over the same cells and profiles. The formula
/// search method is the one profile field the reports may differ in, under
/// any of its spellings ([`Comparison::methods`]). Cells must have the same
/// ordered entry paths, sealed corpus manifest and explicit workload content
/// identities. A derived report must retain one workload identity per cell;
/// a corpus report without workloads compares only with other corpus reports.
///
/// # Errors
/// Refuses an empty list, repeated labels, reports whose cells or profiles
/// differ, a report whose profiles disagree on the search method, and
/// reports lacking the fields the view reads, or qualification evidence without
/// a native timed population.
pub fn compare(reports: &[Labelled<'_>]) -> Result<Comparison, ViewError> {
    let first = reports.first().ok_or(ViewError::Empty)?;
    let entries = cases(first)?;
    let workloads = workload_identities(first, &entries)?;
    let manifest = provenance(first)?.manifest_sha256;
    let requested = profiles(first)?;
    let mut labels = Vec::with_capacity(reports.len());
    let mut provenance = BTreeMap::new();
    let mut methods = BTreeMap::new();
    for labelled in reports {
        if !samples(labelled)?.iter().any(|sample| {
            sample["slot"]["phase"] == "timed" && sample["slot"]["producer"]["solver"] == "native"
        }) {
            return Err(ViewError::NoTimedPopulation {
                label: labelled.label.into(),
            });
        }
        if labels.contains(&labelled.label.to_owned()) {
            return Err(ViewError::Label {
                label: labelled.label.into(),
            });
        }
        let recorded = self::provenance(labelled)?;
        if cases(labelled)? != entries
            || workload_identities(labelled, &entries)? != workloads
            || recorded.manifest_sha256 != manifest
        {
            return Err(ViewError::Cells {
                label: labelled.label.into(),
            });
        }
        if profiles(labelled)? != requested {
            return Err(ViewError::Profiles {
                label: labelled.label.into(),
            });
        }
        labels.push(labelled.label.to_owned());
        provenance.insert(labelled.label.to_owned(), recorded);
        methods.insert(labelled.label.to_owned(), method(labelled)?);
    }
    let workloads = first.report["report"]["workloads"].as_array();
    let mut cells = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let label = label(entry, workloads.and_then(|workloads| workloads.get(index)));
        let mut reference = BTreeMap::new();
        let mut qualification = BTreeMap::new();
        let mut failure_reasons = BTreeMap::new();
        for labelled in reports {
            if let Some(record) = self::reference(labelled, index)? {
                reference.insert(labelled.label.to_owned(), record);
            }
            qualification.insert(
                labelled.label.to_owned(),
                self::qualification(labelled, index)?,
            );
            let reasons = self::failure_reasons(labelled, index)?;
            if !reasons.is_empty() {
                failure_reasons.insert(labelled.label.to_owned(), reasons);
            }
        }
        let mut rows = Vec::with_capacity(requested.len());
        for (profile, request) in requested.iter().enumerate() {
            let mut records = BTreeMap::new();
            for labelled in reports {
                records.insert(labelled.label.to_owned(), native(labelled, index, profile)?);
            }
            rows.push(ProfileRow {
                profile: request.clone(),
                reports: records,
            });
        }
        cells.push(Cell {
            entry: entry.clone(),
            label,
            profiles: rows,
            reference,
            qualification,
            failure_reasons,
        });
    }
    // A scoreboard stands a report against clingo, so a clingo-free report has none.
    let with_clingo: Vec<String> = labels
        .iter()
        .filter(|label| provenance[*label].reference_sha256.is_some())
        .cloned()
        .collect();
    let scoreboards = scoreboards(&with_clingo, &methods, &cells);
    Ok(Comparison {
        labels,
        cells,
        provenance,
        methods,
        scoreboards,
    })
}

/// One scoreboard per report in `labels` and per profile, over the cells
/// where both passed.
fn scoreboards(
    labels: &[String],
    methods: &BTreeMap<String, String>,
    cells: &[Cell],
) -> Vec<Scoreboard> {
    let profiles = cells.first().map_or(0, |cell| cell.profiles.len());
    let mut scoreboards = Vec::with_capacity(labels.len() * profiles);
    for label in labels {
        for profile in 0..profiles {
            let mut verdicts: Vec<Verdict> = cells
                .iter()
                .filter_map(|cell| {
                    let Some(Native::Passed(passed)) = cell.profiles[profile].reports.get(label)
                    else {
                        return None;
                    };
                    let reference = cell.reference.get(label)?;
                    Some(verdict(&cell.label, passed, reference))
                })
                .collect();
            verdicts.sort_by_key(|verdict| {
                Ratio::against_reference(verdict.native_ns, verdict.reference_ns)
            });
            scoreboards.push(Scoreboard {
                report: label.clone(),
                profile,
                method: methods
                    .get(label)
                    .cloned()
                    .unwrap_or_else(|| "default".to_owned()),
                compared: verdicts.len(),
                wins: verdicts
                    .iter()
                    .filter(|verdict| verdict.native_ns < verdict.reference_ns)
                    .count(),
                verdicts,
            });
        }
    }
    scoreboards
}

/// One median interval over another, exact: ordered by cross-multiplication
/// and shown to three decimals, rounded half up by integer arithmetic, as
/// `milliseconds` shows an interval.
#[derive(Clone, Copy, Debug)]
struct Ratio {
    numerator: u64,
    denominator: u64,
}

impl Ratio {
    /// A native median over the reference's; a reference median of zero reads
    /// as one nanosecond, so the ratio is always a number.
    fn against_reference(native_ns: u64, reference_ns: u64) -> Self {
        Self {
            numerator: native_ns,
            denominator: reference_ns.max(1),
        }
    }
    /// A later report's median over an earlier one's, when the earlier is positive.
    fn between(later_ns: u64, earlier_ns: u64) -> Option<Self> {
        (earlier_ns > 0).then_some(Self {
            numerator: later_ns,
            denominator: earlier_ns,
        })
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        // Each product of two u64 values fits in a u128.
        (u128::from(self.numerator) * u128::from(other.denominator))
            .cmp(&(u128::from(other.numerator) * u128::from(self.denominator)))
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Ratio {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Ratio {}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Thousandths rounded half up, floor((2000 n + d) / 2d), exact in u128.
        let numerator = u128::from(self.numerator);
        let denominator = u128::from(self.denominator);
        let thousandths = (2_000 * numerator + denominator) / (2 * denominator);
        write!(f, "{}.{:03}", thousandths / 1_000, thousandths % 1_000)
    }
}

fn verdict(cell: &str, passed: &Passed, reference: &Reference) -> Verdict {
    Verdict {
        cell: cell.to_owned(),
        native_ns: passed.timing.median_ns,
        reference_ns: reference.timing.median_ns,
        native: passed.breakdown.clone(),
        reference_grounding_ns: reference.grounding_ns,
        reference_solving_ns: reference.solving_ns,
        native_peak_rss_bytes: passed.peak_rss_bytes,
        reference_peak_rss_bytes: reference.peak_rss_bytes,
        device_bytes: passed.device_bytes,
    }
}

impl Comparison {
    /// Markdown tables: per profile, the median [minimum, maximum] in
    /// milliseconds of every cell in every report with the ratios; then the
    /// reference solver; then the counters of the last report; then each
    /// report's and profile's scoreboard against the reference, with the
    /// wins and the losses split by part and the peak memory where the
    /// campaign measured it.
    #[must_use]
    pub fn markdown(&self) -> String {
        Markdown(self).to_string()
    }
}

struct Markdown<'a>(&'a Comparison);

/// The profile line of a table, and the search method of each report when
/// any report requested one.
fn heading(f: &mut fmt::Formatter<'_>, comparison: &Comparison, profile: &Value) -> fmt::Result {
    writeln!(
        f,
        "Native wall time, ms: median [minimum, maximum] of the timed intervals; profile {}.\n",
        Profile(profile)
    )?;
    if comparison
        .methods
        .values()
        .any(|method| method != "default")
    {
        write!(f, "Formula search method by report:")?;
        for label in &comparison.labels {
            let method = comparison
                .methods
                .get(label)
                .map_or("default", String::as_str);
            write!(f, " {label}={method};")?;
        }
        writeln!(f, "\n")?;
    }
    Ok(())
}

impl fmt::Display for Markdown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let comparison = self.0;
        let last = comparison
            .labels
            .last()
            .map(String::as_str)
            .unwrap_or_default();
        let profile_count = comparison
            .cells
            .first()
            .map_or(0, |cell| cell.profiles.len());
        for profile in 0..profile_count {
            let first = &comparison.cells[0].profiles[profile];
            heading(f, comparison, &first.profile)?;
            let pairs = report_pairs(&comparison.labels);
            write!(f, "| Cell |")?;
            for label in &comparison.labels {
                write!(f, " {label} |")?;
            }
            for (earlier, later) in &pairs {
                write!(f, " {later}/{earlier} |")?;
            }
            for label in &comparison.labels {
                write!(f, " {label}/reference |")?;
            }
            write!(f, "\n|---|")?;
            for _ in 0..2 * comparison.labels.len() + pairs.len() {
                write!(f, "---:|")?;
            }
            writeln!(f)?;
            for cell in &comparison.cells {
                let row = &cell.profiles[profile];
                write!(f, "| {} |", cell.label)?;
                for label in &comparison.labels {
                    write!(f, " {} |", native_cell(row.reports.get(label)))?;
                }
                for (earlier, later) in &pairs {
                    let ratio = passed_median(row.reports.get(*later))
                        .zip(passed_median(row.reports.get(*earlier)))
                        .and_then(|(later, earlier)| Ratio::between(later, earlier));
                    match ratio {
                        Some(ratio) => write!(f, " {ratio} |")?,
                        None => write!(f, " n/a |")?,
                    }
                }
                for label in &comparison.labels {
                    let ratio = passed_median(row.reports.get(label))
                        .zip(cell.reference.get(label))
                        .map(|(native, reference)| {
                            Ratio::against_reference(native, reference.timing.median_ns)
                        });
                    match ratio {
                        Some(ratio) => write!(f, " {ratio} |")?,
                        None if !clingo_ran(comparison, label) => write!(f, " not run |")?,
                        None => write!(f, " n/a |")?,
                    }
                }
                writeln!(f)?;
            }
            writeln!(f)?;
        }
        reference_table(f, comparison)?;
        writeln!(
            f,
            "\nCounters of report {last}: published models, candidates examined, charged search work, driver median ms.\n\n| Cell | profile | models | candidates | work | driver ms |\n|---|---|---:|---:|---:|---:|"
        )?;
        for cell in &comparison.cells {
            for (index, row) in cell.profiles.iter().enumerate() {
                write!(f, "| {} | {index} |", cell.label)?;
                match row.reports.get(last) {
                    Some(Native::Passed(passed)) => writeln!(
                        f,
                        " {} | {} | {} | {} |",
                        optional(passed.published_models),
                        optional(passed.candidates_examined),
                        optional(passed.search_work),
                        passed
                            .driver_median_ns
                            .map_or_else(|| "n/a".to_owned(), milliseconds)
                    )?,
                    Some(Native::NotPassed { decisions, reasons }) => {
                        writeln!(f, " {} | | | |", non_pass_text(decisions, reasons))?;
                    }
                    None => writeln!(f, " n/a | | | |")?,
                }
            }
        }
        for scoreboard in &comparison.scoreboards {
            scoreboard_tables(f, comparison, scoreboard)?;
        }
        failure_table(f, comparison)?;
        Ok(())
    }
}

/// Clingo's wall time per cell and report: "not passed" where it failed,
/// "not run" where the report's campaign ran without it.
fn reference_table(f: &mut fmt::Formatter<'_>, comparison: &Comparison) -> fmt::Result {
    write!(f, "Reference wall time, ms, same notation.\n\n| Cell |")?;
    for label in &comparison.labels {
        write!(f, " {label} |")?;
    }
    write!(f, "\n|---|")?;
    for _ in &comparison.labels {
        write!(f, "---:|")?;
    }
    writeln!(f)?;
    for cell in &comparison.cells {
        write!(f, "| {} |", cell.label)?;
        for label in &comparison.labels {
            match cell.reference.get(label) {
                Some(record) => write!(f, " {} |", timing_cell(&record.timing))?,
                None if !clingo_ran(comparison, label) => write!(f, " not run |")?,
                None => write!(f, " not passed |")?,
            }
        }
        writeln!(f)?;
    }
    Ok(())
}

/// Whether clingo took part in the campaign of the report labelled `label`.
fn clingo_ran(comparison: &Comparison, label: &str) -> bool {
    comparison
        .provenance
        .get(label)
        .is_some_and(|provenance| provenance.reference_sha256.is_some())
}

/// Every retained non-pass phase, independently of the timed population.
fn failure_table(f: &mut fmt::Formatter<'_>, comparison: &Comparison) -> fmt::Result {
    if comparison
        .cells
        .iter()
        .all(|cell| cell.failure_reasons.is_empty())
    {
        return Ok(());
    }
    writeln!(
        f,
        "\nRecorded failure reasons across all scheduled phases.\n\n| Report | Cell | Producer, phase and reason | Positions |\n|---|---|---|---:|"
    )?;
    for cell in &comparison.cells {
        for label in &comparison.labels {
            if let Some(reasons) = cell.failure_reasons.get(label) {
                for (reason, count) in reasons {
                    writeln!(
                        f,
                        "| {} | {} | {} | {count} |",
                        markdown_text(label),
                        markdown_text(&cell.label),
                        markdown_text(reason),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// One scoreboard: its sentence, its wins, its losses and its peak memory.
fn scoreboard_tables(
    f: &mut fmt::Formatter<'_>,
    comparison: &Comparison,
    scoreboard: &Scoreboard,
) -> fmt::Result {
    let profile = comparison
        .cells
        .first()
        .map(|cell| Profile(&cell.profiles[scoreboard.profile].profile).to_string())
        .unwrap_or_default();
    write!(
        f,
        "\nAgainst the reference: report {}, profile {} ({profile}; search {}): ",
        scoreboard.report, scoreboard.profile, scoreboard.method
    )?;
    if scoreboard.compared == 0 {
        writeln!(f, "no cell where both passed.")?;
        return Ok(());
    }
    let tenths = scoreboard.wins * 1000 / scoreboard.compared;
    writeln!(
        f,
        "faster on {} of {} cells where both passed ({}.{}%).",
        scoreboard.wins,
        scoreboard.compared,
        tenths / 10,
        tenths % 10
    )?;
    let (wins, losses): (Vec<&Verdict>, Vec<&Verdict>) = scoreboard
        .verdicts
        .iter()
        .partition(|verdict| verdict.native_ns < verdict.reference_ns);
    for (name, verdicts) in [
        ("Wins, fastest first", wins),
        ("Losses, closest first", losses),
    ] {
        if verdicts.is_empty() {
            writeln!(f, "\n{name}: none.")?;
            continue;
        }
        writeln!(
            f,
            "\n{name}. Milliseconds: the native and the reference medians and their ratio; the native split into grounding, candidate proposal and membership; the reference's into grounding and solving from its own report.\n\n| Cell | native | reference | native/reference | grounding | proposal | membership | reference grounding | reference solving |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|"
        )?;
        for verdict in verdicts {
            writeln!(
                f,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                verdict.cell,
                milliseconds(verdict.native_ns),
                milliseconds(verdict.reference_ns),
                Ratio::against_reference(verdict.native_ns, verdict.reference_ns),
                optional_ms(verdict.native.grounding),
                optional_ms(verdict.native.proposal),
                optional_ms(verdict.native.membership),
                optional_ms(verdict.reference_grounding_ns),
                optional_ms(verdict.reference_solving_ns),
            )?;
        }
    }
    if scoreboard.verdicts.iter().any(|verdict| {
        verdict.native_peak_rss_bytes.is_some()
            || verdict.reference_peak_rss_bytes.is_some()
            || verdict.device_bytes.is_some()
    }) {
        writeln!(
            f,
            "\nPeak memory, MiB: the resident set of the native and of the reference over the memory rounds, and the device memory the native accounted.\n\n| Cell | native | reference | device |\n|---|---:|---:|---:|"
        )?;
        for verdict in &scoreboard.verdicts {
            writeln!(
                f,
                "| {} | {} | {} | {} |",
                verdict.cell,
                optional_mib(verdict.native_peak_rss_bytes),
                optional_mib(verdict.reference_peak_rss_bytes),
                optional_mib(verdict.device_bytes),
            )?;
        }
    }
    Ok(())
}

fn optional_ms(value: Option<u64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), milliseconds)
}

// Bytes to mebibytes with one decimal, rounded down.
fn optional_mib(value: Option<u64>) -> String {
    value.map_or_else(
        || "n/a".to_owned(),
        |bytes| {
            let tenths = bytes / (1024 * 1024 / 10);
            format!("{}.{}", tenths / 10, tenths % 10)
        },
    )
}

/// A requested profile in one line.
struct Profile<'a>(&'a Value);

impl fmt::Display for Profile<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = |name: &str| match self.0[name].as_str() {
            Some(text) => text.to_owned(),
            None => self.0[name].to_string(),
        };
        write!(
            f,
            "backend={}, grounder={}, oracle={}, workers={}, completion workers={}, batch={}",
            field("backend"),
            field("grounder"),
            field("oracle"),
            field("workers"),
            field("completion_workers"),
            field("batch_size")
        )?;
        if let Some(seconds) = self.0["time_limit_seconds"].as_u64() {
            write!(f, ", time limit={seconds} s")?;
        }
        if let Some(work) = self.0["max_expansion_work"].as_u64() {
            write!(f, ", expansion work limit={work}")?;
        }
        Ok(())
    }
}

fn label(entry: &str, workload: Option<&Value>) -> String {
    let edits = workload
        .filter(|workload| workload["amended"] == true)
        .and_then(|workload| workload["sources"].as_array())
        .into_iter()
        .flatten()
        .flat_map(|source| source["edits"].as_array().into_iter().flatten())
        .filter_map(|edit| Some((edit["before"].as_str()?, edit["after"].as_str()?)));
    super::super::matrix::workload_label(
        entry,
        workload.is_some_and(|workload| workload.get("generated").is_some()),
        edits,
    )
}

fn native_cell(record: Option<&Native>) -> String {
    match record {
        Some(Native::Passed(passed)) => timing_cell(&passed.timing),
        Some(Native::NotPassed { decisions, reasons }) => non_pass_text(decisions, reasons),
        None => "n/a".to_owned(),
    }
}

fn decisions_text(decisions: &BTreeMap<String, usize>) -> String {
    decisions
        .iter()
        .map(|(decision, count)| format!("{decision} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn non_pass_text(decisions: &BTreeMap<String, usize>, reasons: &BTreeMap<String, usize>) -> String {
    markdown_text(&format!(
        "{}; {}",
        decisions_text(decisions),
        decisions_text(reasons)
    ))
}

/// A retained diagnostic is text, never Markdown or a new table row.
fn markdown_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\\' | '|' | '`' | '*' | '_' | '[' | ']' => {
                escaped.push('\\');
                escaped.push(character);
            }
            character if character.is_control() => escaped.extend(character.escape_default()),
            character => escaped.push(character),
        }
    }
    escaped
}

fn timing_cell(timing: &Timing) -> String {
    format!(
        "{} [{}, {}]",
        milliseconds(timing.median_ns),
        milliseconds(timing.minimum_ns),
        milliseconds(timing.maximum_ns)
    )
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |value| value.to_string())
}

// Nanoseconds to milliseconds with three decimals, rounded half up at the
// microsecond, by integer arithmetic.
fn milliseconds(nanoseconds: u64) -> String {
    let microseconds = (nanoseconds + 500) / 1_000;
    format!("{}.{:03}", microseconds / 1_000, microseconds % 1_000)
}

/// The reports the tables divide, as `(earlier, later)`: each report after the
/// first over its predecessor, and the last over the first when there are more
/// than two.
fn report_pairs(labels: &[String]) -> Vec<(&String, &String)> {
    let mut pairs: Vec<(&String, &String)> = labels.windows(2).map(|w| (&w[0], &w[1])).collect();
    if labels.len() > 2 {
        pairs.push((&labels[0], &labels[labels.len() - 1]));
    }
    pairs
}

/// The median interval of a native record that passed.
fn passed_median(record: Option<&Native>) -> Option<u64> {
    match record {
        Some(Native::Passed(passed)) => Some(passed.timing.median_ns),
        _ => None,
    }
}

fn cases(labelled: &Labelled<'_>) -> Result<Vec<String>, ViewError> {
    labelled.report["report"]["cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .map(|case| case.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or(ViewError::Malformed {
            label: labelled.label.into(),
            field: "report.cases",
        })
}

/// Content identities distinguish amendments that deliberately share an
/// entry path. Schema 2 always carries them; older corpus reports do not.
fn workload_identities<'a>(
    labelled: &Labelled<'a>,
    entries: &[String],
) -> Result<Option<Vec<&'a str>>, ViewError> {
    let malformed = |field| ViewError::Malformed {
        label: labelled.label.into(),
        field,
    };
    let Some(raw) = labelled.report["report"].get("workloads") else {
        return if labelled.report["report"]["schema"] == 2 {
            Err(malformed("report.workloads"))
        } else {
            Ok(None)
        };
    };
    let workloads = raw
        .as_array()
        .ok_or_else(|| malformed("report.workloads"))?;
    if workloads.len() != entries.len() {
        return Err(malformed("one workload per report.cases entry"));
    }
    workloads
        .iter()
        .zip(entries)
        .map(|(workload, entry)| {
            if workload["entry"].as_str() != Some(entry.as_str()) {
                return Err(malformed("workload.entry matching report.cases"));
            }
            workload["identity"]
                .as_str()
                .filter(|identity| {
                    identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .ok_or_else(|| malformed("workload.identity SHA-256"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// The fields that spell a profile's search method, left out of the profile
/// comparison: `search`, the method's name; `candidates`, its name in
/// reports written before `search`; and `region_workers`, the workers one
/// campaign's reports named beside it. A profile without any of them
/// requested the executable's default.
const METHOD_FIELDS: [&str; 3] = ["search", "candidates", "region_workers"];

fn profiles(labelled: &Labelled<'_>) -> Result<Vec<Value>, ViewError> {
    let mut profiles = labelled.report["report"]["plan"]["profiles"]
        .as_array()
        .cloned()
        .ok_or(ViewError::Malformed {
            label: labelled.label.into(),
            field: "report.plan.profiles",
        })?;
    for profile in &mut profiles {
        if let Some(fields) = profile.as_object_mut() {
            for field in METHOD_FIELDS {
                fields.remove(field);
            }
        }
    }
    Ok(profiles)
}

/// The search method the report's profiles requested, or the executable's
/// default; the profiles must agree on it.
fn method(labelled: &Labelled<'_>) -> Result<String, ViewError> {
    let profiles = labelled.report["report"]["plan"]["profiles"]
        .as_array()
        .ok_or(ViewError::Malformed {
            label: labelled.label.into(),
            field: "report.plan.profiles",
        })?;
    let mut methods = profiles.iter().map(profile_method);
    let Some(method) = methods.next() else {
        return Ok("default".to_owned());
    };
    if methods.any(|other| other != method) {
        return Err(ViewError::Methods {
            label: labelled.label.into(),
        });
    }
    Ok(method)
}

/// The method one profile spells, under any of the method's spellings, the
/// fields `METHOD_FIELDS` names.
fn profile_method(profile: &Value) -> String {
    let method = ["search", "candidates"]
        .iter()
        .find_map(|field| profile[field].as_str())
        .unwrap_or("default");
    match profile["region_workers"].as_u64() {
        Some(workers) => format!("{method} with {workers} workers"),
        None => method.to_owned(),
    }
}

/// Whether the report's campaign ran without clingo, as its plan records.
fn clingo_free(labelled: &Labelled<'_>) -> bool {
    labelled.report["report"]["plan"]["reference_policy"].as_str() == Some("clingo_free")
}

/// What qualified a case in one report: clingo where it took part; without
/// it, the recorded contract, unless the campaign recorded that the case
/// needs clingo.
fn qualification(labelled: &Labelled<'_>, case: usize) -> Result<Qualification, ViewError> {
    if !clingo_free(labelled) {
        return Ok(Qualification::Clingo);
    }
    let needs_clingo = samples(labelled)?
        .iter()
        .any(|sample| sample["slot"]["case"] == case && sample["decision"] == "needs_clingo");
    Ok(if needs_clingo {
        Qualification::NeedsClingo
    } else {
        Qualification::Contract
    })
}

fn provenance(labelled: &Labelled<'_>) -> Result<Provenance, ViewError> {
    let malformed = |field| ViewError::Malformed {
        label: labelled.label.into(),
        field,
    };
    // The campaign seals the native executable, the reference executable when
    // clingo takes part, and the manifest first, in that order, before the
    // sources. The recorded policy says whether clingo took part.
    let seal = |index: usize, field| {
        labelled.report["report"]["before"]
            .as_array()
            .and_then(|seals| seals.get(index))
            .and_then(|seal| seal["sha256"].as_str())
            .map(str::to_owned)
            .ok_or(malformed(field))
    };
    let clingo_free = clingo_free(labelled);
    Ok(Provenance {
        native_sha256: seal(0, "report.before[0].sha256")?,
        reference_sha256: if clingo_free {
            None
        } else {
            Some(seal(1, "report.before[1].sha256")?)
        },
        manifest_sha256: if clingo_free {
            seal(1, "report.before[1].sha256")?
        } else {
            seal(2, "report.before[2].sha256")?
        },
        started_unix_ns: labelled.report["report"]["started_unix_ns"]
            .as_u64()
            .ok_or(malformed("report.started_unix_ns"))?,
        finished_unix_ns: labelled.report["report"]["finished_unix_ns"].as_u64(),
        passed: labelled.report["passed"]
            .as_bool()
            .ok_or(malformed("passed"))?,
        accounted: labelled.report["accounted"]
            .as_bool()
            .ok_or(malformed("accounted"))?,
    })
}

fn samples<'a>(labelled: &Labelled<'a>) -> Result<&'a [Value], ViewError> {
    labelled.report["report"]["samples"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(ViewError::Malformed {
            label: labelled.label.into(),
            field: "report.samples",
        })
}

fn timed<'a>(
    labelled: &Labelled<'a>,
    case: usize,
    producer: impl Fn(&Value) -> bool,
) -> Result<Vec<&'a Value>, ViewError> {
    Ok(samples(labelled)?
        .iter()
        .filter(|sample| {
            sample["slot"]["case"].as_u64() == Some(case as u64)
                && sample["slot"]["phase"] == "timed"
                && producer(&sample["slot"]["producer"])
        })
        .collect())
}

fn timing(labelled: &Labelled<'_>, samples: &[&Value]) -> Result<Timing, ViewError> {
    let mut intervals = samples
        .iter()
        .map(|sample| {
            sample["capture"]["elapsed_ns"]
                .as_u64()
                .ok_or(ViewError::Malformed {
                    label: labelled.label.into(),
                    field: "capture.elapsed_ns",
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let median_ns = median(&mut intervals).ok_or(ViewError::Malformed {
        label: labelled.label.into(),
        field: "report.samples",
    })?;
    Ok(Timing {
        samples: intervals.len(),
        minimum_ns: intervals[0],
        median_ns,
        maximum_ns: intervals[intervals.len() - 1],
    })
}

fn native(labelled: &Labelled<'_>, case: usize, profile: usize) -> Result<Native, ViewError> {
    let records = timed(labelled, case, |producer| {
        producer["solver"] == "native" && producer["profile"].as_u64() == Some(profile as u64)
    })?;
    let all = samples(labelled)?;
    let mut decisions = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    for record in &records {
        let decision = record["decision"].as_str().unwrap_or("unknown");
        if decision == "pass" {
            continue;
        }
        // A position the campaign never launched is described by the
        // decision that disabled its cell, when the record names it.
        let name = match record["blocked_by"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| all.get(index))
            .and_then(|blocker| blocker["decision"].as_str())
        {
            Some(blocker) if decision == "not_attempted" => format!("blocked by {blocker}"),
            _ => decision.to_owned(),
        };
        *decisions.entry(name).or_insert(0) += 1;
        *reasons.entry(sample_reason(record, all)).or_insert(0) += 1;
    }
    if records.is_empty() {
        return Err(ViewError::Malformed {
            label: labelled.label.into(),
            field: "timed native samples for the cell",
        });
    }
    if !decisions.is_empty() {
        return Ok(Native::NotPassed { decisions, reasons });
    }
    let timing = timing(labelled, &records)?;
    let first = records[0];
    let stdout: Option<Value> = first["capture"]["stdout"]["data"]
        .as_str()
        .and_then(|text| serde_json::from_str(text).ok());
    let stderr = first["capture"]["stderr"]["data"].as_str().unwrap_or("");
    let mut drivers: Vec<u64> = records
        .iter()
        .filter_map(|record| record["observation"]["timing"]["driver_elapsed_ns"].as_u64())
        .collect();
    let driver_median_ns = median(&mut drivers);
    let mut phases: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for record in &records {
        if let Some(measured) = record["observation"]["timing"]["phases"].as_object() {
            for (name, value) in measured {
                if let Some(elapsed) = value["elapsed_ns"].as_u64() {
                    phases.entry(name.clone()).or_default().push(elapsed);
                }
            }
        }
    }
    let phases = phases
        .into_iter()
        .filter_map(|(name, mut values)| {
            median(&mut values).map(|median_ns| (name, PhaseTiming { median_ns }))
        })
        .collect();
    let peak_rss_bytes = memory(labelled, case, |producer| {
        producer["solver"] == "native" && producer["profile"].as_u64() == Some(profile as u64)
    })?;
    Ok(Native::Passed(Passed {
        timing,
        driver_median_ns,
        breakdown: breakdown(&records),
        peak_rss_bytes,
        device_bytes: stdout.as_ref().and_then(|document| {
            document["statistics"]["execution"]["peak_accounted_bytes"].as_u64()
        }),
        published_models: stdout
            .as_ref()
            .and_then(|document| document["outcome"]["published_models"].as_u64()),
        // The formula route types the count as `statistics.search.candidates`;
        // the closure route reports it only on the human results line.
        candidates_examined: stdout
            .as_ref()
            .and_then(|document| document["statistics"]["search"]["candidates"].as_u64())
            .or_else(|| candidates_examined(stderr)),
        search_work: stdout.as_ref().and_then(|document| {
            document["statistics"]["search"]["work"]
                .as_u64()
                .or_else(|| document["statistics"]["closure_execution"]["work"].as_u64())
        }),
        phases,
    }))
}

fn failure_reasons(
    labelled: &Labelled<'_>,
    case: usize,
) -> Result<BTreeMap<String, usize>, ViewError> {
    let all = samples(labelled)?;
    let mut reasons = BTreeMap::new();
    for record in all.iter().filter(|record| {
        record["slot"]["case"].as_u64() == Some(case as u64) && record["decision"] != "pass"
    }) {
        let producer = &record["slot"]["producer"];
        let producer = match producer["solver"].as_str() {
            Some("native") => producer["profile"].as_u64().map_or_else(
                || "native profile unavailable".to_owned(),
                |profile| format!("native profile index {profile}"),
            ),
            Some(solver) => solver.to_owned(),
            None => "producer unavailable".to_owned(),
        };
        let reason = format!("{producer}: {}", sample_reason(record, all));
        *reasons.entry(reason).or_insert(0) += 1;
    }
    Ok(reasons)
}

fn sample_reason(record: &Value, samples: &[Value]) -> String {
    let phase = record["slot"]["phase"]
        .as_str()
        .unwrap_or("phase unavailable");
    let decision = record["decision"].as_str().unwrap_or("unknown");
    let detail = record["detail"]
        .as_str()
        .unwrap_or("reason unavailable in retained sample");
    let mut reason = format!("{phase}: {decision}: {detail}");
    if let Some(index) = record["blocked_by"].as_u64() {
        use std::fmt::Write;
        if let Some(blocker) = usize::try_from(index)
            .ok()
            .and_then(|index| samples.get(index))
        {
            write!(
                reason,
                "; blocked by sample {index} ({}, {}): {}",
                blocker["slot"]["phase"]
                    .as_str()
                    .unwrap_or("phase unavailable"),
                blocker["decision"].as_str().unwrap_or("unknown"),
                blocker["detail"]
                    .as_str()
                    .unwrap_or("reason unavailable in retained sample"),
            )
            .expect("writing to a String cannot fail");
        } else {
            write!(reason, "; blocking sample {index} unavailable")
                .expect("writing to a String cannot fail");
        }
    }
    reason
}

// The human statistics line `results: displayed models=…; candidates examined=…`.
fn candidates_examined(stderr: &str) -> Option<u64> {
    stderr
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("results: "))
        .flat_map(|line| line.split("; "))
        .find_map(|field| field.strip_prefix("candidates examined="))
        .and_then(|value| value.trim().parse().ok())
}

fn reference(labelled: &Labelled<'_>, case: usize) -> Result<Option<Reference>, ViewError> {
    let records = timed(labelled, case, |producer| producer["solver"] == "reference")?;
    if records.is_empty() || records.iter().any(|record| record["decision"] != "pass") {
        return Ok(None);
    }
    let timing = timing(labelled, &records)?;
    // The reference prints its own times in seconds to the millisecond; its
    // total less its solving time is grounding and preprocessing.
    let mut grounding = Vec::new();
    let mut solving = Vec::new();
    for record in &records {
        let Some(times) = record["capture"]["stdout"]["data"]
            .as_str()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .map(|document| document["Time"].clone())
        else {
            continue;
        };
        if let (Some(total), Some(solve)) = (
            times["Total"].as_f64().and_then(seconds_to_ns),
            times["Solve"].as_f64().and_then(seconds_to_ns),
        ) {
            grounding.push(total.saturating_sub(solve));
            solving.push(solve);
        }
    }
    Ok(Some(Reference {
        timing,
        grounding_ns: median(&mut grounding),
        solving_ns: median(&mut solving),
        peak_rss_bytes: memory(labelled, case, |producer| producer["solver"] == "reference")?,
    }))
}

/// The sums of each part's phases per record, and their medians.
fn breakdown(records: &[&Value]) -> Breakdown {
    let mut grounding = Vec::new();
    let mut proposal = Vec::new();
    let mut membership = Vec::new();
    for record in records {
        let timing = &record["observation"]["timing"];
        if let Some(elapsed) = timing["stages"]["grounding"]["elapsed_ns"].as_u64() {
            grounding.push(elapsed);
        }
        let sum = |names: &[&str]| {
            let measured: Vec<u64> = names
                .iter()
                .filter_map(|name| timing["phases"][*name]["elapsed_ns"].as_u64())
                .collect();
            (!measured.is_empty()).then(|| measured.iter().sum::<u64>())
        };
        proposal.extend(sum(&PROPOSAL_PHASES));
        membership.extend(sum(&MEMBERSHIP_PHASES));
    }
    Breakdown {
        grounding: median(&mut grounding),
        proposal: median(&mut proposal),
        membership: median(&mut membership),
    }
}

/// The median peak resident set over the passed memory rounds of one
/// producer on one cell; absent when the campaign ran none.
fn memory(
    labelled: &Labelled<'_>,
    case: usize,
    producer: impl Fn(&Value) -> bool,
) -> Result<Option<u64>, ViewError> {
    let mut peaks: Vec<u64> = samples(labelled)?
        .iter()
        .filter(|sample| {
            sample["slot"]["case"].as_u64() == Some(case as u64)
                && sample["slot"]["phase"] == "memory"
                && sample["decision"] == "pass"
                && producer(&sample["slot"]["producer"])
        })
        .filter_map(|sample| sample["memory"]["peak_rss_bytes"].as_u64())
        .collect();
    Ok(median(&mut peaks))
}

/// The one median of the file: the middle value, or the midpoint of the two
/// middle values for an even count; `None` of nothing.
pub(crate) fn median(values: &mut [u64]) -> Option<u64> {
    values.sort_unstable();
    let count = values.len();
    match count {
        0 => None,
        _ if count.is_multiple_of(2) => {
            Some(u64::midpoint(values[count / 2 - 1], values[count / 2]))
        }
        _ => Some(values[count / 2]),
    }
}

/// Seconds as the reference prints them to whole nanoseconds; a negative
/// or non-finite value is not a duration.
fn seconds_to_ns(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is finite and non-negative, and rounded to whole nanoseconds"
    )]
    Some((seconds * 1e9).round() as u64)
}

#[cfg(test)]
mod tests;
