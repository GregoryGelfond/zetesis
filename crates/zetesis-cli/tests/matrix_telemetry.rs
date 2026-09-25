//! Actual CLI publication is accepted by the maintained matrix telemetry decoder.
//! These in-process controls establish neither process timing nor device execution.
use clap::Parser;
use serde_json::Value;
use zetesis_cli::{Completion, Options, Report, SolvePhase, run_detailed_with_diagnostics};
use zetesis_cpu::Cancellation;
use zetesis_sat::CertificatePlanStatistics;
use zetesis_validation::{
    performance::matrix::{DeviceWork, Observation, Procedure},
    selected::{NativeExecution, Oracle},
};

fn capture(source: &str, oracle: &str, workers: &str) -> (Report, Value, Vec<u8>) {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "eager",
        "--search",
        "clauses",
        "--oracle",
        oracle,
        "--workers",
        "1",
        "--completion-workers",
        workers,
        "--models",
        "0",
        "--json",
        "--stats",
    ])
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        source.into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    (
        report,
        serde_json::from_slice(&output).unwrap(),
        diagnostics,
    )
}

#[test]
fn positive_publication_retains_its_actual_matrix_procedure() {
    // This scored cycle is also checked through the public formula-session door
    // in positive_sessions. Objectives require formula admission from the CLI;
    // actual certificate statistics, rather than source shape, establish its route.
    for workers in ["1", "2"] {
        let (report, document, text) = capture(
            "a. b:-a. a:-b. #minimize {2@3,k:b;1@1,k:a}. #show a/0.",
            "auto",
            workers,
        );
        assert_eq!(report.models, 1);
        let certified = report.countermodel_statistics.unwrap().certified.unwrap();
        assert!(
            matches!(certified.plan, Some(CertificatePlanStatistics::Positive(_))),
            "actual certificate: {certified:?}"
        );
        assert!(certified.checks > 0);
        assert!(
            report
                .phase_timings
                .unwrap()
                .get(SolvePhase::ReductPreparation)
                .is_none()
        );
        let observation =
            Observation::from_statistics(&document, &text, NativeExecution::default()).unwrap();
        assert_eq!(
            observation.execution.procedure,
            Procedure::PositiveConsequences
        );
        assert!(matches!(observation.execution.device, DeviceWork::Cpu));
        assert_eq!(observation.timing.phase_schema, 4);
        assert!(observation.timing.phases["reduct_preparation"].is_none());
    }
}

#[test]
fn residual_publication_retains_cold_preparation_measurement() {
    for workers in ["1", "2"] {
        let (report, document, text) = capture("a | b.", "countermodel", workers);
        assert_eq!(report.models, 2);
        let search = report.countermodel_statistics.unwrap();
        assert!(search.countermodel_queries > 0);
        let timings = report.phase_timings.unwrap();
        let preparation = timings.get(SolvePhase::ReductPreparation).unwrap();
        assert_eq!(preparation.calls, 1);
        let request = NativeExecution {
            oracle: Oracle::Countermodel,
            ..Default::default()
        };
        let observation = Observation::from_statistics(&document, &text, request).unwrap();
        assert_eq!(observation.execution.procedure, Procedure::Countermodel);
        assert_eq!(observation.timing.phase_schema, 4);
        let observed = observation.timing.phases["reduct_preparation"]
            .as_ref()
            .unwrap();
        assert_eq!(observed.calls, preparation.calls);
        assert_eq!(
            u128::from(observed.elapsed_ns),
            preparation.elapsed.as_nanos()
        );
    }
}

#[test]
fn terminal_publication_retains_base_and_reconstruction_scopes() {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--grounder",
        "auto",
        "--workers",
        "1",
        "--models",
        "0",
        "--json",
        "--stats",
    ])
    .unwrap();
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_detailed_with_diagnostics(
        "{seed(1);seed(2)}. receipt(X):-seed(X). #show X:receipt(X).".into(),
        &options,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    assert_eq!(report.models, 4);
    let receipt = report.terminal_execution.unwrap();
    assert_eq!(
        (receipt.base_answers, receipt.reconstructed, receipt.pending),
        (4, 4, 0)
    );
    let document: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["statistics"]["search"]["scope"], "terminal_base");
    assert_eq!(document["outcome"]["verified_models"], 4);
    assert_eq!(
        document["statistics"]["terminal_execution"]["reconstruction"]["work"],
        receipt.reconstruction.work
    );
    let observation = Observation::from_statistics(
        &document,
        &diagnostics,
        NativeExecution {
            grounder: zetesis_validation::selected::Grounder::Auto,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        observation.timing.grounding_mode,
        "eager_base_terminal_definitions"
    );
    assert_eq!(observation.terminal.unwrap().reconstructed, 4);
    let text = std::str::from_utf8(&diagnostics).unwrap();
    assert!(text.contains("base only; terminal definitions reconstructed during solving"));
    assert!(text.contains("verified base models=4"));
    assert!(text.contains("full reconstruction before original membership"));
}

#[path = "support/terminal_refusal.rs"]
mod terminal_refusal;
