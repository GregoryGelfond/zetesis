//! Language admission and exact oracle selection for strings and original bundles.

use crate::presentation::{Diagnostics, Label};
use std::io::Write;

use zetesis_cpu::Cancellation;
use zetesis_themelios::{
    BundleAdmissionError, BundleAdmissionOptions, ExpansionLimits, ParsedSource, SourceBundle,
    SourceFailure, admit_bundle_extended_with_cancellation,
};

use crate::SolvePhase;
use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::{Options, Oracle, PublicationFailure, RunError};

enum Input {
    Text(String),
    Parsed(ParsedSource),
}

/// A formula program materialized by the library for the requested grounder;
/// the CLI only presents its warnings and failures.
struct FormulaInput(zetesis_solve::GroundedFormula);

impl FormulaInput {
    fn prepared(&self) -> crate::PreparedInput<'_> {
        self.0.input()
    }

    fn warnings(&self, diagnostics: &mut Diagnostics<impl Write>) -> std::io::Result<()> {
        use zetesis_solve::GroundedFormula as Grounded;
        match &self.0 {
            Grounded::Source(owner) if !owner.warnings().is_empty() => {
                diagnostics.diagnostic(&owner.warning_view())
            }
            Grounded::Bundle(owner) if !owner.warnings().is_empty() => {
                diagnostics.diagnostic(&owner.warning_view())
            }
            Grounded::Hybrid(owner) if !owner.warnings().is_empty() => {
                diagnostics.diagnostic(&owner.warning_view())
            }
            Grounded::Terminal(owner) if !owner.warnings().is_empty() => {
                diagnostics.diagnostic(&owner.warning_view())
            }
            _ => Ok(()),
        }
    }

    fn retain_source(&self, failure: PublicationFailure) -> PublicationFailure {
        use zetesis_solve::GroundedFormula as Grounded;
        match &self.0 {
            Grounded::Source(owner) => {
                if let Some(source) = owner.source() {
                    source_failure(failure, "<input>", source)
                } else {
                    failure
                }
            }
            Grounded::Bundle(owner) => bundle_failure(failure, owner.bundle()),
            Grounded::Hybrid(owner) => {
                if let Some(bundle) = owner.bundle() {
                    bundle_failure(failure, bundle)
                } else if let Some(source) = owner.source() {
                    source_failure(failure, "<input>", source)
                } else {
                    failure
                }
            }
            Grounded::Terminal(owner) => {
                if let Some(bundle) = owner.bundle() {
                    bundle_failure(failure, bundle)
                } else if let Some(source) = owner.source() {
                    source_failure(failure, "<input>", source)
                } else {
                    failure
                }
            }
        }
    }

    fn solve(
        &self,
        options: &Options,
        renderer: &mut impl crate::AnswerRenderer,
        diagnostics: &mut Diagnostics<impl Write>,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Result<Progress, PublicationFailure> {
        self.warnings(diagnostics)?;
        crate::publication::solve(
            self.prepared(),
            None,
            &crate::PublicationConfig::from(options),
            renderer,
            diagnostics,
            cancellation,
            phases,
        )
        .map_err(|failure| self.retain_source(failure))
    }
}

fn validate_formula(options: &Options) -> Result<(), RunError> {
    let config = crate::SolveConfig::from(options);
    if options.grounder == crate::Grounder::Lazy {
        config.validate_hybrid().map_err(Into::into)
    } else {
        config.validate_formula().map_err(Into::into)
    }
}

pub(crate) fn source(
    source: String,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    crate::SolveConfig::from(options).validate()?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) =
            crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
    {
        return Ok(report);
    }
    let admission = options.resources().admission_options();
    let source = if options.oracle == Oracle::Countermodel {
        Input::Text(source)
    } else {
        let source = phases
            .measure(SolvePhase::AdmissionMaterialization, || {
                ParsedSource::new(source, admission)
            })
            .map_err(|error| RunError::Expansion(error.into()))?;
        match phases.measure(SolvePhase::AdmissionMaterialization, || {
            source.admit_extended_with_cancellation(expansion_limits(options), cancellation)
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::publication::solve(
                    crate::PreparedInput::admitted(&admitted),
                    Some(*admitted.expansion_usage()),
                    &crate::PublicationConfig::from(options),
                    renderer,
                    diagnostics,
                    cancellation,
                    phases,
                )
                .map_err(|failure| source_failure(failure, "<input>", admitted.source()));
            }
            Err(error) => {
                if let Some(reason) = expansion_control(error.error()) {
                    return crate::publication::interrupted(reason, renderer, diagnostics, phases);
                }
                if options.oracle == Oracle::Auto && error.error().needs_formula_admission() {
                    Input::Parsed(error.into_source())
                } else {
                    return Err(RunError::Expansion(error.into_error()).into());
                }
            }
        }
    };
    validate_formula(options)?;
    if let Some(report) =
        crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
    {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted: Result<FormulaInput, zetesis_themelios::FormulaFailure> =
        phases.measure(SolvePhase::AdmissionMaterialization, || {
            let parsed = match source {
                Input::Text(text) => ParsedSource::new(text, admission)?,
                Input::Parsed(parsed) => parsed,
            };
            let prepared = parsed
                .prepare_formula_with_cancellation(
                    expansion_limits(options),
                    formula_limits(options),
                    cancellation,
                )
                .map_err(SourceFailure::into_error)?
                .with_grounding_options(grounding_options(options))
                .with_domain_analysis(Some(zetesis_themelios::DomainLimits::default()));
            let observer = observer
                .as_ref()
                .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver);
            zetesis_solve::ground_formula(prepared, options.grounder, observer).map(FormulaInput)
        });
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(error) => {
            if let Some(reason) = error.interruption() {
                return crate::publication::interrupted(reason, renderer, diagnostics, phases);
            }
            return Err(RunError::FormulaAdmission(error).into());
        }
    };
    admitted.solve(options, renderer, diagnostics, cancellation, phases)
}

pub(crate) fn bundle(
    bundle: SourceBundle,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    crate::SolveConfig::from(options).validate()?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) =
            crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
    {
        return Ok(report);
    }
    diagnostics.metadata(
        Label::Source,
        format_args!(
            "{} original {} ({} bytes)",
            bundle.sources().len(),
            if bundle.sources().len() == 1 {
                "file"
            } else {
                "files"
            },
            bundle.total_bytes()
        ),
    )?;
    let bundle = if options.oracle == Oracle::Countermodel {
        bundle
    } else {
        match phases.measure(SolvePhase::AdmissionMaterialization, || {
            admit_bundle_extended_with_cancellation(
                bundle,
                bundle_options(options),
                expansion_limits(options),
                cancellation,
            )
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::publication::solve(
                    crate::PreparedInput::bundle(&admitted),
                    Some(*admitted.expansion_usage()),
                    &crate::PublicationConfig::from(options),
                    renderer,
                    diagnostics,
                    cancellation,
                    phases,
                )
                .map_err(|failure| bundle_failure(failure, admitted.bundle()));
            }
            Err(error) => {
                if let BundleAdmissionError::Expansion(expansion) = error.error()
                    && let Some(reason) = expansion_control(expansion)
                {
                    return crate::publication::interrupted(reason, renderer, diagnostics, phases);
                }
                if options.oracle == Oracle::Auto
                    && matches!(error.error(), BundleAdmissionError::Expansion(expansion)
            if expansion.needs_formula_admission())
                {
                    error.into_bundle()
                } else {
                    return Err(RunError::BundleAdmission(error).into());
                }
            }
        }
    };
    validate_formula(options)?;
    if let Some(report) =
        crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
    {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted: Result<FormulaInput, zetesis_themelios::FormulaBundleFailure> =
        phases.measure(SolvePhase::AdmissionMaterialization, || {
            let prepared = zetesis_themelios::prepare_bundle_formula_with_cancellation(
                bundle,
                bundle_options(options),
                expansion_limits(options),
                formula_limits(options),
                cancellation,
            )?
            .with_grounding_options(grounding_options(options))
            .with_domain_analysis(Some(zetesis_themelios::DomainLimits::default()));
            let observer = observer
                .as_ref()
                .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver);
            zetesis_solve::ground_bundle(prepared, options.grounder, observer).map(FormulaInput)
        });
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(error) => {
            if let Some(reason) = error.error().interruption() {
                return crate::publication::interrupted(reason, renderer, diagnostics, phases);
            }
            return Err(RunError::FormulaBundleAdmission(error).into());
        }
    };
    admitted.solve(options, renderer, diagnostics, cancellation, phases)
}

fn expansion_control(error: &zetesis_themelios::ExpansionFailure) -> Option<zetesis_cpu::Stop> {
    if let zetesis_themelios::ExpansionFailure::Interrupted {
        reason: reason @ (zetesis_cpu::Stop::Cancelled | zetesis_cpu::Stop::Deadline),
        ..
    } = error
    {
        Some(*reason)
    } else {
        None
    }
}

fn source_failure(
    mut failure: PublicationFailure,
    name: &str,
    source: &zetesis_themelios::base::source::Source,
) -> PublicationFailure {
    if let RunError::Observation(error) = failure.cause.as_mut() {
        error.retain_source(name, source);
    }
    failure
}

fn bundle_failure(mut failure: PublicationFailure, bundle: &SourceBundle) -> PublicationFailure {
    use zetesis_themelios::base::source::Sources;

    if let RunError::Observation(error) = failure.cause.as_mut()
        && let Some(location) = error.location()
        && let Some(source) = bundle.get(location.source)
        && let Some(name) = bundle.name(location.source)
    {
        error.retain_source(name, source.source());
    }
    failure
}

pub(crate) fn expansion_limits(options: &Options) -> ExpansionLimits {
    options.resources().expansion_limits()
}

fn grounding_options(options: &Options) -> zetesis_themelios::GroundingOptions {
    zetesis_themelios::GroundingOptions {
        joins: options.formula_joins,
    }
}

pub(crate) fn formula_limits(options: &Options) -> zetesis_themelios::FormulaLimits {
    options.resources().formula_limits()
}

fn bundle_options(options: &Options) -> BundleAdmissionOptions {
    let admission = options.resources().admission_options();
    BundleAdmissionOptions {
        max_syntax_nodes: admission.max_syntax_nodes,
        max_syntax_depth: admission.max_syntax_depth,
        max_body_elements: admission.max_body_elements,
        core_limits: admission.core_limits,
    }
}
