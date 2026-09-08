//! Ordinary lazy-device routing and physical qualification boundaries.

use clap::Parser;
use zetesis_cli::{Completion, Options, run_with_diagnostics};
use zetesis_cpu::Control;

fn options(arguments: &[&str]) -> Options {
    let mut selected =
        Options::try_parse_from(["zetesis"].into_iter().chain(arguments.iter().copied())).unwrap();
    if !arguments.contains(&"--models") {
        selected.models = 0;
    }
    selected.oracle = zetesis_cli::Oracle::Closure;
    selected.grounder = zetesis_cli::Grounder::Lazy;
    selected
}

#[test]
fn lazy_device_cancellation_precedes_adapter_initialization() {
    let control = Control::default();
    control.cancel();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "a.".into(),
        &options(&["--backend", "metal", "--stats"]),
        &mut output,
        &mut diagnostics,
        &control,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    assert!(report.lazy_execution.is_none());
    assert!(!String::from_utf8(diagnostics).unwrap().contains("Backend:"));
}

#[test]
fn cpu_lazy_reports_no_device_execution() {
    let mut output = Vec::new();
    let report = run_with_diagnostics(
        "a.".into(),
        &options(&["--backend", "cpu", "--stats", "--json"]),
        &mut output,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert!(report.lazy_execution.is_none());
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["statistics"]["lazy_execution"].is_null());
}

#[test]
fn physical_fixtures_belong_to_the_lazy_source_profile() {
    for source in [
        "a:-not b. b:-not a. x:-a. y:-b. cross:-x,y. :-cross.",
        "p(1). p(2). q(X):-p(X).",
        "a:-a. :-not a.",
        "a:-not not a.",
        "p. -q:-p.",
    ] {
        let report = run_with_diagnostics(
            source.into(),
            &options(&["--backend", "cpu", "--json"]),
            &mut Vec::new(),
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
    }
}

#[cfg(feature = "gpu")]
mod physical {
    use super::{Completion, Control, options, run_with_diagnostics};
    use zetesis_cli::{Interruption, RunError};
    use zetesis_cpu::Stop;

    const WORLDS: &str = "a:-not b. b:-not a. x:-a. y:-b. cross:-x,y. :-cross.";

    fn solve(source: &str, arguments: &[&str]) -> (zetesis_cli::Report, serde_json::Value, String) {
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let report = run_with_diagnostics(
            source.into(),
            &options(arguments),
            &mut output,
            &mut diagnostics,
            &Control::default(),
        )
        .unwrap();
        (
            report,
            serde_json::from_slice(&output).unwrap(),
            String::from_utf8(diagnostics).unwrap(),
        )
    }

    #[test]
    #[ignore = "requires physical Metal through the ordinary lazy solver"]
    fn ordinary_lazy_metal_preserves_complete_cpu_models() {
        for source in [
            WORLDS,
            "p(1). p(2). q(X):-p(X).",
            "a:-a. :-not a.",
            "a:-not not a.",
            "p. -q:-p.",
        ] {
            let (_, expected, _) = solve(source, &["--backend", "cpu", "--stats", "--json"]);
            for batch in ["1", "3", "33"] {
                let (report, actual, diagnostics) = solve(
                    source,
                    &[
                        "--backend",
                        "metal",
                        "--batch-size",
                        batch,
                        "--stats",
                        "--json",
                    ],
                );
                assert_eq!(report.completion, Completion::Exhausted);
                assert_eq!(actual["models"], expected["models"]);
                let stats = report.lazy_execution.unwrap();
                assert_eq!(stats.submitted_candidates, stats.completed_candidates);
                assert_eq!(stats.stopped_candidates, 0);
                assert_eq!(stats.queued_results, 0);
                assert!(stats.dispatches > 0);
                assert!(stats.world_instances > 0);
                assert!(stats.transport_allocations > 0);
                assert!(stats.peak_transport_bytes > 0);
                assert_eq!(
                    stats
                        .transport_allocations
                        .checked_add(stats.transport_reuses),
                    Some(stats.dispatches)
                );
                assert_eq!(stats.requested_backend, zetesis_cli::Backend::Metal);
                assert_eq!(stats.backend, "Metal");
                assert!(diagnostics.contains("effective=lazy"));
                assert!(!diagnostics.contains("effective=eager"));
                assert!(diagnostics.contains("effective execution: oracle=closure; backend=requested GPU policy; grounder=lazy; see backend diagnostics for actual adapter"));
                assert_eq!(
                    actual["statistics"]["lazy_execution"]["dispatches"],
                    stats.dispatches
                );
                assert_eq!(
                    actual["statistics"]["lazy_execution"]["transport_reuses"],
                    stats.transport_reuses
                );
                assert_eq!(
                    actual["statistics"]["lazy_execution"]["peak_transport_bytes"],
                    stats.peak_transport_bytes
                );
                eprintln!("{}", actual["statistics"]["lazy_execution"]);
            }
        }
    }

    #[test]
    #[ignore = "requires physical Metal through the ordinary lazy solver"]
    fn requested_model_limit_retains_completed_lazy_candidates() {
        let (report, value, _) = solve(
            WORLDS,
            &[
                "--backend",
                "metal",
                "--batch-size",
                "3",
                "--models",
                "1",
                "--stats",
                "--json",
            ],
        );
        assert_eq!(report.completion, Completion::RequestedModels);
        let stats = report.lazy_execution.unwrap();
        assert_eq!(stats.submitted_candidates, 4);
        assert_eq!(stats.completed_candidates, 4);
        assert_eq!(stats.queued_results, 2);
        assert_eq!(report.checked, 2);
        assert_eq!(value["outcome"]["verified_models"], 2);
        assert_eq!(value["outcome"]["published_models"], 1);
    }

    #[test]
    #[ignore = "requires physical Metal through the ordinary lazy solver"]
    fn lazy_source_stop_preserves_unfinished_candidate_counts() {
        let (report, value, _) = solve(
            WORLDS,
            &["--backend", "metal", "--max-work", "0", "--stats", "--json"],
        );
        assert_eq!(report.completion, Completion::Interrupted);
        assert_eq!(
            report.interruption,
            Some(Interruption::Oracle(Stop::WorkLimit))
        );
        let stats = report.lazy_execution.unwrap();
        assert_eq!(stats.submitted_candidates, 1);
        assert_eq!(stats.stopped_candidates, 1);
        assert_eq!(stats.completed_candidates, 0);
        assert_eq!(stats.dispatches, 0);
        assert_eq!(value["outcome"]["status"], "incomplete");
    }

    struct BrokenWriter;
    impl std::io::Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected model sink failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    #[ignore = "requires physical Metal through the ordinary lazy solver"]
    fn lazy_writer_failure_preserves_completed_device_work() {
        let error = zetesis_cli::run_detailed_with_diagnostics(
            WORLDS.into(),
            &options(&["--backend", "metal", "--batch-size", "3", "--stats"]),
            &mut BrokenWriter,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(matches!(*error.cause, RunError::Output(_)));
        let partial = error.partial_report.unwrap();
        assert_eq!(partial.published_models, 0);
        assert_eq!(partial.verified_models, 2);
        let stats = partial.lazy_execution.unwrap();
        assert_eq!(stats.completed_candidates, 4);
        assert_eq!(stats.queued_results, 2);
        assert!(stats.dispatches > 0);
    }
}
