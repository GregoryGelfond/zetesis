//! Language admission and exact oracle selection for strings and original bundles.

use crate::presentation::{Diagnostics, Label};
use std::io::Write;

use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions, ExpansionLimits, ParsedSource,
    SourceBundle, SourceFailure, admit_bundle_extended,
};

use crate::SolvePhase;
use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::{Options, Oracle, PublicationFailure, RunError};

enum Input {
    Text(String),
    Parsed(ParsedSource),
}

pub(crate) fn source(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    crate::SolveConfig::from(options).validate()?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) =
            crate::publication::check_control(output, diagnostics, control, phases, options)?
    {
        return Ok(report);
    }
    let admission = AdmissionOptions {
        max_source_bytes: options.max_source_bytes,
        core_limits: core_limits(options),
        ..Default::default()
    };
    let source = if options.oracle == Oracle::Countermodel {
        Input::Text(source)
    } else {
        let source = phases
            .measure(SolvePhase::AdmissionMaterialization, || {
                ParsedSource::new(source, admission)
            })
            .map_err(|error| RunError::Expansion(error.into()))?;
        match phases.measure(SolvePhase::AdmissionMaterialization, || {
            source.admit_extended(expansion_limits(options))
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::publication::solve(
                    crate::PreparedInput::admitted(&admitted),
                    options,
                    output,
                    diagnostics,
                    control,
                    phases,
                )
                .map_err(|failure| source_failure(failure, "<input>", admitted.source()));
            }
            Err(error)
                if options.oracle == Oracle::Auto && error.error().needs_formula_admission() =>
            {
                Input::Parsed(error.into_source())
            }
            Err(error) => return Err(RunError::Expansion(error.into_error()).into()),
        }
    };
    crate::SolveConfig::from(options).validate_formula()?;
    if let Some(report) =
        crate::publication::check_control(output, diagnostics, control, phases, options)?
    {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted = phases
        .measure(SolvePhase::AdmissionMaterialization, || {
            let parsed = match source {
                Input::Text(text) => ParsedSource::new(text, admission)?,
                Input::Parsed(parsed) => parsed,
            };
            parsed
                .prepare_formula(expansion_limits(options), formula_limits(options))
                .map_err(SourceFailure::into_error)?
                .with_grounding_options(grounding_options(options))
                .ground_with_observer(
                    observer
                        .as_ref()
                        .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver),
                )
        })
        .map_err(RunError::FormulaAdmission)?;
    crate::publication::solve(
        crate::PreparedInput::formula(&admitted),
        options,
        output,
        diagnostics,
        control,
        phases,
    )
    .map_err(|failure| source_failure(failure, "<input>", admitted.source()))
}

pub(crate) fn bundle(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    crate::SolveConfig::from(options).validate()?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) =
            crate::publication::check_control(output, diagnostics, control, phases, options)?
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
            admit_bundle_extended(bundle, bundle_options(options), expansion_limits(options))
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::publication::solve(
                    crate::PreparedInput::bundle(&admitted),
                    options,
                    output,
                    diagnostics,
                    control,
                    phases,
                )
                .map_err(|failure| bundle_failure(failure, admitted.bundle()));
            }
            Err(error)
                if options.oracle == Oracle::Auto
                    && matches!(error.error(), BundleAdmissionError::Expansion(expansion)
                    if expansion.needs_formula_admission()) =>
            {
                error.into_bundle()
            }
            Err(error) => return Err(RunError::BundleAdmission(error).into()),
        }
    };
    crate::SolveConfig::from(options).validate_formula()?;
    if let Some(report) =
        crate::publication::check_control(output, diagnostics, control, phases, options)?
    {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted = phases
        .measure(SolvePhase::AdmissionMaterialization, || {
            zetesis_themelios::prepare_bundle_formula(
                bundle,
                bundle_options(options),
                expansion_limits(options),
                formula_limits(options),
            )?
            .with_grounding_options(grounding_options(options))
            .ground_with_observer(
                observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver),
            )
        })
        .map_err(RunError::FormulaBundleAdmission)?;
    crate::publication::solve(
        crate::PreparedInput::formula_bundle(&admitted),
        options,
        output,
        diagnostics,
        control,
        phases,
    )
    .map_err(|failure| bundle_failure(failure, admitted.bundle()))
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
    ExpansionLimits {
        max_term_work: options
            .max_expansion_work
            .unwrap_or_else(|| ExpansionLimits::default().max_term_work),
        max_templates: options.max_expanded_templates,
        max_values: options.max_expansion_values,
        ..Default::default()
    }
}

fn grounding_options(options: &Options) -> zetesis_themelios::GroundingOptions {
    zetesis_themelios::GroundingOptions {
        joins: options.formula_joins,
    }
}

pub(crate) fn formula_limits(options: &Options) -> zetesis_themelios::FormulaLimits {
    zetesis_themelios::FormulaLimits {
        max_domain_values: options
            .max_domain_values
            .unwrap_or_else(|| zetesis_themelios::FormulaLimits::default().max_domain_values),
        max_assignment_values: options.max_assignment_values,
        max_generated_values: options.max_generated_values,
        max_support_rounds: options.max_support_rounds,
        max_support_bytes: options.max_support_bytes,
        max_substitutions: u64::try_from(options.max_substitutions).unwrap_or(u64::MAX),
        max_work: options.max_expansion_work.map_or_else(
            || zetesis_themelios::FormulaLimits::default().max_work,
            |work| u64::try_from(work).unwrap_or(u64::MAX),
        ),
        theory: zetesis_ferraris::AdmissionLimits {
            max_atoms: options.max_atoms,
            max_roots: options.max_ground_rules,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn core_limits(options: &Options) -> zetesis_core::AdmissionLimits {
    zetesis_core::AdmissionLimits {
        max_domain_values: options
            .max_domain_values
            .unwrap_or_else(|| zetesis_core::AdmissionLimits::default().max_domain_values),
        ..Default::default()
    }
}

fn bundle_options(options: &Options) -> BundleAdmissionOptions {
    BundleAdmissionOptions {
        core_limits: core_limits(options),
        ..Default::default()
    }
}
