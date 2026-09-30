//! Completed external display evidence with explicit process and report checks.

use std::collections::BTreeMap;
use std::time::Duration;

use zetesis_clingo_support as oracle;

/// Every answer set of `source` as clingo reports it, accepting the exit codes
/// in `exits`.
pub(crate) fn run(source: &str, exits: &[i32]) -> oracle::Run {
    oracle::run_accepting(
        source,
        &["0", "--outf=2"],
        exits,
        oracle::Limits {
            timeout: Duration::from_secs(30),
            max_output_bytes: 8 * 1024 * 1024,
        },
    )
}

pub(crate) fn compare(source: &str, witnesses: &serde_json::Value) {
    let actual = oracle::answers(&run(source, &oracle::DECIDED));
    assert!(
        actual.cost().is_none(),
        "observation reference unexpectedly optimized"
    );
    let witnesses = witnesses.as_array().unwrap();
    let mut expected = BTreeMap::<Vec<String>, u64>::new();
    for witness in witnesses {
        let mut symbols: Vec<String> = witness
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect();
        symbols.sort();
        *expected.entry(symbols).or_default() += 1;
    }
    assert_eq!(actual.satisfiable(), !witnesses.is_empty(), "{source}");
    assert_eq!(
        actual.model_count(),
        u64::try_from(witnesses.len()).unwrap(),
        "{source}"
    );
    assert_eq!(
        actual.displays(),
        expected.into_iter().collect::<Vec<_>>(),
        "{source}"
    );
}
