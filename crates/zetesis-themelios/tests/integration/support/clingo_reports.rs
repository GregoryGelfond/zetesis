//! clingo's bounded reports on a source, and the models they and the fixtures
//! record: every model, every model or the refusal of an unsafe source, and the
//! optimal models.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value as Json;
use zetesis_clingo_support as oracle;

use super::finite_bindings::Models;

/// clingo's report of every model of `source`, from a decided run within 3 s and
/// 128 KiB of output.
pub fn enumerated(source: &str) -> Json {
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
pub fn enumerated_or_refused_unsafe(source: &str, valid: bool) -> Json {
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
pub fn optimal(source: &str) -> Json {
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

/// The distinct atom spellings of one report witness.
pub fn json_model(values: &Json) -> BTreeSet<String> {
    let atoms = values.as_array().unwrap();
    let result: BTreeSet<_> = atoms
        .iter()
        .map(|atom| atom.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(result.len(), atoms.len(), "full atom identities are unique");
    result
}

/// The models a fixture row records from clingo.
pub fn expected(row: &Json) -> Models {
    row["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(json_model)
        .collect()
}
