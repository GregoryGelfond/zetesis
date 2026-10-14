//! One static engine over prepared rules and compact fact columns.

mod census;
mod lower;
pub use census::{
    NumericBody, NumericCensus, NumericComparison, NumericOperand, NumericRuleAnalysis,
};
mod reader;
mod totality;
pub(crate) use totality::TotalBody;

use zetesis_core::TemplateComponentsRef;
use zetesis_core::catalog::{CatalogRead, PredicateRef, ReadError, TermKey};
use zetesis_domain::{Limits, numeric};

use crate::formula_ir::Prepared;
use crate::formula_support::{Counters, components};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use lower::Lowering;
use reader::Reader;

pub(crate) struct Analysis<'prepared> {
    pub(crate) values: numeric::Analysis<TermKey, ProgramSite>,
    predicates: Vec<lower::Predicate>,
    occurrences: Vec<census::Occurrence<'prepared>>,
    census: Option<NumericCensus>,
    totality: Vec<totality::Request<'prepared>>,
}

/// A borrowed numeric-domain report over the prepared program's canonical terms.
/// The reader authenticates every retained key; argument bounds are conservative
/// source bounds and establish neither truth nor answer-set membership.
#[derive(Clone, Copy)]
pub struct NumericAnalysis<'a, 'source> {
    analysis: &'a Analysis<'a>,
    read: CatalogRead<'source>,
}
impl std::fmt::Debug for NumericAnalysis<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumericAnalysis")
            .field("status", &self.analysis.values.status())
            .field("statistics", &self.analysis.values.statistics())
            .finish_non_exhaustive()
    }
}
impl<'a, 'source> NumericAnalysis<'a, 'source> {
    /// Bounds, finite values, producer sites and logical completion evidence.
    #[must_use]
    pub fn analysis(self) -> &'a numeric::Analysis<TermKey, ProgramSite> {
        &self.analysis.values
    }
    /// Resolve a retained term without copying its logical value.
    #[must_use]
    pub fn catalog(self) -> CatalogRead<'source> {
        self.read
    }
    /// Completed diagnostic populations, or None after a logical analysis stop.
    /// Zero counts in a completed census remain distinct from unavailable results.
    #[must_use]
    pub fn census(self) -> Option<NumericCensus> {
        self.analysis.census
    }

    /// Reports in non-fact compiled occurrence order. No reports are published
    /// after an analysis stop. Equal source sites do not merge occurrences.
    #[must_use]
    pub fn rules(self) -> impl ExactSizeIterator<Item = NumericRuleAnalysis<'a>> + 'a {
        self.analysis
            .occurrences
            .iter()
            .enumerate()
            .map(move |(occurrence, rule)| NumericRuleAnalysis {
                rule,
                values: &self.analysis.values,
                occurrence,
            })
    }

    /// Signed predicates and their argument coordinates in this analysis.
    ///
    /// Each entry resolves against the same canonical prefix as the terms.
    pub fn arguments(
        self,
    ) -> impl Iterator<Item = Result<(PredicateRef<'source>, usize, numeric::ArgumentId), ReadError>> + 'a
    where
        'source: 'a,
    {
        self.analysis.predicates.iter().flat_map(move |predicate| {
            predicate
                .arguments
                .iter()
                .enumerate()
                .map(move |(column, &argument)| {
                    self.read
                        .predicate(&predicate.key)
                        .map(|name| (name, column, argument))
                })
        })
    }
}

impl Analysis<'_> {
    pub(crate) fn view<'a, 'source>(
        &'a self,
        read: CatalogRead<'source>,
    ) -> NumericAnalysis<'a, 'source> {
        NumericAnalysis {
            analysis: self,
            read,
        }
    }

    pub(crate) fn argument(
        &self,
        predicate: PredicateRef<'_>,
        column: usize,
        read: CatalogRead<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<numeric::ArgumentId>, FormulaFailure> {
        let mut low = 0;
        let mut high = self.predicates.len();
        while low < high {
            counters.work(limits, location)?;
            let middle = low + (high - low) / 2;
            let retained = read
                .predicate(&self.predicates[middle].key)
                .map_err(|error| read_failure(error, location))?;
            match predicate.compare_ref_with(retained, || counters.work(limits, location))? {
                std::cmp::Ordering::Less => high = middle,
                std::cmp::Ordering::Greater => low = middle + 1,
                std::cmp::Ordering::Equal => {
                    return Ok(self.predicates[middle].arguments.get(column).copied());
                }
            }
        }
        Ok(None)
    }
}

/// The caller accounts work and preserves the original fault and location.
/// Other analysis statistics are meaningful only for a published result.
pub(crate) struct Failure {
    pub(crate) error: numeric::Error<ReadError>,
    pub(crate) work: u64,
    pub(crate) context: Option<ProgramSite>,
}

pub(crate) fn analyze<'prepared>(
    prepared: &'prepared Prepared,
    components: TemplateComponentsRef<'_>,
    read: CatalogRead<'_>,
    limits: Limits,
    diagnostics: bool,
) -> Result<Analysis<'prepared>, Failure> {
    let reader = Reader(read);
    let mut lower = Lowering {
        reader: Reader(read),
        components,
        predicates: Vec::new(),
        finite_width: limits.max_values_per_argument,
        diagnostics,
        occurrences: Vec::new(),
        totality: Vec::new(),
    };
    let values = numeric::analyze(&reader, limits, |input| lower.program(prepared, input))
        .map_err(|failure| Failure {
            error: failure.error,
            work: failure.statistics.work,
            context: failure.context,
        })?;
    // A stopped analysis publishes no usable argument coordinates.
    let (predicates, occurrences, totality) = if values.status() == numeric::Status::FixedPoint {
        (lower.predicates, lower.occurrences, lower.totality)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    Ok(Analysis {
        values,
        predicates,
        occurrences,
        census: None,
        totality,
    })
}

pub(crate) fn failure(error: numeric::Error<ReadError>, location: ProgramSite) -> FormulaFailure {
    match error {
        numeric::Error::Allocation(error) => FormulaFailure::AtomAllocation { error, location },
        numeric::Error::Read(error) => read_failure(error, location),
        // Logical limits are settled as stopped analysis inside the engine.
        // An escaped stop or malformed private IR is not an empty domain.
        numeric::Error::Limit(_) | numeric::Error::InvalidInput => components::missing(location),
    }
}

fn read_failure(error: ReadError, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::TemplateCatalog {
        error: zetesis_core::TemplateCatalogFailure::Read(error),
        location,
    }
}
