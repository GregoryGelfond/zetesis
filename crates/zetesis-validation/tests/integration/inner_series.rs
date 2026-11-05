//! The quantified series: the inner preparation's census and door checks on
//! the saturation cores, each size's family checked against clingo.
//!
//! Opt-in, run by hand: `ZETESIS` names the solver executable (an absolute
//! path) and `CLINGO` the oracle, as the oracle campaigns name it. Each size
//! is the committed program with its constants amended by the edits the
//! cores' cases make, applied to the one `#const` line each names.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use zetesis_clingo_support as oracle;
use zetesis_test_support::repository::examples as root;
use zetesis_validation::answers::{self, native_json};
use zetesis_validation::process::{self, Invocation, Limits, Stop};

use super::authored_examples::two_qbf_cores::{
    Fixture, Instance, PARITY, PARITY_DEFAULT, TSEITIN, TSEITIN_DEFAULT, series,
};

/// One size of one core: the fixture, its default and the instance.
#[derive(Clone, Copy, Debug)]
struct Size {
    fixture: &'static Fixture,
    default: Instance,
    instance: Instance,
}

impl Size {
    /// The core's size: parity inputs or Tseitin vertices.
    fn value(self) -> u64 {
        u64::try_from(self.instance.size()).unwrap()
    }
}

/// The parity series, then the Tseitin series.
fn sizes() -> [Vec<Size>; 2] {
    [(&PARITY, PARITY_DEFAULT), (&TSEITIN, TSEITIN_DEFAULT)].map(|(fixture, default)| {
        series(default)
            .into_iter()
            .map(|instance| Size {
                fixture,
                default,
                instance,
            })
            .collect()
    })
}

/// The committed program with the size's constants amended: each edit finds
/// its `#const` line once at the expected value, the amendment's own
/// precondition, and replaces the value.
fn program(size: Size) -> String {
    let path = root().join(size.fixture.directory).join(size.fixture.entry);
    let mut text = std::fs::read_to_string(&path).unwrap();
    for edit in size.instance.edits(size.default, size.fixture.entry) {
        let line = format!("#const {} = {}.", edit.name, edit.expected);
        assert_eq!(text.matches(&line).count(), 1, "{size:?}: {line}");
        text = text.replace(
            &line,
            &format!("#const {} = {}.", edit.name, edit.replacement),
        );
    }
    text
}

fn solver() -> PathBuf {
    let path =
        PathBuf::from(std::env::var_os("ZETESIS").expect("ZETESIS names the solver executable"));
    assert!(
        path.is_absolute(),
        "ZETESIS must name an absolute path: {}",
        path.display()
    );
    path
}

/// What one solve reports: the family, the candidates, the inner walk's
/// regions and the preparation's receipts.
struct Receipt {
    answers: answers::ReportedAnswers,
    models: u64,
    candidates: u64,
    visited: u64,
    refuted: u64,
    preparation: Value,
}

fn solve(directory: &Path, entry: &str) -> Receipt {
    let arguments: Vec<OsString> = [
        "solve",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--grounder",
        "eager",
        "--threads",
        "1",
        "--all",
        "--color",
        "never",
        "--json",
        "--stats",
        entry,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let outcome = process::invoke(
        Invocation {
            executable: &solver(),
            arguments: &arguments,
            directory,
        },
        Limits {
            timeout: Duration::from_mins(10),
            max_output_bytes: 64 << 20,
            cleanup_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    let (capture, pending) = outcome.into_parts();
    if let Some(child) = pending
        && let Some(child) = child.retry(Duration::from_secs(1)).pending
    {
        panic!(
            "the solver's child {} outlived its capture",
            child.abandon()
        );
    }
    assert_eq!(
        capture.stop(),
        Stop::Completed,
        "{}",
        String::from_utf8_lossy(capture.stderr())
    );
    let document: Value = serde_json::from_slice(capture.stdout())
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(capture.stderr())));
    assert_eq!(document["outcome"]["completion"], "exhausted");
    let search = &document["statistics"]["search"];
    let regions = &search["reduct_query_regions"];
    let answers = native_json::parse(capture.stdout(), native_json::Limits::default())
        .unwrap()
        .reported_displays(1 << 20)
        .unwrap();
    Receipt {
        answers,
        models: document["outcome"]["published_models"].as_u64().unwrap(),
        candidates: search["candidates"].as_u64().unwrap(),
        visited: regions["visited"].as_u64().unwrap(),
        refuted: regions["refuted"].as_u64().unwrap(),
        preparation: regions["preparation"].clone(),
    }
}

/// clingo on the committed program with the size's constants, as the cores'
/// cases are checked.
fn clingo(size: Size) -> answers::ReportedAnswers {
    let constants = size
        .instance
        .edits(size.default, size.fixture.entry)
        .iter()
        .map(|edit| format!("{}={}", edit.name, edit.replacement))
        .collect::<Vec<_>>();
    let arguments = ["--models=0", "--outf=2"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            constants
                .iter()
                .flat_map(|constant| ["--const".to_owned(), constant.clone()]),
        )
        .chain([size.fixture.entry.to_owned()])
        .collect::<Vec<_>>();
    oracle::answers(&oracle::run_in(
        &root().join(size.fixture.directory),
        arguments.iter().map(String::as_str),
        &oracle::DECIDED,
        oracle::Limits {
            timeout: Duration::from_mins(10),
            max_output_bytes: 8 * 1024 * 1024,
        },
    ))
}

/// Solve one size in a directory of its own, its family checked against
/// clingo's.
fn checked(size: Size) -> Receipt {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(size.fixture.entry), program(size)).unwrap();
    let receipt = solve(directory.path(), size.fixture.entry);
    let reference = clingo(size);
    assert!(
        answers::same_displays(&reference, &receipt.answers),
        "{size:?}: clingo {reference:?}, zetesis {:?}",
        receipt.answers
    );
    receipt
}

fn count(preparation: &Value, field: &str) -> u64 {
    preparation[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{field} in {preparation}"))
}

#[test]
#[ignore = "requires clingo and ZETESIS: the quantified series' families agree and the inner census holds"]
fn g_census_inner_finds_the_cores_groups_and_rows() {
    let [parity, tseitin] = sizes();
    // Parity over n inputs: two chains of n - 1 gates, four clauses each, and
    // the pair of clauses that makes the results differ: 2(n - 1) + 1 groups
    // per candidate. The stable candidate's rows refute its root in the
    // first round; each dropped-clause candidate's inexact gate is probed to
    // exactness in a second round under the other rows, so every hull ends
    // exact and the rows number at least the gates' (8n - 7 in all), the
    // dropped-clause candidates' units beyond that.
    for size in parity {
        let n = size.value();
        let receipt = checked(size);
        let p = &receipt.preparation;
        let candidates = receipt.candidates;
        assert_eq!((receipt.models, candidates), (1, 4), "n={n}");
        assert_eq!(count(p, "attempts"), candidates, "n={n}");
        assert_eq!(count(p, "prepared"), candidates, "n={n}");
        assert_eq!(count(p, "saturation_units"), candidates, "n={n}");
        assert_eq!(count(p, "groups"), candidates * (2 * (n - 1) + 1), "n={n}");
        assert_eq!(count(p, "inexact_hulls"), 0, "n={n}");
        assert!(
            count(p, "rows") >= 8 * n - 7,
            "n={n}: rows {}",
            count(p, "rows")
        );
        assert_eq!(count(p, "rounds"), candidates + 3, "n={n}");
        println!(
            "parity n={n}: probes={} rounds={} rows={} work={} regions={} refuted={}",
            count(p, "probes"),
            count(p, "rounds"),
            count(p, "rows"),
            count(p, "work"),
            receipt.visited,
            receipt.refuted
        );
    }
    // Tseitin on a cubic graph: one group of four clauses per vertex; the
    // stable candidate loads a row per vertex, and each dropped-clause
    // candidate's inexact vertex is probed to exactness in a second round:
    // at least 4V - 3 rows, every hull exact.
    for size in tseitin {
        let vertices = size.value();
        let receipt = checked(size);
        let p = &receipt.preparation;
        let candidates = receipt.candidates;
        assert_eq!((receipt.models, candidates), (1, 4), "vertices={vertices}");
        assert_eq!(
            count(p, "saturation_units"),
            candidates,
            "vertices={vertices}"
        );
        assert_eq!(
            count(p, "groups"),
            candidates * vertices,
            "vertices={vertices}"
        );
        assert_eq!(count(p, "inexact_hulls"), 0, "vertices={vertices}");
        assert!(
            count(p, "rows") >= 4 * vertices - 3,
            "vertices={vertices}: rows {}",
            count(p, "rows")
        );
        println!(
            "tseitin vertices={vertices}: probes={} rows={} work={} regions={} refuted={}",
            count(p, "probes"),
            count(p, "rows"),
            count(p, "work"),
            receipt.visited,
            receipt.refuted
        );
    }
}

#[test]
#[ignore = "requires clingo and ZETESIS: every stable candidate of the series is refuted at its query root"]
fn g_door_inner_a_stable_candidates_are_root_refuted() {
    for size in sizes().into_iter().flatten() {
        let receipt = checked(size);
        assert_eq!(
            count(&receipt.preparation, "root_refuted"),
            receipt.models,
            "{size:?}"
        );
    }
}

#[test]
#[ignore = "requires clingo and ZETESIS: the inner regions per non-minimal candidate grow polynomially along the series"]
fn g_door_inner_b_inner_regions_grow_polynomially() {
    for (name, family) in ["parity", "tseitin"].into_iter().zip(sizes()) {
        // The smallest size fixes the constant: regions per non-minimal
        // candidate there, over its size squared. Every larger size must stay
        // within that constant times its own size squared, compared exactly
        // by cross-multiplication.
        let mut smallest: Option<(u128, u128, u128)> = None;
        for size in family {
            let receipt = checked(size);
            let root_refuted = count(&receipt.preparation, "root_refuted");
            let nonminimal = u128::from(receipt.candidates - receipt.models);
            assert!(nonminimal > 0, "{size:?}");
            // Each root-refuted candidate's walk is its one region.
            let regions = u128::from(receipt.visited - root_refuted);
            let value = u128::from(size.value());
            let (regions_0, nonminimal_0, value_0) =
                *smallest.get_or_insert((regions, nonminimal, value));
            println!(
                "{name} size={value}: regions per non-minimal candidate={}",
                regions / nonminimal
            );
            assert!(
                regions * nonminimal_0 * value_0 * value_0
                    <= regions_0 * nonminimal * value * value,
                "{size:?}: {regions} regions over {nonminimal} candidates exceed the quadratic bound set at size {value_0}"
            );
        }
    }
}
