//! Ordinary lazy-device routing and physical qualification boundaries.

#[cfg(feature = "gpu")]
use crate::support::physical_backend;
use clap::Parser;
use zetesis_cli::{Completion, Options, run_with_diagnostics};
use zetesis_cpu::Cancellation;

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
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        "a.".into(),
        &options(&["--backend", "metal", "--stats"]),
        &mut output,
        &mut diagnostics,
        &cancellation,
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
        &Cancellation::default(),
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
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(report.completion, Completion::Exhausted);
    }
}

#[cfg(feature = "gpu")]
mod physical {
    use super::physical_backend::Physical;
    use super::{Cancellation, Completion, options, run_with_diagnostics};
    use zetesis_backend::GpuApi;
    use zetesis_cli::{Interruption, RunError};
    use zetesis_cpu::Stop;

    const WORLDS: &str = "a:-not b. b:-not a. x:-a. y:-b. cross:-x,y. :-cross.";

    #[test]
    #[ignore = "requires Metal: automatic grounder keeps source joins"]
    fn metal_automatic_grounder_keeps_source_joins() {
        qualify_automatic_grounder(GpuApi::Metal);
    }

    #[test]
    #[ignore = "requires Vulkan: automatic grounder keeps source joins"]
    fn vulkan_automatic_grounder_keeps_source_joins() {
        qualify_automatic_grounder(GpuApi::Vulkan);
    }

    fn qualify_automatic_grounder(backend: GpuApi) {
        let source = "{a}. {b}. {c}. {d}. {e}. {f}.";
        let (_, expected, _) = solve(source, &["--backend", "cpu", "--json"]);
        for grounder in [zetesis_cli::Grounder::Auto, zetesis_cli::Grounder::Lazy] {
            let mut selected = options(&["--stats", "--json"]);
            selected.backend = backend.requested();
            selected.grounder = grounder;
            // A hidden static lowering cannot succeed under these limits.
            selected.max_ground_rules = 0;
            selected.max_substitutions = 0;
            let mut output = Vec::new();
            let mut diagnostics = Vec::new();
            let report = run_with_diagnostics(
                source.into(),
                &selected,
                &mut output,
                &mut diagnostics,
                &Cancellation::default(),
            )
            .unwrap();
            let actual: serde_json::Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            assert_eq!(report.models, 64);
            assert_eq!(actual["models"], expected["models"]);
            let stats = report.lazy_execution.unwrap();
            assert_eq!(stats.requested_backend, backend.requested());
            assert_eq!(stats.backend, backend.name());
            assert!(stats.dispatches > 0);
            assert!(stats.completed_candidates > 0);
            assert_eq!(stats.submitted_candidates, stats.completed_candidates);
            assert!(stats.completed_candidates <= report.checked);
            assert_eq!(stats.stopped_candidates, 0);
            let diagnostics = String::from_utf8(diagnostics).unwrap();
            assert!(diagnostics.contains("effective=lazy"));
            assert!(!diagnostics.contains("effective=eager"));
            assert!(!diagnostics.contains("effective execution: backend=cpu;"));
            assert!(!diagnostics.contains("grounder=untracked"));
        }
    }

    fn solve(source: &str, arguments: &[&str]) -> (zetesis_cli::Report, serde_json::Value, String) {
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let report = run_with_diagnostics(
            source.into(),
            &options(arguments),
            &mut output,
            &mut diagnostics,
            &Cancellation::default(),
        )
        .unwrap();
        (
            report,
            serde_json::from_slice(&output).unwrap(),
            String::from_utf8(diagnostics).unwrap(),
        )
    }

    #[test]
    #[ignore = "requires Metal: ordinary lazy Metal preserves complete CPU models"]
    fn ordinary_lazy_metal_preserves_complete_cpu_models() {
        qualify_lazy_models(GpuApi::Metal);
    }

    #[test]
    #[ignore = "requires Vulkan: ordinary lazy Vulkan matches CPU models"]
    fn ordinary_lazy_vulkan_matches_cpu_models() {
        qualify_lazy_models(GpuApi::Vulkan);
    }

    fn qualify_lazy_models(backend: GpuApi) {
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
                        backend.argument(),
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
                // A program the root's narrowing refutes, such as a self-supported
                // atom under a constraint requiring it, submits no seed and
                // dispatches nothing; every submitted seed is dispatched.
                if stats.submitted_candidates == 0 {
                    assert_eq!(stats.dispatches, 0);
                } else {
                    assert!(stats.dispatches > 0);
                    assert!(stats.world_instances > 0);
                    assert!(stats.transport_allocations > 0);
                    assert!(stats.peak_transport_bytes > 0);
                    qualify_transport_usage(&stats);
                }
                assert_eq!(
                    stats
                        .transport_allocations
                        .checked_add(stats.transport_reuses),
                    Some(stats.dispatches)
                );
                assert_eq!(stats.requested_backend, backend.requested());
                assert_eq!(stats.backend, backend.name());
                assert!(diagnostics.contains("effective=lazy"));
                assert!(!diagnostics.contains("effective=eager"));
                if stats.submitted_candidates == 0 {
                    assert!(diagnostics.contains(
                        "effective execution: none needed; the root narrowing refuted every seed"
                    ));
                } else {
                    assert!(diagnostics.contains("effective execution: oracle=closure; backend=requested GPU policy; grounder=lazy; see backend diagnostics for actual adapter"));
                }
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

    fn qualify_transport_usage(stats: &zetesis_cli::LazyExecutionStatistics) {
        let usage = stats.transport_usage;
        for binding in [
            usage.uniform,
            usage.offsets,
            usage.records,
            usage.snapshots,
            usage.seeds,
            usage.output,
            usage.readback,
        ] {
            assert_eq!(
                binding.allocations.checked_add(binding.reuses),
                Some(stats.dispatches)
            );
            assert!(binding.allocations > 0);
            assert!(binding.allocations <= stats.transport_allocations);
            assert!(binding.reuses >= stats.transport_reuses);
        }
        assert_eq!(usage.output, usage.readback);
        assert!(usage.budget_releases <= stats.transport_allocations);
        assert!(usage.accounting_overflow_releases <= stats.transport_allocations);
    }

    #[test]
    #[ignore = "requires Metal: requested model limit retains completed lazy candidates"]
    fn requested_model_limit_retains_completed_lazy_candidates() {
        qualify_model_limit(GpuApi::Metal);
    }

    #[test]
    #[ignore = "requires Vulkan: model limit retains completed candidates"]
    fn vulkan_model_limit_retains_completed_candidates() {
        qualify_model_limit(GpuApi::Vulkan);
    }

    fn qualify_model_limit(backend: GpuApi) {
        let (report, value, _) = solve(
            WORLDS,
            &[
                "--backend",
                backend.argument(),
                "--batch-size",
                "3",
                "--models",
                "1",
                "--stats",
                "--json",
            ],
        );
        assert_eq!(report.completion, Completion::RequestedModels);
        // The region tree offers the two decided seeds one at a time, so
        // the one requested model needs one submission.
        let stats = report.lazy_execution.unwrap();
        assert_eq!(stats.submitted_candidates, 1);
        assert_eq!(stats.completed_candidates, 1);
        assert_eq!(stats.queued_results, 0);
        assert_eq!(report.checked, 1);
        assert_eq!(value["outcome"]["verified_models"], 1);
        assert_eq!(value["outcome"]["published_models"], 1);
    }

    #[test]
    #[ignore = "requires Metal: lazy source stop preserves unfinished candidate counts"]
    fn lazy_source_stop_preserves_unfinished_candidate_counts() {
        qualify_source_stop(GpuApi::Metal);
    }

    #[test]
    #[ignore = "requires Vulkan: source stop retains unfinished candidates"]
    fn vulkan_source_stop_retains_unfinished_candidates() {
        qualify_source_stop(GpuApi::Vulkan);
    }

    fn qualify_source_stop(backend: GpuApi) {
        let (report, value, _) = solve(
            WORLDS,
            &[
                "--backend",
                backend.argument(),
                "--max-work",
                "0",
                "--stats",
                "--json",
            ],
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

    #[test]
    #[ignore = "requires Metal: lazy writer failure preserves completed device work"]
    fn lazy_writer_failure_preserves_completed_device_work() {
        qualify_writer_failure(GpuApi::Metal);
    }

    #[test]
    #[ignore = "requires Vulkan: writer failure retains completed work"]
    fn vulkan_writer_failure_retains_completed_work() {
        qualify_writer_failure(GpuApi::Vulkan);
    }

    fn qualify_writer_failure(backend: GpuApi) {
        let error = zetesis_cli::run_detailed_with_diagnostics(
            WORLDS.into(),
            &options(&[
                "--backend",
                backend.argument(),
                "--batch-size",
                "3",
                "--stats",
            ]),
            &mut zetesis_test_support::io::Closed,
            &mut Vec::new(),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(matches!(*error.cause, RunError::Output(_)));
        let partial = error.partial_report.unwrap();
        assert_eq!(partial.published_models, 0);
        // The region tree offers the decided seeds one at a time and the
        // session asks for one model, so one seed was checked and verified
        // before the writer failed; its result was taken for publication.
        assert_eq!(partial.verified_models, 1);
        let stats = partial.lazy_execution.unwrap();
        assert_eq!(stats.completed_candidates, 1);
        assert_eq!(stats.queued_results, 0);
        assert!(stats.dispatches > 0);
    }
}
