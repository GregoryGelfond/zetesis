//! Optional domains over exactly the normalized source and its positive-flat rules.

use crate::formula_support::{Context, GroundingWork};

use zetesis_domain::{Analysis, Status};

use crate::expansion::Budget;
use crate::formula_ir::{Prepared, RuleIr};
use crate::formula_support::{Candidates, Computation};
use crate::grounding_observer::{Event, Profile};
use crate::{DomainLimits, DomainObservation, FormulaFailure};

mod positive;
pub(crate) use positive::PositiveSource;

/// Only this module constructs the certificate joining an eligible normalized
/// owner, its original rule occurrence array, a completed analysis and the
/// narrowed candidates of every rule, prepared once for every completion
/// round and the final instantiation.
pub(crate) struct Domains<'source> {
    analysis: Analysis<'source>,
    source: PositiveSource<'source>,
    candidates: Vec<Candidates>,
}

impl Domains<'_> {
    pub(crate) fn for_rule(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<&Candidates, FormulaFailure> {
        if self.analysis.belongs_to(&self.source.prepared().analyzed)
            && self.source.contains(index, rule)
            && let Some(candidates) = self.candidates.get(index)
        {
            Ok(candidates)
        } else {
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            })
        }
    }
}

pub(crate) fn analyze<'source>(
    prepared: &'source Prepared,
    options: Option<DomainLimits>,
    budget: &mut Budget,
    profile: &Profile<'_>,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Result<Option<Domains<'source>>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let Some(mut options) = options else {
        profile.domain_analysis(DomainObservation::Disabled);
        return Ok(None);
    };
    let before = counters.accounting.work;
    let eligible = PositiveSource::check(
        prepared,
        Some(computation.static_components(location)?),
        limits,
        counters,
        location,
    );
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    let Some(source) = eligible? else {
        profile.domain_analysis(DomainObservation::Inapplicable);
        return Ok(None);
    };
    // Domain analysis has no internal control hook. Its logical populations and
    // remaining work bound this cooperative interval; poll before and after it.
    counters.charge_work(0, limits, location)?;
    options.max_work = options
        .max_work
        .min(limits.max_work - counters.accounting.work);
    let analysis = zetesis_domain::analyze(&prepared.analyzed, options);
    let work = analysis.statistics().work;
    counters.charge_work(u128::from(work), limits, location)?;
    counters.record(Event::DomainPrepareWork(work));
    profile.domain_analysis(DomainObservation::Analyzed(&analysis));
    if analysis.status() != Status::FixedPoint {
        return Ok(None);
    }
    let before = counters.accounting.work;
    let mut candidates = Vec::new();
    for rule in &prepared.rules {
        candidates.push(Candidates::prepare(
            rule,
            &analysis,
            computation,
            limits,
            budget,
            counters,
        )?);
    }
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    Ok(Some(Domains {
        analysis,
        source,
        candidates,
    }))
}
