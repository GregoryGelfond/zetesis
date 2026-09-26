//! Owned preparation receipts preserve budgets across deferred materialization.

use std::fmt;

use themelios_base::source::Source;
use themelios_base::span::Location;
use themelios_program::program::Program;

use super::{AdmittedFormula, AdmittedFormulaBundle, Compiled, FormulaBundleFailure};
use crate::{FormulaFailure, FormulaLimits, GroundingObserver, SourceBundle, SourceMetadata};
use crate::{expansion::Budget, formula_ground, formula_ir, grounding_observer};

pub(crate) struct Preparation {
    pub(crate) catalog: crate::formula_support::SupportCatalog,
    pub(crate) accounting: crate::formula_support::Accounting,
    pub(crate) program: formula_ir::Prepared,
    pub(crate) budget: Budget,
    pub(crate) limits: FormulaLimits,
    pub(crate) options: crate::grounding_options::Execution,
    pub(crate) location: Location,
}

impl Preparation {
    pub(super) fn new(
        program: formula_ir::Prepared,
        catalog: crate::formula_support::SupportCatalog,
        accounting: crate::formula_support::Accounting,
        budget: Budget,
        limits: &FormulaLimits,
        location: Location,
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
    ) -> Result<(Compiled, crate::formula_hybrid::Constraints), FormulaFailure> {
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

/// A checked source preparation that has not completed possible support or
/// materialized a formula theory. Consuming it resumes the original budgets;
/// callers cannot replace those budgets at the materialization boundary.
pub struct PreparedFormula {
    preparation: Preparation,
    source: Source,
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
    pub(super) fn new(preparation: Preparation, source: Source, metadata: SourceMetadata) -> Self {
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

    /// Bounded, pool-free analysis input retaining parsed origins. Its semantic
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

    /// Original bytes and source identity.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
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
    /// Analysis uses bounded infallible collections and has no cancellation or
    /// deadline polling; this does not strengthen eager materialization's
    /// existing control/allocation contract. Observers receive its typed outcome.
    #[must_use]
    pub const fn with_domain_analysis(mut self, limits: Option<crate::DomainLimits>) -> Self {
        self.preparation.options.domains = limits;
        self
    }

    /// Materialize producers and ineligible constraints while retaining ordinary
    /// atom/scalar integrity constraints for repeated bounded satisfaction checks.
    /// Complete possible support and original arithmetic admission still run.
    /// This explicit schedule currently requires indexed joins and no objectives.
    /// No solver runs; core answer sets still require the retained constraints.
    ///
    /// # Errors
    /// Returns a located capability, arithmetic, allocation or resource refusal.
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
        let (compiled, constraints) = self.preparation.ground_hybrid(observer)?;
        Ok(crate::HybridFormula::new(
            compiled,
            constraints,
            crate::formula_hybrid::SourceOwner::Single(self.source),
            self.metadata,
        ))
    }

    /// Materialize the complete formula theory using the retained preparation.
    /// This computes no answer sets and invokes no solver.
    ///
    /// # Errors
    /// Returns a located grounding, arithmetic or resource refusal. A successful
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
            match crate::formula_terminal::materialize(self.preparation, observer)? {
                Materialized {
                    compiled,
                    terminal: None,
                } => crate::FormulaMaterialization::Complete(AdmittedFormula {
                    compiled,
                    source: self.source,
                    metadata: self.metadata,
                }),
                Materialized {
                    compiled,
                    terminal: Some(extension),
                } => crate::FormulaMaterialization::Terminal(crate::TerminalFormula::new(
                    compiled,
                    extension,
                    crate::formula_hybrid::SourceOwner::Single(self.source),
                    self.metadata,
                )),
            },
        )
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
        let compiled = self.preparation.ground(observer, Some(request))?;
        Ok(AdmittedFormula {
            compiled,
            source: self.source,
            metadata: self.metadata,
        })
    }

    /// Like [`Self::ground`], observing only the actual eager-grounding interval.
    /// No clock is read by this API; the caller retains its observer on failure.
    ///
    /// # Errors
    /// Returns the same located failures as [`Self::ground`].
    pub fn ground_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormula, FormulaFailure> {
        let compiled = self.preparation.ground(observer, None)?;
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
    bundle: SourceBundle,
    metadata: SourceMetadata,
}

impl fmt::Debug for PreparedFormulaBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedFormulaBundle")
            .field("bundle", &self.bundle)
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
                terminal: None,
            }) => Ok(crate::FormulaMaterialization::Complete(
                AdmittedFormulaBundle {
                    compiled,
                    bundle: self.bundle,
                    metadata: self.metadata,
                },
            )),
            Ok(Materialized {
                compiled,
                terminal: Some(extension),
            }) => Ok(crate::FormulaMaterialization::Terminal(
                crate::TerminalFormula::new(
                    compiled,
                    extension,
                    crate::formula_hybrid::SourceOwner::Bundle(self.bundle),
                    self.metadata,
                ),
            )),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.bundle,
                error: Box::new(error),
            }),
        }
    }

    pub(super) fn new(
        preparation: Preparation,
        bundle: SourceBundle,
        metadata: SourceMetadata,
    ) -> Self {
        Self {
            preparation,
            bundle,
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

    /// Bounded, pool-free combined analysis input retaining parsed origins;
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

    /// Original source bytes, identities, paths and include occurrences.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
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
                crate::formula_hybrid::SourceOwner::Bundle(self.bundle),
                self.metadata,
            )),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.bundle,
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
                bundle: self.bundle,
                metadata: self.metadata,
            }),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.bundle,
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
                bundle: self.bundle,
                metadata: self.metadata,
            }),
            Err(error) => Err(FormulaBundleFailure {
                bundle: self.bundle,
                error: Box::new(error),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
