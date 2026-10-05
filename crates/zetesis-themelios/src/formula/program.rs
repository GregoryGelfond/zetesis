//! Canonical input controls, independent of source parsing and solve sessions.

use std::sync::Arc;

use themelios_program::program::{Program, Statement};
use zetesis_cpu::Cancellation;

use super::{FormulaFailure, FormulaLimits, PreparedFormula};
use crate::{ExpansionLimits, ProgramAdmissionFailure, ProgramAdmissionOptions, ProgramSite};

/// Which declarations participate in canonical program preparation and materialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FormulaPurpose {
    /// Preserve objectives and explicit projection for ordinary solving.
    #[default]
    Ordinary,
    /// Prepare full answer-set enumeration without objective scoring or projection.
    /// Objective and `#project` declarations are not normalized or instantiated;
    /// `#show` declarations retain their ordinary display meaning.
    AnswerSets,
}

impl FormulaPurpose {
    pub(crate) fn includes(self, statement: &Statement) -> bool {
        self == Self::Ordinary
            || !matches!(
                statement,
                Statement::Optimize(_) | Statement::WeakConstraint(_) | Statement::Project(_)
            )
    }
}

/// Limits, declaration purpose and optional control for one canonical preparation.
/// Defaults preserve [`super::prepare_program_formula`]'s ordinary behavior.
/// Cancellation is shared through preparation and subsequent materialization;
/// it never refreshes the cumulative resource allowances.
#[derive(Clone, Debug, Default)]
pub struct ProgramFormulaOptions {
    /// Bounds on borrowed input structure before it is copied or rewritten.
    pub admission: ProgramAdmissionOptions,
    /// Cumulative scalar, template, provenance and expansion allowances.
    pub expansion: ExpansionLimits,
    /// Formula preparation, support, theory and analysis allowances.
    pub formula: FormulaLimits,
    /// Which declarations the prepared computation serves.
    pub purpose: FormulaPurpose,
    /// Optional shared cancellation and absolute deadline for the whole operation.
    pub cancellation: Option<Cancellation>,
}

/// Inspect a borrowed canonical formula program without normalization or grounding.
///
/// The walk bounds logical structure, provenance and text before any input copy.
/// It allocates traversal scratch proportional to nesting depth and borrows the
/// original statement on refusal. It does not expand intervals or pools, bind
/// variables, evaluate arithmetic, complete support, or establish solvability.
/// Later preparation and grounding retain their own capability and resource checks.
///
/// # Errors
/// Returns a structural ceiling or an excluded outer language construct, naming
/// its original statement or part. No source coordinate is fabricated.
pub fn validate_program_formula(
    program: &Program,
    options: ProgramAdmissionOptions,
) -> Result<(), ProgramAdmissionFailure<'_>> {
    crate::formula_program_check::check(program, options)
}

/// Prepare canonical input for the selected purpose under shared runtime control.
///
/// The original `Arc<Program>` is retained unchanged. Original statement indices
/// are assigned before declaration filtering, so every emitted site resolves in
/// that same owner. Structural input limits still inspect every declaration,
/// including declarations excluded by the purpose. Preparation includes finite
/// fact expansion; callers requiring a validation-only lowering boundary use
/// [`validate_program_formula`] first.
///
/// Control is polled at existing charged expansion and formula-work boundaries,
/// around structural inspection and bounded upstream analysis, and during
/// observation compilation. Upstream analysis has no internal cancellation hook;
/// its already bounded call is one cooperative interval. Control is checked again
/// before a successful preparation or materialization is returned. This is not a
/// hard real-time deadline or an allocator/RSS bound.
///
/// # Errors
/// Retains the original program with every structural, capability, arithmetic,
/// resource or cancellation refusal. No partial preparation is returned.
pub fn prepare_program_formula_with(
    program: Arc<Program>,
    options: ProgramFormulaOptions,
) -> Result<PreparedFormula, FormulaFailure> {
    let owner = crate::formula_owner::Owner::logical(program);
    let prepare = || {
        let budget = crate::expansion::Budget::new(
            options.expansion,
            options.admission.core_limits.max_templates,
        )
        .with_cancellation(options.cancellation);
        budget.poll(ProgramSite::program())?;
        validate_program_formula(owner.program(), options.admission)
            .map_err(|error| super::logical_failure(owner.program(), error))?;
        budget.poll(ProgramSite::program())?;
        if options.purpose == FormulaPurpose::Ordinary {
            crate::formula_program_check::check_objectives(owner.program(), &options.formula)?;
        }
        crate::metadata::check_program_count(owner.program(), &budget, options.purpose)?;
        let mut metadata = crate::metadata::Builder::new(options.formula.metadata_storage);
        crate::metadata::collect_program_for(
            owner.program(),
            &mut metadata,
            options.purpose,
            &budget,
        )?;
        let mut compilation = crate::formula_ir::CompilationOptions::from(options.admission);
        compilation.purpose = options.purpose;
        let result = super::prepare(
            owner.program(),
            compilation,
            budget,
            &options.formula,
            ProgramSite::program(),
            metadata,
        )?;
        result.0.budget.poll(ProgramSite::program())?;
        Ok(result)
    };
    match prepare() {
        Ok((preparation, metadata)) => Ok(PreparedFormula::new(preparation, owner, metadata)),
        Err(error) => Err(owner.retain_failure(error)),
    }
}
