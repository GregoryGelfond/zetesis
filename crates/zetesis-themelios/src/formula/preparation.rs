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
        limits: FormulaLimits,
        location: Location,
    ) -> Self {
        Self {
            program,
            budget,
            limits,
            location,
        }
    }

    fn ground(
        mut self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<Compiled, FormulaFailure> {
        grounding_observer::observe(observer, || {
            formula_ground::ground(self.program, self.limits, &mut self.budget, self.location)
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
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.preparation.program.analysis
    }

    /// Bounded normalized, pool-free logical projection retaining parsed origins.
    /// Metadata and subsequently generated support/coherence formulas are separate.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.preparation.program.analyzed
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

    /// Like [`Self::ground`], observing only the actual eager-grounding interval.
    /// No clock is read by this API; the caller retains its observer on failure.
    ///
    /// # Errors
    /// Returns the same located failures as [`Self::ground`].
    pub fn ground_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormula, FormulaFailure> {
        let compiled = self.preparation.ground(observer)?;
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
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        &self.preparation.program.analysis
    }

    /// Bounded normalized, pool-free combined program retaining parsed origins.
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        &self.preparation.program.analyzed
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

    /// Like [`Self::ground`], observing only the actual eager-grounding interval.
    ///
    /// # Errors
    /// Retains the original bundle and returns the same failures as [`Self::ground`].
    pub fn ground_with_observer(
        self,
        observer: Option<&dyn GroundingObserver>,
    ) -> Result<AdmittedFormulaBundle, FormulaBundleFailure> {
        match self.preparation.ground(observer) {
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
