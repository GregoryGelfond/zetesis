//! Owned preparation receipts preserve budgets across deferred materialization.

use std::fmt;

use themelios_base::source::Source;
use themelios_program::program::Program;

use super::{AdmittedFormula, AdmittedFormulaBundle, Compiled, FormulaBundleFailure};
use crate::ProgramSite;
use crate::formula_owner::Owner;
use crate::{FormulaFailure, FormulaLimits, GroundingObserver, SourceBundle, SourceMetadata};
use crate::{expansion::Budget, formula_ground, formula_ir, grounding_observer};

pub(crate) struct Preparation {
    pub(crate) catalog: crate::formula_support::SupportCatalog,
    pub(crate) accounting: crate::formula_support::Accounting,
    pub(crate) program: formula_ir::Prepared,
    pub(crate) budget: Budget,
    pub(crate) limits: FormulaLimits,
    pub(crate) options: crate::grounding_options::Execution,
    pub(crate) location: ProgramSite,
}

impl Preparation {
    pub(super) fn new(
        program: formula_ir::Prepared,
        catalog: crate::formula_support::SupportCatalog,
        accounting: crate::formula_support::Accounting,
        budget: Budget,
        limits: &FormulaLimits,
        location: ProgramSite,
    ) -> Self {
        Self {
            program,
            catalog,
            accounting,
            budget,
            // The deferred receipt owns one immutable snapshot; its later
            // compilation and grounding helpers borrow this configuration.
            limits: *limits,
            options: crate::grounding_options::Execution::default(),
            location,
        }
    }

    fn ground_hybrid(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<(Compiled, Option<crate::formula_hybrid::Constraints>), FormulaFailure> {
        grounding_observer::observe(observer, || formula_ground::ground_hybrid(self, observer))
    }

    fn ground(
        self,
        observer: Option<&dyn GroundingObserver>,
        count_plan: Option<crate::formula_count_plan::Request<'_>>,
    ) -> Result<Compiled, FormulaFailure> {
        grounding_observer::observe(observer, || {
            formula_ground::ground(self, observer, count_plan)
        })
    }
}

/// A checked canonical input that has not completed possible support or
/// materialized a formula theory. Consuming it resumes the original budgets;
/// callers cannot replace those budgets at the materialization boundary.
pub struct PreparedFormula {
    preparation: Preparation,
    source: Owner,
    metadata: SourceMetadata,
}

impl fmt::Debug for PreparedFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedFormula")
            .field("source", &self.source)
            .field("analyzed_program", self.analyzed_program())
            .finish_non_exhaustive()
    }
}

impl PreparedFormula {
    pub(super) fn new(preparation: Preparation, source: Owner, metadata: SourceMetadata) -> Self {
        Self {
            preparation,
            source,
            metadata,
        }
    }

    /// Structural facts about exactly [`Self::analyzed_program`]. Unknown verdicts
    /// establish neither nonmembership in a class nor a grounder requirement.
    /// Consult [`Self::analysis_basis`] before interpreting these as source facts.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.preparation.program.analysis
    }

    /// Bounded, pool-free analysis input retaining available provenance. Its semantic
    /// status is identified by [`Self::analysis_basis`].
    /// Metadata and subsequently generated support/coherence formulas are separate.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.preparation.program.analyzed
    }

    /// Meaning of the retained analysis input. A dependency projection is not
    /// a semantic normalization or a source safety/eligibility certificate.
    #[must_use]
    pub fn analysis_basis(&self) -> super::AnalysisBasis {
        self.preparation.program.analysis_basis
    }

    /// Original canonical program before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.source.program()
    }

    /// Original bytes and source identity, absent for a logical program input.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        self.source.source()
    }

    /// Compiled source declarations and display policy.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }

    /// Choose the positive-join execution policy without changing the retained
    /// source, preparation budgets or source identity. This stores a small value;
    /// it performs no grounding, allocation, timing or solver operation. All
    /// materialization doors, including count planning, use this policy.
    #[must_use]
    pub const fn with_grounding_options(mut self, options: crate::GroundingOptions) -> Self {
        self.preparation.options.joins = options.joins;
        self
    }

    /// Attempt finite-domain guards during final instantiation of an exact
    /// normalized positive flat program. Disabled by default. Inapplicable,
    /// Unknown or Stopped analysis leaves the complete join unchanged.
    ///
    /// The analysis borrows this exact normalized source and keeps its logical
    /// population limits separate from support/guard capacity. Its work and all
    /// bridge/guard work consume the original cumulative formula budget.
    /// Analysis uses bounded infallible collections and has no internal control
    /// hook. A configured preparation token is polled around that bounded call;
    /// it does not provide a hard real-time deadline or allocator/RSS bound.
    /// Observers receive the typed analysis outcome.
    #[must_use]
    pub const fn with_domain_analysis(mut self, limits: Option<crate::DomainLimits>) -> Self {
        self.preparation.options.domains = limits;
        self
    }

    /// Materialize producers and ineligible constraints while retaining ordinary
    /// atom/scalar integrity constraints for repeated bounded satisfaction checks.
    /// Complete possible support and original arithmetic admission still run.
    /// This explicit schedule requires indexed joins. Objectives are retained;
    /// solving must check the original constraints before scoring a core answer.
    /// No solver runs; core answer sets still require the retained constraints.
    ///
    /// # Errors
    /// Returns a typed capability, arithmetic, allocation or resource refusal,
    /// identifying its original statement when applicable.
    pub fn ground_hybrid(self) -> Result<crate::HybridFormula, FormulaFailure> {
        self.ground_hybrid_with_observer(None)
    }

    /// Hybrid materialization with the same source phase observer as eager
    /// grounding. Counts describe actual retained core nodes and admission work.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_hybrid`].
    pub fn ground_hybrid_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::HybridFormula, FormulaFailure> {
        let (compiled, constraints) = self
            .preparation
            .ground_hybrid(observer)
            .map_err(|error| self.source.retain_failure(error))?;
        Ok(crate::HybridFormula::new(
            compiled,
            constraints,
            self.source,
            self.metadata,
        ))
    }

    /// Materialize the complete formula theory using the retained preparation.
    /// This computes no answer sets and invokes no solver.
    ///
    /// # Errors
    /// Returns a typed grounding, arithmetic or resource refusal. A successful
    /// preparation does not guarantee successful materialization.
    pub fn ground(self) -> Result<AdmittedFormula, FormulaFailure> {
        self.ground_with_observer(None)
    }

    /// Materialize a certified base and defer complete terminal positive
    /// definition groups when their source/IR correspondence is established and
    /// at least one producer has a variable. A group shares a predicate name,
    /// arity and sign; its ground producers defer with its variable producers.
    /// Ground-only groups remain in the base, including rules with nonempty
    /// bodies. If no group is selected, materialize the original theory. This
    /// physical policy does not narrow the mathematical terminal classification.
    /// Both routes retain preparation charges and compute no answer sets.
    ///
    /// # Errors
    /// Returns source, allocation or resource refusals; an applicable terminal
    /// plan is never silently replaced after its materialization has failed.
    pub fn ground_adaptive(
        self,
    ) -> Result<crate::FormulaMaterialization<AdmittedFormula>, FormulaFailure> {
        self.ground_adaptive_with_observer(None)
    }

    /// Adaptive materialization with caller-owned grounding observations.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_adaptive`].
    pub fn ground_adaptive_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::FormulaMaterialization<AdmittedFormula>, FormulaFailure> {
        use crate::formula_terminal::Materialized;
        Ok(
            match crate::formula_terminal::materialize(self.preparation, observer)
                .map_err(|error| self.source.retain_failure(error))?
            {
                Materialized {
                    compiled,
                    core: _,
                    terminal: None,
                } => crate::FormulaMaterialization::Complete(AdmittedFormula {
                    compiled,
                    source: self.source,
                    metadata: self.metadata,
                }),
                Materialized {
                    compiled,
                    core,
                    terminal: Some(extension),
                } => crate::FormulaMaterialization::Terminal(crate::TerminalFormula::new(
                    compiled,
                    core,
                    extension,
                    self.source,
                    self.metadata,
                )),
            },
        )
    }

    /// Lazy materialization: the terminal partition defers certified terminal
    /// definitions, as [`Self::ground_adaptive`] does, but the base (or, with
    /// nothing deferred, the program) is grounded under the hybrid schedule:
    /// its producer core is instantiated and its eligible integrity constraints
    /// are streamed. A terminal owner's base is then [`crate::BaseKind::Hybrid`],
    /// even with no eligible constraint. Objective declarations disable terminal
    /// deferral and produce [`crate::FormulaMaterialization::Complete`] with a
    /// [`crate::HybridFormula`], retaining every producer. Score its answers only
    /// after complete streamed constraint acceptance.
    ///
    /// # Errors
    /// Returns the failures of [`Self::ground_adaptive`] and
    /// [`Self::ground_hybrid`].
    pub fn ground_lazy(
        self,
    ) -> Result<crate::FormulaMaterialization<crate::HybridFormula>, FormulaFailure> {
        self.ground_lazy_with_observer(None)
    }

    /// Lazy materialization with caller-owned grounding observations.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_lazy`].
    pub fn ground_lazy_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::FormulaMaterialization<crate::HybridFormula>, FormulaFailure> {
        let materialized = crate::formula_terminal::materialize_lazy(self.preparation, observer)
            .map_err(|error| self.source.retain_failure(error))?;
        Ok(lazy_outcome(materialized, self.source, self.metadata))
    }

    /// Materialize the same original theory while independently attempting
    /// bounded source-count partition consequences. Planning is opt-in; its
    /// status is retained by [`AdmittedFormula::count_plan`]. Its control applies
    /// only to planning, not to existing source materialization. No solver runs.
    /// Capture reuses complete source joins and canonical guard evaluations;
    /// [`crate::CountPlan`] describes the bounded applicability and cost model.
    /// The outer observer boundary includes optional planning; source phase work
    /// counters remain separate from [`crate::CountPlanStatistics`].
    ///
    /// # Errors
    /// Returns existing source-grounding failures. Optional planning failures
    /// retain the successfully admitted original and an incomplete plan status.
    pub fn ground_with_count_plan(
        self,
        limits: crate::CountPlanLimits,
        cancellation: &zetesis_cpu::Cancellation,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormula, FormulaFailure> {
        let request = crate::formula_count_plan::Request {
            limits,
            cancellation,
        };
        let compiled = self
            .preparation
            .ground(observer, Some(request))
            .map_err(|error| self.source.retain_failure(error))?;
        Ok(AdmittedFormula {
            compiled,
            source: self.source,
            metadata: self.metadata,
        })
    }

    /// Like [`Self::ground`], observing only the actual eager-grounding interval.
    /// The frontend reads no clock; configured control polls shared flags.
    /// The caller retains its observer on failure.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground`].
    pub fn ground_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormula, FormulaFailure> {
        let compiled = self
            .preparation
            .ground(observer, None)
            .map_err(|error| self.source.retain_failure(error))?;
        Ok(AdmittedFormula {
            compiled,
            source: self.source,
            metadata: self.metadata,
        })
    }
}

/// A checked formula preparation retaining the complete original source bundle.
/// Source analysis is available before eager materialization; grounding resumes
/// the original budgets and retains the bundle on any located refusal.
pub struct PreparedFormulaBundle {
    preparation: Preparation,
    source: Owner,
    metadata: SourceMetadata,
}

impl fmt::Debug for PreparedFormulaBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedFormulaBundle")
            .field("bundle", &self.bundle())
            .field("analyzed_program", self.analyzed_program())
            .finish_non_exhaustive()
    }
}

impl PreparedFormulaBundle {
    /// Bundle counterpart of [`PreparedFormula::ground_adaptive`].
    ///
    /// # Errors
    /// Retains the original bundle on source, allocation or resource refusal.
    pub fn ground_adaptive(
        self,
    ) -> Result<crate::FormulaMaterialization<AdmittedFormulaBundle>, FormulaBundleFailure> {
        self.ground_adaptive_with_observer(None)
    }

    /// Adaptive bundle materialization with grounding observations.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_adaptive`].
    pub fn ground_adaptive_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::FormulaMaterialization<AdmittedFormulaBundle>, FormulaBundleFailure> {
        use crate::formula_terminal::Materialized;
        match crate::formula_terminal::materialize(self.preparation, observer) {
            Ok(Materialized {
                compiled,
                core: _,
                terminal: None,
            }) => Ok(crate::FormulaMaterialization::Complete(
                AdmittedFormulaBundle {
                    compiled,
                    source: self.source,
                    metadata: self.metadata,
                },
            )),
            Ok(Materialized {
                compiled,
                core,
                terminal: Some(extension),
            }) => Ok(crate::FormulaMaterialization::Terminal(
                crate::TerminalFormula::new(compiled, core, extension, self.source, self.metadata),
            )),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.source.into_bundle(),
                error: Box::new(error),
            }),
        }
    }

    /// Bundle counterpart of [`PreparedFormula::ground_lazy`].
    ///
    /// # Errors
    /// Retains the original bundle on every refusal.
    pub fn ground_lazy(
        self,
    ) -> Result<crate::FormulaMaterialization<crate::HybridFormula>, FormulaBundleFailure> {
        self.ground_lazy_with_observer(None)
    }

    /// Lazy bundle materialization with grounding observations.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_lazy`].
    pub fn ground_lazy_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::FormulaMaterialization<crate::HybridFormula>, FormulaBundleFailure> {
        match crate::formula_terminal::materialize_lazy(self.preparation, observer) {
            Ok(materialized) => Ok(lazy_outcome(materialized, self.source, self.metadata)),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.source.into_bundle(),
                error: Box::new(error),
            }),
        }
    }

    pub(super) fn new(preparation: Preparation, source: Owner, metadata: SourceMetadata) -> Self {
        Self {
            preparation,
            source,
            metadata,
        }
    }

    /// Structural facts about exactly [`Self::analyzed_program`], not a guarantee
    /// that grounding succeeds or that lazy formula execution is implemented.
    /// Consult [`Self::analysis_basis`] before interpreting these as source facts.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.preparation.program.analysis
    }

    /// Bounded, pool-free combined analysis input retaining available provenance;
    /// consult [`Self::analysis_basis`] before interpreting its verdicts.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.preparation.program.analyzed
    }

    /// Meaning of the retained analysis input. A dependency projection is not
    /// a semantic normalization or a source safety/eligibility certificate.
    #[must_use]
    pub fn analysis_basis(&self) -> super::AnalysisBasis {
        self.preparation.program.analysis_basis
    }

    /// Original canonical program before normalization or analysis projection.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.source.program()
    }

    /// Original source bytes, identities, paths and include occurrences.
    /// Bundle admission retains this catalog by construction.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        self.source.required_bundle()
    }

    /// Global source declarations and display policy.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }

    /// Choose the positive-join execution policy without changing the retained
    /// source, preparation budgets or source identity. This stores a small value;
    /// it performs no grounding, allocation, timing or solver operation. All
    /// materialization doors, including count planning, use this policy.
    #[must_use]
    pub const fn with_grounding_options(mut self, options: crate::GroundingOptions) -> Self {
        self.preparation.options.joins = options.joins;
        self
    }

    /// Configure the optional final-instantiation analysis with the same owner,
    /// fallback, work and allocation boundaries as
    /// [`PreparedFormula::with_domain_analysis`]. `None` disables the attempt.
    #[must_use]
    pub const fn with_domain_analysis(mut self, limits: Option<crate::DomainLimits>) -> Self {
        self.preparation.options.domains = limits;
        self
    }

    /// Materialize the complete formula theory using the retained preparation.
    ///
    /// # Errors
    /// Retains the original source bundle alongside every grounding refusal.
    pub fn ground(self) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        self.ground_with_observer(None)
    }

    /// Bundle counterpart of [`PreparedFormula::ground_hybrid`]. The original
    /// include catalog remains owned through successful admission or refusal.
    ///
    /// # Errors
    /// Returns a located source failure with the complete original bundle.
    pub fn ground_hybrid(self) -> Result<crate::HybridFormula, FormulaBundleFailure> {
        self.ground_hybrid_with_observer(None)
    }

    /// Hybrid bundle materialization with source phase observations.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::ground_hybrid`].
    pub fn ground_hybrid_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<crate::HybridFormula, FormulaBundleFailure> {
        match self.preparation.ground_hybrid(observer) {
            Ok((compiled, constraints)) => Ok(crate::HybridFormula::new(
                compiled,
                constraints,
                self.source,
                self.metadata,
            )),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.source.into_bundle(),
                error: Box::new(error),
            }),
        }
    }

    /// Bundle counterpart of [`PreparedFormula::ground_with_count_plan`], with
    /// the same independent planning status and immutable original identity.
    ///
    /// # Errors
    /// Retains original bundle ownership on source-grounding failure. Planning
    /// failure alone does not discard a successfully admitted bundle.
    pub fn ground_with_count_plan(
        self,
        limits: crate::CountPlanLimits,
        cancellation: &zetesis_cpu::Cancellation,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        let request = crate::formula_count_plan::Request {
            limits,
            cancellation,
        };
        match self.preparation.ground(observer, Some(request)) {
            Ok(compiled) => Ok(AdmittedFormulaBundle {
                compiled,
                source: self.source,
                metadata: self.metadata,
            }),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.source.into_bundle(),
                error: Box::new(error),
            }),
        }
    }

    /// Like [`Self::ground`], observing only the actual eager-grounding interval.
    ///
    /// # Errors
    /// Retains the original bundle and returns the same failures as [`Self::ground`].
    pub fn ground_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        match self.preparation.ground(observer, None) {
            Ok(compiled) => Ok(AdmittedFormulaBundle {
                compiled,
                source: self.source,
                metadata: self.metadata,
            }),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.source.into_bundle(),
                error: Box::new(error),
            }),
        }
    }
}

#[cfg(test)]
mod tests;

/// The owner a lazy materialization admits: a hybrid owner when nothing is
/// deferred, else a terminal owner with a hybrid base.
fn lazy_outcome(
    materialized: crate::formula_terminal::Materialized,
    source: crate::formula_owner::Owner,
    metadata: crate::SourceMetadata,
) -> crate::FormulaMaterialization<crate::HybridFormula> {
    use crate::formula_terminal::{Core, Materialized};
    match materialized {
        Materialized {
            compiled,
            core,
            terminal: Some(extension),
        } => crate::FormulaMaterialization::Terminal(crate::TerminalFormula::new(
            compiled, core, extension, source, metadata,
        )),
        Materialized {
            compiled,
            core: Core::Hybrid(constraints),
            terminal: None,
        } => crate::FormulaMaterialization::Complete(crate::HybridFormula::new(
            compiled,
            constraints.map(|constraints| *constraints),
            source,
            metadata,
        )),
        Materialized {
            core: Core::Eager,
            terminal: None,
            ..
        } => unreachable!("lazy materialization grounds an undeferred program as hybrid"),
    }
}
