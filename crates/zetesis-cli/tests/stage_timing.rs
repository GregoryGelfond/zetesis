//! Actual routes expose exclusive typed stages without changing model output.
use clap::Parser;
use std::io;
use zetesis_cli::{
    Completion, GroundingMode, Options, PhaseTimings, SolvePhase, SolveStage,
    run_detailed_with_diagnostics,
};
use zetesis_cpu::Control;

fn options(args: &[&str], enabled: bool) -> Options {
    let mut options = Options::try_parse_from(
        [
            "zetesis",
            "--backend",
            "cpu",
            "--workers",
            "1",
            "--models",
            "0",
        ]
        .into_iter()
        .chain(args.iter().copied()),
    )
    .unwrap();
    options.stats = enabled;
    options
}
fn partition(timings: &PhaseTimings) {
    let stages = timings.stages;
    assert_eq!(stages.driver_elapsed, timings.driver_elapsed);
    assert!(stages.is_complete());
    let mut elapsed = stages.unattributed.unwrap();
    for stage in SolveStage::ALL {
        if let Some(measurement) = stages.get(stage) {
            assert!(measurement.calls > 0);
            assert!(!measurement.overflowed);
            elapsed += measurement.elapsed;
        }
    }
    assert_eq!(elapsed, stages.driver_elapsed);
}

#[test]
fn eager_lazy_formula_certified_and_parallel_routes_preserve_results() {
    for (source, args, mode) in [
        (
            "p:-not q.q:-not p.",
            vec!["--grounder", "lazy"],
            GroundingMode::LazyInterleaved,
        ),
        (
            "p:-not q.q:-not p.",
            vec!["--grounder", "eager"],
            GroundingMode::Eager,
        ),
        ("1{p;q}1.", vec![], GroundingMode::Eager),
        (
            "a|b.",
            vec!["--oracle", "countermodel", "--completion-workers", "2"],
            GroundingMode::Eager,
        ),
        ("1{p;q}1.#minimize{1:p}.", vec![], GroundingMode::Eager),
    ] {
        let mut outputs = Vec::new();
        for enabled in [false, true] {
            let mut output = Vec::new();
            let mut diagnostics = Vec::new();
            let report = run_detailed_with_diagnostics(
                source.into(),
                &options(&args, enabled),
                &mut output,
                &mut diagnostics,
                &Control::default(),
            )
            .unwrap();
            assert_eq!(report.completion, Completion::Exhausted);
            let text = String::from_utf8(diagnostics).unwrap();
            assert_eq!(text.contains("Stage timings:"), enabled);
            if let Some(timings) = report.phase_timings {
                partition(&timings);
                assert_eq!(timings.stages.grounding_mode, mode);
                for stage in [
                    SolveStage::SourcePreparation,
                    SolveStage::Solving,
                    SolveStage::ObservationOutput,
                ] {
                    assert!(timings.stages.get(stage).is_some());
                }
                assert_eq!(
                    timings.stages.get(SolveStage::Grounding).is_some(),
                    mode == GroundingMode::Eager
                );
                assert!(
                    text.find("Stage timings:").unwrap() < text.find("Phase timings:").unwrap()
                );
                assert!(text.contains("failed_attempts=included; schema=2"));
                if source == "1{p;q}1." {
                    assert_eq!(
                        timings
                            .get(SolvePhase::AdmissionMaterialization)
                            .unwrap()
                            .calls,
                        2
                    );
                    assert_eq!(
                        timings
                            .stages
                            .get(SolveStage::SourcePreparation)
                            .unwrap()
                            .calls,
                        2
                    );
                    assert!(timings.get(SolvePhase::CertificateSetup).is_some());
                }
            } else {
                assert!(!enabled);
            }
            outputs.push(output);
        }
        assert_eq!(outputs[0], outputs[1], "{source}");
    }
}

#[test]
fn source_grounding_and_setup_failures_keep_only_entered_stages() {
    for (source, args, grounding, solving) in [
        ("a(.", vec![], false, false),
        (
            "1#sum{1:a;1:b}1.",
            vec!["--oracle", "countermodel"],
            true,
            false,
        ),
        (
            "p.",
            vec!["--grounder", "eager", "--max-ground-rules", "0"],
            true,
            true,
        ),
    ] {
        let mut output = Vec::new();
        let failure = run_detailed_with_diagnostics(
            source.into(),
            &options(&args, true),
            &mut output,
            &mut Vec::new(),
            &Control::default(),
        )
        .unwrap_err();
        assert!(!String::from_utf8(output).unwrap().contains("Answer:"));
        let timings = *failure.phase_timings.unwrap();
        partition(&timings);
        assert!(timings.stages.get(SolveStage::SourcePreparation).is_some());
        assert_eq!(
            timings.stages.get(SolveStage::Grounding).is_some(),
            grounding
        );
        assert_eq!(timings.stages.get(SolveStage::Solving).is_some(), solving);
        assert!(timings.stages.get(SolveStage::ObservationOutput).is_none());
    }
    let report = run_detailed_with_diagnostics(
        "a|b.".into(),
        &options(&["--max-search-work", "0"], true),
        &mut Vec::new(),
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    let timings = report.phase_timings.unwrap();
    partition(&timings);
    assert!(timings.stages.get(SolveStage::Solving).is_some());
    assert!(timings.get(SolvePhase::CandidateSetup).is_some());
}

struct Closed;
impl io::Write for Closed {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn early_cancellation_and_output_failure_retain_typed_attempts() {
    let control = Control::default();
    control.cancel();
    let report = run_detailed_with_diagnostics(
        "invalid input never admitted".into(),
        &options(&["--oracle", "countermodel"], true),
        &mut Vec::new(),
        &mut Vec::new(),
        &control,
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Interrupted);
    let timings = report.phase_timings.unwrap();
    partition(&timings);
    for stage in [
        SolveStage::SourcePreparation,
        SolveStage::Grounding,
        SolveStage::Solving,
    ] {
        assert!(timings.stages.get(stage).is_none());
    }
    assert_eq!(timings.stages.grounding_mode, GroundingMode::Unentered);
    assert!(timings.stages.get(SolveStage::ObservationOutput).is_some());
    let failure = run_detailed_with_diagnostics(
        "a|b.".into(),
        &options(&[], true),
        &mut Closed,
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap_err();
    assert!(matches!(*failure.cause, zetesis_cli::RunError::Output(_)));
    let timings = *failure.phase_timings.unwrap();
    partition(&timings);
    assert_eq!(
        timings
            .stages
            .get(SolveStage::ObservationOutput)
            .unwrap()
            .calls,
        1
    );
}

#[test]
fn original_bundle_formula_grounding_has_the_same_host_boundaries() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kr-domains/accepted/shortest-path-reachable.lp");
    let bundle =
        zetesis_themelios::SourceBundle::load(path, zetesis_themelios::BundleLimits::default())
            .unwrap();
    let report = zetesis_cli::run_bundle_detailed_with_diagnostics(
        bundle,
        &options(&["--oracle", "countermodel"], true),
        &mut Vec::new(),
        &mut Vec::new(),
        &Control::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    let timings = report.phase_timings.unwrap();
    partition(&timings);
    assert_eq!(timings.stages.grounding_mode, GroundingMode::Eager);
    assert_eq!(timings.stages.get(SolveStage::Grounding).unwrap().calls, 1);
}
