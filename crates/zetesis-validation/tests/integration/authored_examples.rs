//! Fixture integrity is portable; complete clingo contract checks are opt-in.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use zetesis_validation::{
    examples,
    performance::{
        matrix::{Workload, WorkloadLimits},
        scalability,
    },
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn defaults() -> Vec<Workload> {
    let mut workloads = scalability::defaults(&root(), WorkloadLimits::default()).unwrap();
    workloads.push(scalability::einstein(&root(), WorkloadLimits::default()).unwrap());
    workloads
}

#[test]
fn reviewed_default_sources_retain_their_shared_typed_contracts() {
    let workloads = defaults();
    assert_eq!(
        workloads.iter().map(Workload::entry).collect::<Vec<_>>(),
        [
            "scalability/n-queens.lp",
            "scalability/pigeonhole.lp",
            "einstein-riddle.lp"
        ]
    );
    for (workload, count) in workloads.iter().zip([92, 0, 1]) {
        assert!(workload.is_authored());
        assert!(!workload.is_amended());
        assert!(!workload.is_generated());
        let contract = workload.contract().unwrap();
        assert_eq!(contract.model_count(), Some(count));
        assert_eq!(contract.family(), examples::Family::All);
        assert_eq!(
            contract.satisfiability(),
            if count == 0 {
                examples::Satisfiability::Unsat
            } else {
                examples::Satisfiability::Sat
            }
        );
        let value = serde_json::to_value(workload).unwrap();
        assert!(value.get("manifest_sha256").is_none());
        assert!(value.get("authored").is_some());
        assert_eq!(
            value["sources"][0]["base_sha256"],
            value["sources"][0]["derived_sha256"]
        );
    }
    assert_eq!(workloads[2].contract().unwrap().witnesses()[0].len(), 5);
}

#[test]
fn scaling_population_has_distinct_sizes_and_reference_qualified_amendments() {
    let corpus = examples::load(&root().join("correctness"), examples::Limits::default()).unwrap();
    let workloads =
        scalability::workloads(&corpus, &root(), true, WorkloadLimits::default()).unwrap();
    assert_eq!(workloads.len(), 10);
    assert_eq!(
        workloads
            .iter()
            .map(Workload::identity)
            .collect::<BTreeSet<_>>()
            .len(),
        10
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_authored())
            .count(),
        7
    );
    assert_eq!(
        workloads
            .iter()
            .filter(|workload| workload.is_amended())
            .count(),
        4
    );
    for workload in &workloads {
        assert_eq!(workload.contract().is_none(), workload.is_amended());
    }
    let edits: Vec<_> = workloads[..6]
        .iter()
        .map(|workload| {
            let value = serde_json::to_value(workload).unwrap();
            value["sources"][0]["edits"].clone()
        })
        .collect();
    assert_eq!(edits[0], serde_json::json!([]));
    assert_eq!(edits[5], serde_json::json!([]));
    assert_eq!(edits[1][0]["before"], "8");
    assert_eq!(edits[1][0]["after"], "9");
    assert_eq!(edits[2][0]["after"], "10");
    assert_eq!(edits[3][0]["before"], "7");
    assert_eq!(edits[3][0]["after"], "5");
    assert_eq!(edits[4][0]["after"], "6");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "requires CLINGO as an absolute clingo 5.8.x executable path"]
fn default_authored_families_satisfy_their_contracts() {
    use std::{ffi::OsString, time::Duration};
    use zetesis_validation::{answers, process};
    let clingo = PathBuf::from(std::env::var_os("CLINGO").expect("set CLINGO to an absolute path"));
    assert!(clingo.is_absolute());
    for workload in defaults() {
        let arguments: Vec<OsString> = ["--models=0", "--outf=2", workload.entry()]
            .into_iter()
            .map(Into::into)
            .collect();
        let outcome = process::invoke(
            process::Invocation {
                executable: &clingo,
                arguments: &arguments,
                directory: &root(),
            },
            process::Limits {
                timeout: Duration::from_mins(1),
                max_output_bytes: 8 * 1024 * 1024,
                cleanup_timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
        let (capture, pending) = outcome.into_parts();
        if let Some(child) = pending {
            let cleanup = child.retry(Duration::from_secs(2));
            if let Some(child) = cleanup.pending {
                panic!("unreaped child {}", child.abandon());
            }
        }
        assert_eq!(
            capture.stop(),
            process::Stop::Completed,
            "{}: {capture:?}",
            workload.entry()
        );
        assert!(capture.failure().is_none(), "{capture:?}");
        assert!(capture.cleanup_failure().is_none(), "{capture:?}");
        let exit = capture.exit().unwrap();
        assert_eq!(exit.signal, None);
        assert!(matches!(exit.code, Some(10 | 20 | 30)), "{capture:?}");
        let answers = answers::clingo_json(capture.stdout(), answers::Limits::default()).unwrap();
        workload.contract().unwrap().check(&answers).unwrap();
        println!(
            "pass: {} complete_models={}",
            workload.entry(),
            answers.model_count()
        );
    }
}
