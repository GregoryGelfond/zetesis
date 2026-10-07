//! Exercise publication with explicit library bounds, independently of CLI policy.

use std::io::{self, Write};

use zetesis_cpu::Cancellation;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

use crate::{PublicationConfig, PublicationFailure, PublicationOutcome};

pub(crate) struct FormulaCase {
    pub(crate) config: PublicationConfig,
    pub(crate) presentation: crate::Options,
}

impl FormulaCase {
    pub(crate) fn new(presentation: crate::Options) -> Self {
        Self {
            config: PublicationConfig::from(&presentation),
            presentation,
        }
    }

    pub(crate) fn run(
        &self,
        source: &str,
        output: &mut impl Write,
        diagnostics: &mut impl Write,
        cancellation: &Cancellation,
    ) -> Result<PublicationOutcome, PublicationFailure> {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let mut config = self.config.clone();
        config.solve.stats = self.presentation.stats;
        with_diagnostics(
            crate::PreparedInput::formula(&admitted),
            &config,
            &self.presentation,
            output,
            diagnostics,
            cancellation,
        )
    }
}

/// Drive the real publication and reporting controllers with explicit library
/// bounds. Options govern presentation only; no removed CLI flag is emulated.
pub(crate) fn with_diagnostics(
    input: crate::PreparedInput<'_>,
    config: &PublicationConfig,
    options: &crate::Options,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<PublicationOutcome, PublicationFailure> {
    let mut renderer = crate::view::builtin::Builtin::new(output, options);
    let mut invocation = crate::view::session::Session::start(&mut renderer)?;
    let phases = crate::phase_timing::recorder(config.solve.stats, &invocation);
    let mut diagnostics = crate::presentation::Diagnostics::for_solve(
        diagnostics,
        options.color.human(options.json),
        options,
    );
    let result = crate::publication::solve(
        input,
        None,
        config,
        &mut invocation,
        &mut diagnostics,
        cancellation,
        &phases,
    );
    let result = super::report_progress_statistics(result, &mut diagnostics, options, &phases);
    crate::publication::finalize(&mut invocation, result)
}

pub(crate) fn formula(
    source: &str,
    config: &PublicationConfig,
    renderer: &mut impl crate::AnswerRenderer,
    cancellation: &Cancellation,
) -> Result<PublicationOutcome, PublicationFailure> {
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    crate::publish_prepared(
        crate::PreparedInput::formula(&admitted),
        config,
        renderer,
        &mut io::sink(),
        cancellation,
    )
}

pub(crate) fn human(
    source: &str,
    config: &PublicationConfig,
    output: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<PublicationOutcome, PublicationFailure> {
    formula(
        source,
        config,
        &mut crate::HumanRenderer::new(
            output,
            crate::ColorMode::Never,
            config.observations.max_output_bytes,
        ),
        cancellation,
    )
}

pub(crate) fn json(
    source: &str,
    config: &PublicationConfig,
    record_bytes: usize,
    output: &mut impl Write,
    cancellation: &Cancellation,
) -> Result<PublicationOutcome, PublicationFailure> {
    formula(
        source,
        config,
        &mut crate::JsonRenderer::new(output, record_bytes, config.solve.max_atoms),
        cancellation,
    )
}
