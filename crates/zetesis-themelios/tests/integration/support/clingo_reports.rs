//! clingo's bounded reports on a source, and the models they and the fixtures
//! record: every model, every model or the refusal of an unsafe source, the
//! optimal models, and the optimal witnesses of an optN report.

use std::time::Duration;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;

use super::finite_bindings::Models;

/// clingo's report of every model of `source`, from a decided run within 3 s and
/// 128 KiB of output.
pub(crate) fn enumerated(source: &str) -> Json {
    oracle::json(&oracle::run(
        source,
        &["--models=0", "--outf=2"],
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    ))
}

/// clingo's report of every model of a `valid` source, or of its refusal of an
/// invalid one, which must name unsafe variables.
pub(crate) fn enumerated_or_refused_unsafe(source: &str, valid: bool) -> Json {
    let exits: &[i32] = if valid { &oracle::DECIDED } else { &[65] };
    let run = oracle::run_accepting(
        source,
        &[
            "--models=0",
            "--outf=2",
            "--parallel-mode=1",
            "--opt-mode=enum",
        ],
        exits,
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    );
    let diagnostics = String::from_utf8_lossy(run.stderr());
    // A refused source must be refused for its unsafe variables.
    assert!(
        valid || diagnostics.contains("unsafe variables"),
        "{diagnostics}"
    );
    oracle::json(&run)
}

/// clingo's report of `source`'s optimal models; an undecided run and a refusal
/// are reported, not failed.
pub(crate) fn optimal(source: &str) -> Json {
    // An undecided run (0) and a refusal (65) are reported, not failures: the
    // caller reads the report's result.
    oracle::json(&oracle::run_accepting(
        source,
        &["--models=0", "--outf=2", "--opt-mode=optN"],
        &[0, 10, 20, 30, 65],
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    ))
}

/// The optimal witnesses among an optN report's `records`, each a witness's
/// display with its costs. Such a report lists incumbents first; its first
/// witness at the optimum's costs, `summary["Costs"]`, is the final incumbent,
/// which recurs once more in the complete tie pass, so every witness up to and
/// including it is dropped. The rest have the optimum's costs, and there are
/// `summary["Optimal"]` of them.
///
/// # Panics
/// Panics unless the summary names the optimum's costs, a witness has them and
/// recurs later, and the remaining witnesses are exactly the optimal ones.
pub(crate) fn optimal_witnesses<T: PartialEq>(
    summary: &Json,
    records: Vec<(T, Option<Vec<i64>>)>,
) -> Vec<(T, Option<Vec<i64>>)> {
    let best = oracle::costs(&summary["Costs"]).expect("an optimum's costs");
    let first = records
        .iter()
        .position(|(_, costs)| costs.as_ref() == Some(&best))
        .expect("a witness at the optimum's costs");
    assert!(
        records[first + 1..].contains(&records[first]),
        "optN replays its final incumbent in the complete tie pass"
    );
    let optimal: Vec<_> = records.into_iter().skip(first + 1).collect();
    assert!(
        optimal
            .iter()
            .all(|(_, costs)| costs.as_ref() == Some(&best))
    );
    assert_eq!(summary["Optimal"].as_u64(), Some(optimal.len() as u64));
    optimal
}

/// The models a fixture row records from clingo.
pub(crate) fn expected(row: &Json) -> Models {
    row["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(oracle::atoms)
        .collect()
}
