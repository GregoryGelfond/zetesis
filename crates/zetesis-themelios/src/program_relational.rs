//! Owned canonical input for the existing relational compiler.

use std::sync::Arc;

use themelios_program::program::Program;
use zetesis_cpu::Cancellation;

use crate::expansion::Budget;
use crate::formula_owner::Owner;
use crate::{
    ExpansionLimits, ExpansionUsage, FormulaFailure, FormulaPurpose, MetadataStorageLimits,
    ProgramAdmissionOptions, ProgramSite, SourceMetadata, extended, metadata,
    validate_program_formula,
};

/// Limits, declaration purpose and shared control for relational preparation.
///
/// Defaults select the existing extended relational language. This includes
/// scalar constants and finite fact expansion, but not general formula heads or
/// aggregates. [`FormulaPurpose::AnswerSets`] additionally excludes objectives
/// and `#project` before their normalization or metadata compilation.
#[derive(Clone, Debug, Default)]
pub struct ProgramRelationalOptions {
    /// Bounds on the entire borrowed input, including excluded declarations.
    pub admission: ProgramAdmissionOptions,
    /// Cumulative scalar normalization and fact expansion, including observation constants.
    pub expansion: ExpansionLimits,
    /// Bounds on compiled `#show` term queries.
    pub observation: crate::observation::AdmissionLimits,
    /// Bounds on shared declaration and observation vocabulary storage.
    pub metadata_storage: MetadataStorageLimits,
    /// Which declarations participate; `#show` always retains its display meaning.
    pub purpose: FormulaPurpose,
    /// Optional shared cancellation and absolute deadline for this preparation.
    pub cancellation: Option<Cancellation>,
}

/// A relational program, its display metadata and its exact original owner.
///
/// Finite facts have been expanded and the native program has passed admission;
/// variable joins and reduct membership have not been evaluated. The same program
/// can therefore enter native lazy or eager execution. Metadata affects display,
/// never membership or the identity of complete interpretations.
///
/// Template sites index [`Self::original_program`], including after excluded
/// declarations or one-to-many fact expansion. No source text is synthesized.
#[derive(Debug)]
pub struct PreparedRelational {
    program: zetesis_core::Program,
    owner: Owner,
    metadata: SourceMetadata,
    template_sites: Vec<Vec<ProgramSite>>,
    expansion: ExpansionUsage,
}

impl PreparedRelational {
    /// The admitted native program, ready for relational execution.
    #[must_use]
    pub fn program(&self) -> &zetesis_core::Program {
        &self.program
    }

    /// The caller's unchanged canonical program, shared through its original `Arc`.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.owner.program()
    }

    /// Declaration and display policy compiled against the original program.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }

    /// Original evidence in native template order. Generated coherence constraints
    /// retain both contributing statement identities and any actual coordinates.
    #[must_use]
    pub fn template_sites(&self) -> &[Vec<ProgramSite>] {
        &self.template_sites
    }

    /// Accepted cumulative charges for this preparation, without resetting at the
    /// observation or relational compilation boundary.
    #[must_use]
    pub const fn expansion_usage(&self) -> &ExpansionUsage {
        &self.expansion
    }
}

/// Prepare canonical input through the existing extended relational compiler.
///
/// The receipt retains the supplied `Arc` unchanged. Whole-input structural
/// inspection precedes normalization; it counts even declarations excluded by
/// [`FormulaPurpose::AnswerSets`]. Excluded objectives and `#project` contribute
/// no semantic checks, evaluation, scoring or projection metadata. Ordinary
/// preparation preserves their existing relational profile refusals. Constants,
/// `#defined` and signature/term `#show` use the shared frontend mechanisms.
///
/// This operation may expand finite fact pools and intervals. A validation-only
/// lowering boundary can call [`validate_program_formula`] first; successful
/// validation does not establish relational language support. No text is rendered
/// or reparsed, no answer search runs, and unsupported constructs never trigger
/// a switch to another grounding profile.
///
/// Scalar normalization for observations and relational templates shares one
/// cumulative expansion budget. Observation plans have their own named limits.
/// Control is polled at charged work boundaries and around bounded structural
/// inspection, upstream rewrites and native admission. Those operations have no
/// internal control callback and form cooperative intervals. This is neither a
/// hard real-time deadline nor an allocator/RSS bound; input construction and
/// ordinary AST carriers remain outside the expansion byte allowance.
///
/// # Errors
/// Returns the shared [`FormulaFailure`] family for structural, capability,
/// arithmetic, observation, resource and control refusals, retaining the exact
/// original program for subject lookup. No partial receipt is returned.
pub fn prepare_program_relational(
    program: Arc<Program>,
    options: ProgramRelationalOptions,
) -> Result<PreparedRelational, FormulaFailure> {
    let owner = Owner::logical(program);
    let prepare = || {
        let site = ProgramSite::program();
        let mut budget = Budget::new(
            options.expansion,
            options.admission.core_limits.max_templates,
        )
        .with_cancellation(options.cancellation);
        budget.poll(site)?;
        validate_program_formula(owner.program(), options.admission)
            .map_err(|error| crate::formula::logical_failure(owner.program(), error))?;
        budget.poll(site)?;
        metadata::check_program_count(owner.program(), &budget, options.purpose)?;
        let mut metadata = metadata::Builder::new(options.metadata_storage);
        metadata::collect_program_for(owner.program(), &mut metadata, options.purpose, &budget)?;
        let mut compilation = crate::formula_ir::CompilationOptions::from(options.admission);
        compilation.purpose = options.purpose;
        metadata.compile_observations(
            owner.program(),
            compilation,
            options.observation,
            &mut budget,
            site,
        )?;
        let (program, template_sites) = extended::compile_relational(
            owner.program(),
            options.admission.core_limits,
            &mut budget,
            site,
            options.purpose,
            extended::program_sites,
            |site| *site,
        )?;
        let metadata = metadata.finish(site)?;
        budget.poll(site)?;
        Ok((program, template_sites, metadata, budget.usage()))
    };
    match prepare() {
        Ok((program, template_sites, metadata, expansion)) => Ok(PreparedRelational {
            program,
            owner,
            metadata,
            template_sites,
            expansion,
        }),
        Err(error) => Err(owner.retain_failure(error)),
    }
}

#[cfg(test)]
mod tests;
