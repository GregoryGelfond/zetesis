//! Exact finite source evaluation for a complete signed acyclic normal program.
//!
//! The result is a source subject, not a replacement formula theory. Complete
//! support and original-family arithmetic admission precede the one dependency
//! ordered activity scan. The ordinary ground door remains the authority for
//! arbitrary original and reduct queries.

use crate::formula::Preparation;
use crate::formula_owner::Owner;
use crate::formula_source_activity::{self, Exact};
use crate::formula_support::{AcyclicPlan, Computation, GroundingWork, Publication};
use crate::grounding_observer::Profile;
use crate::{FormulaFailure, GroundingObserver, SourceBundle, SourceMetadata};
use std::{fmt, sync::Arc};
use themelios_base::source::Source;
use themelios_program::program::Program;
use zetesis_core::AtomCatalog;

/// The complete source class either admits one exact subject or returns the
/// original preparation, including all accepted classification charges.
#[derive(Debug)]
pub enum SourceAdmission<P> {
    /// A complete signed acyclic normal source, evaluated exactly once.
    Admitted(AcyclicSource),
    /// The original preparation still owns its ordinary grounding/query door.
    NotApplicable(P),
}

/// All accepted source preparation and evaluation charges. These are admission
/// totals, not work attributed to later answer publication or a search worker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceEvaluationStatistics {
    /// Cumulative formula work, including support, arithmetic and exact activity.
    pub work: u64,
    /// Cumulative complete substitutions accepted by the original formula budget.
    pub substitutions: u64,
}

struct Admitted {
    source: Owner,
    metadata: SourceMetadata,
    evaluation: Evaluated,
}

/// Immutable exact interpretation of one admitted acyclic source program.
/// Clones share its original identity and canonical typed atom payload. No
/// Boolean theory is synthesized and no formula-query contract is changed.
#[derive(Clone)]
pub struct AcyclicSource(Arc<Admitted>);

impl fmt::Debug for AcyclicSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcyclicSource")
            .field("source", &self.0.source)
            .field("atoms", &self.atom_catalog().atoms().len())
            .field("satisfiable", &self.selected_positions().is_some())
            .finish_non_exhaustive()
    }
}

impl AcyclicSource {
    pub(crate) fn new(evaluation: Evaluated, source: Owner, metadata: SourceMetadata) -> Self {
        Self(Arc::new(Admitted {
            source,
            metadata,
            evaluation,
        }))
    }

    /// Exact original admitted-owner identity; clones share it.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Canonical admitted producer atoms. Dense positions belong only to this
    /// catalog and need not equal positions in a separately grounded theory.
    #[must_use]
    pub fn atom_catalog(&self) -> &AtomCatalog {
        &self.0.evaluation.atoms
    }

    /// The unique consequence interpretation, or no answer if a constraint or
    /// opposite classical signs reject it. Positions are increasing and scoped
    /// to [`Self::atom_catalog`]; this borrowed receipt cannot be constructed by
    /// callers. All source evaluation work was charged during admission.
    #[must_use]
    pub fn selected_positions(&self) -> Option<&[usize]> {
        self.0.evaluation.selected.as_deref()
    }

    /// Accepted preparation and exact evaluation work, including publication.
    #[must_use]
    pub fn statistics(&self) -> SourceEvaluationStatistics {
        self.0.evaluation.statistics
    }

    /// Original canonical program before source normalization.
    #[must_use]
    pub fn original_program(&self) -> &Program {
        self.0.source.program()
    }

    /// Original single source, absent for logical and bundle inputs.
    #[must_use]
    pub fn source(&self) -> Option<&Source> {
        self.0.source.source()
    }

    /// Original include bundle, absent for single-source and logical inputs.
    #[must_use]
    pub fn bundle(&self) -> Option<&SourceBundle> {
        self.0.source.source_bundle()
    }

    /// Original output declarations and display policy.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.0.metadata
    }

    /// Complete original-family warnings, established before exact evaluation.
    #[must_use]
    pub fn warnings(&self) -> &[crate::FormulaWarning] {
        &self.0.evaluation.warnings
    }

    /// Render warnings using the retained original source identity.
    #[must_use]
    pub fn warning_view(&self) -> impl fmt::Display + '_ {
        self.0.source.warning_view(self.warnings())
    }

    /// Analysis of the exact normalized source used by the class certificate.
    /// If preparation has not needed the complete normalized input, the first
    /// full inspection constructs and retains it and its analysis. Later calls
    /// borrow that shared result. Preparation's summary has the same dependency
    /// graph and class verdicts; full inspection pays canonical ingestion and
    /// analysis work and full-program storage, without an expansion charge.
    #[must_use]
    pub fn source_analysis(&self) -> &themelios_analysis::Analysis {
        self.0
            .evaluation
            .analyzed
            .full_analysis(&self.0.evaluation.analysis)
    }

    /// Structural classes of the retained source analysis. This view does not
    /// require constructing the full analyzed program. Consult
    /// [`Self::analysis_basis`] before using a verdict as a source property.
    #[must_use]
    pub fn source_classes(&self) -> &themelios_analysis::classify::Classes {
        self.0.evaluation.analysis.classes()
    }

    /// Exact pool-free normalized source analyzed before admission.
    /// Shares the complete view and its construction cost with [`Self::source_analysis`].
    #[must_use]
    pub fn analyzed_program(&self) -> &Program {
        self.0.evaluation.analyzed.full_program()
    }

    /// This subject is admitted only from complete normalized-program analysis.
    #[must_use]
    pub const fn analysis_basis(&self) -> crate::AnalysisBasis {
        crate::AnalysisBasis::NormalizedProgram
    }

    /// Source expansion charges retained through exact evaluation.
    #[must_use]
    pub fn expansion_usage(&self) -> &crate::ExpansionUsage {
        &self.0.evaluation.expansion
    }

    /// Applicable key rewrites made during original source preparation.
    #[must_use]
    pub fn keyed_constraints(&self) -> usize {
        self.0.evaluation.keyed_constraints
    }

    /// Completion of the bounded original key analysis.
    #[must_use]
    pub fn key_analysis(&self) -> crate::KeyAnalysis {
        self.0.evaluation.key_analysis
    }
}

pub(crate) struct Evaluated {
    atoms: AtomCatalog,
    selected: Option<Vec<usize>>,
    warnings: Vec<crate::FormulaWarning>,
    statistics: SourceEvaluationStatistics,
    expansion: crate::ExpansionUsage,
    analysis: themelios_analysis::Analysis,
    analyzed: crate::formula_analysis::Summary,
    keyed_constraints: usize,
    key_analysis: crate::KeyAnalysis,
}

/// Classification keeps every accepted charge in the original preparation.
/// An owned order retains the existing canonical source token; it cannot be
/// applied to another independently prepared owner after either value moves.
pub(crate) fn classify(
    preparation: &mut Preparation,
) -> Result<Option<AcyclicPlan>, FormulaFailure> {
    preparation.accounting.controlled::<_, FormulaFailure>(
        preparation.budget.cancellation(),
        crate::grounding_observer::Work::default(),
        preparation.location,
        |counters| {
            AcyclicPlan::prepare(
                &preparation.program,
                &preparation.catalog,
                &preparation.limits,
                counters,
                preparation.location,
            )
        },
    )
}

pub(crate) fn evaluate(
    preparation: Preparation,
    plan: AcyclicPlan,
    observer: Option<&dyn GroundingObserver>,
) -> Result<Evaluated, FormulaFailure> {
    let Preparation {
        program,
        catalog,
        mut accounting,
        mut budget,
        limits,
        options,
        location,
    } = preparation;
    let profile = Profile::new(observer);
    let cancellation = budget.cancellation().cloned();
    accounting.controlled::<_, FormulaFailure>(
        cancellation.as_ref(),
        profile.work(),
        location,
        |counters| {
            counters.work(&limits, location)?;
            if !plan.authenticates(&catalog) {
                return Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    location,
                });
            }
            let (mut catalog, domains) = crate::formula_ground::complete_support(
                &program,
                catalog,
                options.domains,
                &mut budget,
                &profile,
                GroundingWork::new(&limits, counters, location),
            )?;
            // Complete-domain analysis may assist support; exact activity always reads
            // the complete original rows and does not depend on an optional guard.
            drop(domains);
            let (exact, warnings) = {
                let (completed, mut append) = catalog.split(&limits, counters, location)?;
                let queries = completed.queries(options.joins, &limits, counters, location)?;
                let mut computation = Computation::new(&mut append, queries.support());
                let warnings =
                    profile.phase(crate::GroundingPhase::SupportCompletion, None, || {
                        crate::formula_ground::arithmetic::prepare(
                            &program,
                            queries.support(),
                            &mut computation,
                            &limits,
                            &mut budget,
                            counters,
                        )
                    })?;
                let exact = formula_source_activity::evaluate_acyclic(
                    &plan,
                    &program,
                    queries.support(),
                    &profile,
                    &mut formula_source_activity::Context {
                        computation: &mut computation,
                        limits: &limits,
                        budget: &mut budget,
                        counters,
                        location,
                    },
                )?;
                (exact, warnings)
            };
            let Exact {
                atoms,
                selected,
                consistent,
                lease,
            } = exact;
            let mut publication = Publication::new(&mut catalog, counters, location)?;
            publication.observe_peak(size_of::<Admitted>() as u128, &limits, counters, location)?;
            let atoms = publication.atoms(atoms, &limits, counters, location)?;
            publication.retain(lease, &limits, counters, location)?;
            counters.work(&limits, location)?;
            let statistics = SourceEvaluationStatistics {
                work: counters.accounting.work,
                substitutions: counters.accounting.substitutions,
            };
            // All surviving maps have been admitted while their source and scratch
            // coexist. The immutable output keeps the shared canonical payload only.
            drop(publication);
            drop(plan);
            Ok(Evaluated {
                atoms,
                selected: consistent.then_some(selected),
                warnings: warnings.into_values(),
                statistics,
                expansion: budget.usage(),
                analysis: program.analysis,
                analyzed: program.analyzed,
                keyed_constraints: program.keyed_constraints,
                key_analysis: program.key_analysis,
            })
        },
    )
}

#[cfg(test)]
mod tests;
