//! Completed external display evidence with explicit process and report checks.

use std::collections::BTreeMap;
use std::ffi::OsString;

use zetesis_validation::answers;
use zetesis_validation::process::{Exit, Invocation, Limits, Stop, invoke};

pub(super) fn compare(source: &str, witnesses: &serde_json::Value) {
    let executable =
        std::path::PathBuf::from(std::env::var_os("CLINGO").expect("set absolute CLINGO"));
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.lp");
    std::fs::write(&input, source).unwrap();
    let arguments = [
        input.into_os_string(),
        OsString::from("0"),
        OsString::from("--outf=2"),
    ];
    let (capture, pending) = invoke(
        Invocation {
            executable: &executable,
            arguments: &arguments,
            directory: directory.path(),
        },
        Limits::default(),
    )
    .unwrap()
    .into_parts();
    if let Some(pending) = pending {
        let cleanup = pending.retry(std::time::Duration::from_secs(1));
        let abandoned = cleanup
            .pending
            .map(zetesis_validation::process::PendingChild::abandon);
        assert!(
            abandoned.is_none(),
            "unreaped child ownership abandoned: {abandoned:?}; cleanup failure: {:?}",
            cleanup.failure
        );
        assert!(
            cleanup.failure.is_none(),
            "cleanup retry failed: {:?}",
            cleanup.failure
        );
    }
    assert_eq!(capture.stop(), Stop::Completed, "{capture:?}");
    assert!(capture.failure().is_none(), "{capture:?}");
    assert!(capture.cleanup_failure().is_none(), "{capture:?}");
    assert!(
        matches!(
            capture.exit(),
            Some(Exit {
                code: Some(10 | 20 | 30),
                signal: None
            })
        ),
        "{capture:?}"
    );
    let actual = answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap();
    assert_eq!(actual.solver(), "clingo version 5.8.2");
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
