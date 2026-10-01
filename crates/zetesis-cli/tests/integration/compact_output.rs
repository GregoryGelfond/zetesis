//! Compact human summaries remain separate from optional detailed measurements.

use crate::support::options::serial;
use std::{
    io,
    time::{Duration, Instant},
};
use zetesis_cli::{
    AnswerRenderer, AnswerView, BackendView, ColorMode, Completion, ConfigurationView,
    GroundingDisplay, GroundingMode, JsonRenderer, Options, PreparedInput, PublicationConfig,
    PublicationView, Report, RunError, SolvePhase, SolveStage, StatisticsView, SummaryDelivery,
    publish_prepared, run_with_diagnostics, run_with_renderer,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, GroundingPhase, admit_formula,
};

struct Rendered {
    report: Report,
    answers: String,
    diagnostics: String,
}

fn solve(source: &str, options: &Options) -> Rendered {
    let mut answers = Vec::new();
    let mut diagnostics = Vec::new();
    let report = run_with_diagnostics(
        source.into(),
        options,
        &mut answers,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .unwrap();
    Rendered {
        report,
        answers: String::from_utf8(answers).unwrap(),
        diagnostics: String::from_utf8(diagnostics).unwrap(),
    }
}

#[test]
fn eager_summary_reports_separate_stage_times() {
    let rendered = solve("p.", &serial(&["--grounder", "eager"]));
    assert_eq!(rendered.report.completion, Completion::Exhausted);
    assert_eq!(rendered.report.models, 1);
    assert_eq!(
        rendered.report.phase_timings.unwrap().stages.grounding_mode,
        GroundingMode::Eager
    );
    let (summary, timing) = rendered.answers.split_once("\nTime: ").unwrap();
    assert!(summary.starts_with(&format!("zetesis {}", env!("CARGO_PKG_VERSION"))));
    assert!(summary.contains("Gregory Gelfond"));
    assert!(summary.contains("MIT"));
    assert!(summary.contains("\nBackend: CPU · 1 thread · eager grounding\n\n"));
    assert!(summary.ends_with("Answer: 1\np\nSATISFIABLE\nModels: 1\n"));
    assert!(timing.starts_with("grounding "));
    assert!(timing.contains(" ms · solving "));
    assert!(timing.ends_with(" ms\n"));
    assert!(!timing.contains("unavailable"));
    assert!(rendered.diagnostics.is_empty());
}

#[test]
fn lazy_summary_reports_interleaved_work() {
    let rendered = solve("p:-not q. q:-not p.", &serial(&["--grounder", "lazy"]));
    assert_eq!(rendered.report.completion, Completion::Exhausted);
    assert_eq!(rendered.report.models, 2);
    let stages = rendered.report.phase_timings.unwrap().stages;
    assert_eq!(stages.grounding_mode, GroundingMode::LazyInterleaved);
    assert!(stages.get(SolveStage::Grounding).is_none());
    assert!(stages.get(SolveStage::Solving).is_some());
    assert!(
        rendered
            .answers
            .contains("\nBackend: CPU · 1 thread · lazy grounding\n\n")
    );
    let (summary, timing) = rendered.answers.split_once("\nTime: ").unwrap();
    assert!(summary.ends_with("SATISFIABLE\nModels: 2\n"));
    assert!(timing.starts_with("grounding + solving "));
    assert!(timing.ends_with(" ms\n"));
    assert!(!timing.contains("unavailable"));
    assert!(rendered.diagnostics.is_empty());
}

#[test]
fn default_summary_does_not_enable_detailed_clocks() {
    // This formula exercises eager grounding and a general reduct query, so
    // absent detail measurements cannot be explained by an inapplicable route.
    let rendered = solve("a|b. a:-b. b:-a.", &serial(&["--oracle", "countermodel"]));
    assert_eq!(rendered.report.completion, Completion::Exhausted);
    assert_eq!(rendered.report.models, 1);
    let timings = rendered.report.phase_timings.unwrap();
    assert!(timings.stages.get(SolveStage::Grounding).is_some());
    assert!(timings.stages.get(SolveStage::Solving).is_some());
    for phase in SolvePhase::ALL {
        assert!(timings.get(phase).is_none());
    }
    for phase in GroundingPhase::ALL {
        assert!(timings.grounding.get(phase).is_none());
    }
    assert!(
        rendered
            .report
            .countermodel_statistics
            .unwrap()
            .phase_timings
            .is_none()
    );
    assert!(rendered.diagnostics.is_empty());
}

fn human_statistics(color: ColorMode) -> Rendered {
    let mut options = serial(&["--grounder", "eager"]);
    options.stats = true;
    options.statistics_view = StatisticsView::Human;
    options.color = color;
    solve("p.", &options)
}

#[test]
fn human_statistics_remain_separate_from_answers() {
    let rendered = human_statistics(ColorMode::Never);
    assert_eq!(rendered.report.completion, Completion::Exhausted);
    assert!(
        rendered
            .answers
            .contains("SATISFIABLE\nModels: 1\n\nTime: ")
    );
    assert!(!rendered.answers.contains("Statistics"));
    assert!(rendered.diagnostics.starts_with("\nStatistics\n"));
    for label in [
        "Stage",
        "Time (ms)",
        "Execution",
        "Work",
        "Recorded execution",
    ] {
        assert!(rendered.diagnostics.contains(label));
    }
    for label in [
        "Source:",
        "Oracle:",
        "Grounding:",
        "Backend:",
        "Answer:",
        "Time:",
    ] {
        assert!(!rendered.diagnostics.contains(label));
    }
}

#[test]
fn statistics_tables_never_use_italics() {
    let rendered = human_statistics(ColorMode::Always);
    assert!(rendered.answers.contains("\u{1b}[3;90m"));
    assert!(rendered.diagnostics.contains("\u{1b}[34m"));
    // The table palette uses simple SGR attributes. Italic is attribute 3;
    // blue and gray remain available without inheriting metadata italics.
    for styled in rendered.diagnostics.split("\u{1b}[").skip(1) {
        let (attributes, _) = styled.split_once('m').unwrap();
        assert!(!attributes.split(';').any(|attribute| attribute == "3"));
    }
}

fn unmeasured_json(bytes: &[u8]) {
    let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(document["models"].as_array().unwrap().len(), 1);
    assert_eq!(document["outcome"]["completion"], "exhausted");
    assert_eq!(document.get("statistics"), Some(&serde_json::Value::Null));
}

#[test]
fn source_json_renderer_does_not_request_timings() {
    let options = serial(&[]);
    assert!(!options.json);
    assert!(!options.stats);
    let mut renderer = JsonRenderer::new(Vec::new(), 8192, 1024);
    let outcome = run_with_renderer(
        "p.".into(),
        &options,
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(outcome.report().unwrap().phase_timings.is_none());
    unmeasured_json(&renderer.into_inner());
}

#[test]
fn prepared_json_renderer_does_not_request_timings() {
    let admitted = admit_formula(
        "p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let config = PublicationConfig {
        solve: crate::support::options::serial_config(),
        ..Default::default()
    };
    assert!(!config.solve.stats);
    let mut renderer = JsonRenderer::new(Vec::new(), 8192, 1024);
    let outcome = publish_prepared(
        PreparedInput::formula(&admitted),
        &config,
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(outcome.report().unwrap().phase_timings.is_none());
    unmeasured_json(&renderer.into_inner());
}

#[derive(Default)]
struct ConfigurationTime {
    elapsed: Option<Duration>,
}

impl AnswerRenderer for ConfigurationTime {
    fn needs_stage_timings(&self) -> bool {
        true
    }

    fn configuration(&mut self, view: ConfigurationView<'_>) -> Result<(), RunError> {
        assert_eq!(view.backend, BackendView::Cpu);
        assert_eq!(view.grounding, GroundingDisplay::Eager);
        assert_eq!(self.elapsed, None);
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(10));
        self.elapsed = Some(start.elapsed());
        Ok(())
    }

    fn answer(&mut self, _: AnswerView<'_>, _: &Cancellation) -> Result<(), RunError> {
        Ok(())
    }

    fn finish(&mut self, _: PublicationView<'_>) -> Result<SummaryDelivery, RunError> {
        Ok(SummaryDelivery::Accepted)
    }
}

#[test]
fn configuration_callback_is_output_work() {
    let mut renderer = ConfigurationTime::default();
    let outcome = run_with_renderer(
        "p.".into(),
        &serial(&["--grounder", "eager"]),
        &mut renderer,
        &mut io::sink(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(outcome.semantic().completion(), Some(Completion::Exhausted));
    let stages = outcome.report().unwrap().phase_timings.unwrap().stages;
    assert!(stages.is_complete());
    let observation = stages.get(SolveStage::ObservationOutput).unwrap();
    assert!(!observation.overflowed);
    // Compare nested intervals from the same monotonic clock. The assertion has
    // no deadline or precision tolerance and makes no solver-speed assumption.
    assert!(observation.elapsed >= renderer.elapsed.unwrap());
}
