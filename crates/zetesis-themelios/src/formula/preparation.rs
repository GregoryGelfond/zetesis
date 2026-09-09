//! Owned preparation receipts preserve budgets across deferred materialization.

use std::fmt;

use themelios_base::source::Source;
use themelios_base::span::Location;
use themelios_program::program::Program;

use super::{AdmittedFormula, AdmittedFormulaBundle, Compiled, FormulaBundleFailure};
use crate::{FormulaFailure, FormulaLimits, GroundingObserver, SourceBundle, SourceMetadata};
use crate::{expansion::Budget, formula_ground, formula_ir, grounding_observer};

pub(super) struct Preparation {
    program: formula_ir::Prepared,
    budget: Budget,
    limits: FormulaLimits,
    location: Location,
}

impl Preparation {
    pub(super) fn new(
        program: formula_ir::Prepared,
        budget: Budget,
        limits: &FormulaLimits,
        location: Location,
    ) -> Self {
        Self {
            program,
            budget,
            // The deferred receipt owns one immutable snapshot; its later
            // compilation and grounding helpers borrow this configuration.
            limits: *limits,
            location,
        }
    }

    fn ground(
        mut self,
        observer: Option<&dyn GroundingObserver>,
        count_plan: Option<crate::formula_count_plan::Request<'_>>,
    ) -> Result<Compiled, FormulaFailure> {
        grounding_observer::observe(observer, || {
            formula_ground::ground(
                self.program,
                &self.limits,
                &mut self.budget,
                self.location,
                observer,
                count_plan,
            )
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

    /// Materialize the complete formula theory using the retained preparation.
    /// This computes no answer sets and invokes no solver.
    ///
    /// # Errors
    /// Returns a located grounding, arithmetic or resource refusal. A successful
    /// preparation does not guarantee successful materialization.
    pub fn ground(self) -> Result<AdmittedFormula, FormulaFailure> {
        self.ground_with_observer(None)
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
        control: &zetesis_cpu::Control,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormula, FormulaFailure> {
        let request = crate::formula_count_plan::Request { limits, control };
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

    /// Materialize the complete formula theory using the retained preparation.
    ///
    /// # Errors
    /// Retains the original source bundle alongside every grounding refusal.
    pub fn ground(self) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        self.ground_with_observer(None)
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
        control: &zetesis_cpu::Control,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        let request = crate::formula_count_plan::Request { limits, control };
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
