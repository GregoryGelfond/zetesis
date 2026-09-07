//! Language admission and exact oracle selection for strings and original bundles.

use crate::presentation::{Diagnostics, Label};
use std::io::Write;

use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, BundleAdmissionError, BundleAdmissionOptions, ExpansionLimits, SourceBundle,
    admit_bundle_extended, admit_extended,
};

use crate::failure::Progress;
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{Options, Oracle, RunError, SolveFailure};

pub(crate) fn source(
    source: String,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, SolveFailure> {
    crate::engine::validate_combination(&options.into())?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) = crate::countermodel::check_control(
            output,
            diagnostics,
            control,
            phases,
            options.json,
            None,
        )?
    {
        return Ok(report);
    }
    let admission = AdmissionOptions {
        max_source_bytes: options.max_source_bytes,
        ..Default::default()
    };
    let source = if options.oracle == Oracle::Countermodel {
        source
    } else {
        // A retry never duplicates an input already beyond its source ceiling.
        let retry = (options.oracle == Oracle::Auto && source.len() <= options.max_source_bytes)
            .then(|| source.clone());
        match phases.measure(SolvePhase::AdmissionMaterialization, || {
            admit_extended(source, admission, expansion_limits(options))
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::driver::solve_program(
                    admitted.program(),
                    admitted.metadata().output(),
                    options,
                    output,
                    diagnostics,
                    control,
                    phases,
                );
            }
            Err(error) if retry.is_some() && error.needs_formula_admission() => {
                retry.expect("retry guard established an owned source")
            }
            Err(error) => return Err(RunError::Expansion(error).into()),
        }
    };
    crate::engine::validate_countermodel(&options.into())?;
    if let Some(report) = crate::countermodel::check_control(
        output,
        diagnostics,
        control,
        phases,
        options.json,
        None,
    )? {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted = phases
        .measure(SolvePhase::AdmissionMaterialization, || {
            zetesis_themelios::admit_formula_with_grounding_observer(
                source,
                admission,
                expansion_limits(options),
                formula_limits(options),
                observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver),
            )
        })
        .map_err(RunError::FormulaAdmission)?;
    crate::countermodel::run_formula(
        crate::countermodel::Input {
            theory: admitted.theory(),
            atoms: admitted.atoms(),
            gate_atoms: 0,
            objectives: admitted.objectives(),
            observations: admitted.metadata().observations(),
        },
        admitted.metadata().output(),
        options,
        output,
        diagnostics,
        control,
        phases,
    )
}

pub(crate) fn bundle(
    bundle: SourceBundle,
    options: &Options,
    output: &mut impl Write,
    diagnostics: &mut Diagnostics<impl Write>,
    control: &Control,
    phases: &Recorder,
) -> Result<Progress, SolveFailure> {
    crate::engine::validate_combination(&options.into())?;
    if options.oracle == Oracle::Countermodel
        && let Some(report) = crate::countermodel::check_control(
            output,
            diagnostics,
            control,
            phases,
            options.json,
            None,
        )?
    {
        return Ok(report);
    }
    diagnostics.metadata(
        Label::Source,
        format_args!(
            "{} original files ({} bytes)",
            bundle.sources().len(),
            bundle.total_bytes()
        ),
    )?;
    let bundle = if options.oracle == Oracle::Countermodel {
        bundle
    } else {
        match phases.measure(SolvePhase::AdmissionMaterialization, || {
            admit_bundle_extended(
                bundle,
                BundleAdmissionOptions::default(),
                expansion_limits(options),
            )
        }) {
            Ok(admitted) => {
                diagnostics.metadata(Label::Oracle, format_args!("reduct closure"))?;
                return crate::driver::solve_program(
                    admitted.program(),
                    admitted.metadata().output(),
                    options,
                    output,
                    diagnostics,
                    control,
                    phases,
                );
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
    crate::engine::validate_countermodel(&options.into())?;
    if let Some(report) = crate::countermodel::check_control(
        output,
        diagnostics,
        control,
        phases,
        options.json,
        None,
    )? {
        return Ok(report);
    }
    let observer = phases.grounding_observer();
    let admitted = phases
        .measure(SolvePhase::AdmissionMaterialization, || {
            zetesis_themelios::admit_bundle_formula_with_grounding_observer(
                bundle,
                BundleAdmissionOptions::default(),
                expansion_limits(options),
                formula_limits(options),
                observer
                    .as_ref()
                    .map(|observer| observer as &dyn zetesis_themelios::GroundingObserver),
            )
        })
        .map_err(RunError::FormulaBundleAdmission)?;
    crate::countermodel::run_formula(
        crate::countermodel::Input {
            theory: admitted.theory(),
            atoms: admitted.atoms(),
            gate_atoms: 0,
            objectives: admitted.objectives(),
            observations: admitted.metadata().observations(),
        },
        admitted.metadata().output(),
        options,
        output,
        diagnostics,
        control,
        phases,
    )
}

pub(crate) fn expansion_limits(options: &Options) -> ExpansionLimits {
    ExpansionLimits {
        max_term_work: options.max_expansion_work,
        max_templates: options.max_expanded_templates,
        max_values: options.max_expansion_values,
        ..Default::default()
    }
}

pub(crate) fn formula_limits(options: &Options) -> zetesis_themelios::FormulaLimits {
    zetesis_themelios::FormulaLimits {
        max_substitutions: u64::try_from(options.max_substitutions).unwrap_or(u64::MAX),
        max_work: u64::try_from(options.max_expansion_work).unwrap_or(u64::MAX),
        theory: zetesis_ferraris::AdmissionLimits {
            max_atoms: options
                .max_atoms
                .min(zetesis_ferraris::AdmissionLimits::default().max_atoms),
            max_roots: options
                .max_ground_rules
                .min(zetesis_ferraris::AdmissionLimits::default().max_roots),
            ..Default::default()
        },
        ..Default::default()
    }
}
