//! Language admission and exact oracle selection for strings and original bundles.

use crate::presentation::{Diagnostics, Label};
use std::io::Write;

use zetesis_cpu::Cancellation;
use zetesis_themelios::{
    AutomaticAdmission, AutomaticAdmissionFailure, BundleAdmissionError, BundleAdmissionOptions,
    ExpansionLimits, ParsedSource, SourceBundle, SourceFailure,
    admit_bundle_automatic_with_cancellation, admit_bundle_extended_with_cancellation,
};

use crate::SolvePhase;
use crate::failure::Progress;
use crate::phase_timing::Recorder;
use crate::{Options, Oracle, PublicationFailure, RunError};

/// A program prepared by the shared library execution policy, with how its
/// preparation compiled its comparisons; the CLI only presents its warnings,
/// receipts and failures.
struct FormulaInput(
    zetesis_solve::PreparedExecution,
    zetesis_themelios::ComparisonForms,
);

impl FormulaInput {
    fn prepared(&self) -> crate::PreparedInput<'_> {
        self.0.input()
    }

    fn warnings(&self, diagnostics: &mut Diagnostics<impl Write>) -> std::io::Result<()> {
        use zetesis_solve::GroundedFormula as Grounded;
        let grounded = match &self.0 {
            zetesis_solve::PreparedExecution::Source(owner) => {
                return if owner.warnings().is_empty() {
                    Ok(())
                } else {
                    diagnostics.diagnostic(&owner.warning_view())
                };
            }
            zetesis_solve::PreparedExecution::Grounded(grounded) => grounded.as_ref(),
        };
        match grounded {
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

    /// The preparation receipt of the integer comparison forms, as a record.
    fn comparison_forms(&self, diagnostics: &mut Diagnostics<impl Write>) -> std::io::Result<()> {
        let forms = self.1;
        diagnostics.metadata(
            Label::Comparisons,
            format_args!(
                "with_form={}; long_operand={}; wide_slot={}; constructor={}; nonnumeric_constant={}; argument_position={}",
                forms.with_form,
                forms.long_operand,
                forms.wide_slot,
                forms.constructor,
                forms.nonnumeric_constant,
                forms.argument_position
            ),
        )
    }

    fn retain_source(&self, failure: PublicationFailure) -> PublicationFailure {
        use zetesis_solve::GroundedFormula as Grounded;
        let grounded = match &self.0 {
            zetesis_solve::PreparedExecution::Source(owner) => {
                return if let Some(bundle) = owner.bundle() {
                    bundle_failure(failure, bundle)
                } else if let Some(source) = owner.source() {
                    source_failure(failure, "<input>", source)
                } else {
                    failure
                };
            }
            zetesis_solve::PreparedExecution::Grounded(grounded) => grounded.as_ref(),
        };
        match grounded {
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
        self.comparison_forms(diagnostics)?;
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
    // This reads configuration only. Consult a refusal at the same boundary as
    // before: after an extended refusal that actually requires formulas.
    let formula_policy = validate_formula(options);
    let prepared = if options.oracle == Oracle::Countermodel {
        formula_policy?;
        if let Some(report) =
            crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
        {
            return Ok(report);
        }
        phases.measure(SolvePhase::AdmissionMaterialization, || {
            ParsedSource::new(source, admission)
                .map_err(zetesis_themelios::FormulaFailure::from)?
                .prepare_formula_with_cancellation(
                    expansion_limits(options),
                    formula_limits(options),
                    cancellation,
                )
                .map_err(SourceFailure::into_error)
        })
    } else {
        let source = phases
            .measure(SolvePhase::AdmissionMaterialization, || {
                ParsedSource::new(source, admission)
            })
            .map_err(|error| RunError::Expansion(error.into()))?;
        let admitted = phases.measure(SolvePhase::AdmissionMaterialization, || {
            if options.oracle == Oracle::Auto && formula_policy.is_ok() {
                source.admit_automatic_with_cancellation(
                    expansion_limits(options),
                    formula_limits(options),
                    cancellation,
                )
            } else {
                source
                    .admit_extended_with_cancellation(expansion_limits(options), cancellation)
                    .map(AutomaticAdmission::Extended)
                    .map_err(AutomaticAdmissionFailure::Extended)
            }
        });
        match admitted {
            Ok(AutomaticAdmission::Extended(admitted)) => {
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
            Ok(AutomaticAdmission::Formula(prepared)) => Ok(prepared),
            Err(error) => {
                if let Some(reason) = error.interruption() {
                    return crate::publication::interrupted(reason, renderer, diagnostics, phases);
                }
                match error {
                    AutomaticAdmissionFailure::Extended(error) => {
                        if options.oracle == Oracle::Auto && error.error().needs_formula_admission()
                        {
                            formula_policy?;
                        }
                        return Err(RunError::Expansion(error.into_error()).into());
                    }
                    AutomaticAdmissionFailure::Formula(error) => Err(error.into_error()),
                }
            }
        }
    };
    solve_source_formula(
        prepared,
        options,
        renderer,
        diagnostics,
        cancellation,
        phases,
    )
}

fn solve_source_formula(
    prepared: Result<zetesis_themelios::PreparedFormula, zetesis_themelios::FormulaFailure>,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    let observer = phases.grounding_observer();
    let admitted: Result<FormulaInput, zetesis_themelios::FormulaFailure> =
        prepared.and_then(|prepared| {
            phases.measure(SolvePhase::AdmissionMaterialization, || {
                let prepared = prepared
                    .with_grounding_options(grounding_options(options))
                    .with_domain_analysis(Some(zetesis_themelios::DomainLimits::default()));
                let forms = prepared.comparison_forms();
                let observer = observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver);
                zetesis_solve::prepare_formula_execution(
                    prepared,
                    &crate::SolveConfig::from(options),
                    observer,
                )
                .map(|execution| FormulaInput(execution, forms))
            })
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
    let formula_policy = validate_formula(options);
    let prepared = if options.oracle == Oracle::Countermodel {
        formula_policy?;
        if let Some(report) =
            crate::publication::check_cancellation(renderer, diagnostics, cancellation, phases)?
        {
            return Ok(report);
        }
        phases.measure(SolvePhase::AdmissionMaterialization, || {
            zetesis_themelios::prepare_bundle_formula_with_cancellation(
                bundle,
                bundle_options(options),
                expansion_limits(options),
                formula_limits(options),
                cancellation,
            )
        })
    } else {
        let admitted = phases.measure(SolvePhase::AdmissionMaterialization, || {
            if options.oracle == Oracle::Auto && formula_policy.is_ok() {
                admit_bundle_automatic_with_cancellation(
                    bundle,
                    bundle_options(options),
                    expansion_limits(options),
                    formula_limits(options),
                    cancellation,
                )
            } else {
                admit_bundle_extended_with_cancellation(
                    bundle,
                    bundle_options(options),
                    expansion_limits(options),
                    cancellation,
                )
                .map(AutomaticAdmission::Extended)
                .map_err(AutomaticAdmissionFailure::Extended)
            }
        });
        match admitted {
            Ok(AutomaticAdmission::Extended(admitted)) => {
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
            Ok(AutomaticAdmission::Formula(prepared)) => Ok(prepared),
            Err(error) => {
                if let Some(reason) = error.interruption() {
                    return crate::publication::interrupted(reason, renderer, diagnostics, phases);
                }
                match error {
                    AutomaticAdmissionFailure::Extended(error) => {
                        if options.oracle == Oracle::Auto
                            && matches!(error.error(), BundleAdmissionError::Expansion(expansion) if expansion.needs_formula_admission())
                        {
                            formula_policy?;
                        }
                        return Err(RunError::BundleAdmission(error).into());
                    }
                    AutomaticAdmissionFailure::Formula(error) => Err(error),
                }
            }
        }
    };
    solve_bundle_formula(
        prepared,
        options,
        renderer,
        diagnostics,
        cancellation,
        phases,
    )
}

fn solve_bundle_formula(
    prepared: Result<
        zetesis_themelios::PreparedFormulaBundle,
        zetesis_themelios::FormulaBundleFailure,
    >,
    options: &Options,
    renderer: &mut impl crate::AnswerRenderer,
    diagnostics: &mut Diagnostics<impl Write>,
    cancellation: &Cancellation,
    phases: &Recorder,
) -> Result<Progress, PublicationFailure> {
    let observer = phases.grounding_observer();
    let admitted: Result<FormulaInput, zetesis_themelios::FormulaBundleFailure> = prepared
        .and_then(|prepared| {
            phases.measure(SolvePhase::AdmissionMaterialization, || {
                let prepared = prepared
                    .with_grounding_options(grounding_options(options))
                    .with_domain_analysis(Some(zetesis_themelios::DomainLimits::default()));
                let forms = prepared.comparison_forms();
                let observer = observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver);
                zetesis_solve::prepare_bundle_execution(
                    prepared,
                    &crate::SolveConfig::from(options),
                    observer,
                )
                .map(|execution| FormulaInput(execution, forms))
            })
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
