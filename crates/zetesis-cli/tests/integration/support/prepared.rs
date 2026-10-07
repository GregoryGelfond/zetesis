//! Typed failure injection after canonical source preparation. No removed CLI
//! flag is parsed here; tests set the existing publication or renderer limits.

use std::io::Write;
use zetesis_cli::{
    AnswerRenderer, ColorMode, HumanRenderer, PublicationConfig, PublicationOutcome,
    PublicationReport, Report, RunFailure, publish_prepared,
};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, ParsedSource};

pub fn config(arguments: &[&str]) -> PublicationConfig {
    PublicationConfig::from(&super::options::serial(arguments))
}

/// These callers test publication of a supported formula, independently of
/// source-admission failures and the CLI's relational/formula route selection.
pub fn formula(
    source: &str,
    config: &PublicationConfig,
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    let prepared = ParsedSource::new(source.into(), AdmissionOptions::default())
        .unwrap()
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap();
    let admitted = zetesis_solve::ground_formula(prepared, config.solve.grounder, None).unwrap();
    publish_prepared(
        admitted.input(),
        config,
        renderer,
        diagnostics,
        cancellation,
    )
    .and_then(PublicationOutcome::into_legacy)
    .map(PublicationReport::into_report)
    .map_err(zetesis_cli::PublicationFailure::into_legacy)
}

pub fn human(
    source: &str,
    config: &PublicationConfig,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    formula(
        source,
        config,
        &mut HumanRenderer::new(
            output,
            ColorMode::Never,
            config.observations.max_output_bytes,
        ),
        diagnostics,
        cancellation,
    )
}

/// Explicit relational admission for closure tests; no formula fallback.
pub fn relational(
    source: &str,
    config: &PublicationConfig,
    renderer: &mut impl AnswerRenderer,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    let admitted = ParsedSource::new(source.into(), AdmissionOptions::default())
        .unwrap()
        .admit_extended(ExpansionLimits::default())
        .unwrap();
    publish_prepared(
        zetesis_cli::PreparedInput::admitted(&admitted),
        config,
        renderer,
        diagnostics,
        cancellation,
    )
    .and_then(PublicationOutcome::into_legacy)
    .map(PublicationReport::into_report)
    .map_err(zetesis_cli::PublicationFailure::into_legacy)
}

pub fn formula_run(
    source: &str,
    arguments: &[&str],
    configure: impl FnOnce(&mut PublicationConfig),
) -> (Result<Report, zetesis_cli::RunError>, String, String) {
    let mut config = config(arguments);
    configure(&mut config);
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = human(
        source,
        &config,
        &mut output,
        &mut diagnostics,
        &Cancellation::default(),
    )
    .map_err(|failure| *failure.cause);
    (
        result,
        String::from_utf8(output).unwrap(),
        String::from_utf8(diagnostics).unwrap(),
    )
}

pub fn relational_human(
    source: &str,
    config: &PublicationConfig,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<Report, RunFailure> {
    relational(
        source,
        config,
        &mut HumanRenderer::new(
            output,
            ColorMode::Never,
            config.observations.max_output_bytes,
        ),
        diagnostics,
        cancellation,
    )
}

/// Bounded source admission remains a typed library operation. A refusal never
/// reaches the renderer; the ordinary CLI's policy is tested separately.
pub fn bounded_admission(
    source: &str,
    expansion: ExpansionLimits,
    limits: &FormulaLimits,
) -> (Result<Report, zetesis_cli::RunError>, String) {
    let mut output = Vec::new();
    let result = (|| {
        let admitted = zetesis_themelios::admit_formula(
            source.into(),
            AdmissionOptions::default(),
            expansion,
            *limits,
        )
        .map_err(zetesis_cli::RunError::FormulaAdmission)?;
        let config = config(&["--oracle", "countermodel"]);
        publish_prepared(
            zetesis_cli::PreparedInput::formula(&admitted),
            &config,
            &mut HumanRenderer::new(
                &mut output,
                ColorMode::Never,
                config.observations.max_output_bytes,
            ),
            &mut std::io::sink(),
            &Cancellation::default(),
        )
        .and_then(PublicationOutcome::into_legacy)
        .map(PublicationReport::into_report)
        .map_err(|failure| *failure.into_legacy().cause)
    })();
    (result, String::from_utf8(output).unwrap())
}
