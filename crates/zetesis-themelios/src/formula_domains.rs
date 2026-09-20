//! Optional domains over exactly the normalized positive source and its flat IR.

use themelios_base::span::Location;
use zetesis_domain::{Analysis, Status};

use crate::expansion::Budget;
use crate::formula_ir::{Prepared, RuleIr};
use crate::formula_support::{Candidates, Counters};
use crate::grounding_observer::{Event, Profile};
use crate::{DomainLimits, DomainObservation, FormulaFailure, FormulaLimits};

mod positive;
pub(crate) use positive::PositiveSource;

/// Only this module constructs the certificate joining an eligible normalized
/// owner, its original rule occurrence array, a completed analysis and the
/// narrowed candidates of every rule, prepared once for every completion
/// round and the final instantiation.
pub(crate) struct Domains<'source> {
    analysis: Analysis<'source>,
    source: PositiveSource<'source>,
    candidates: Vec<Candidates<'source>>,
}

impl<'source> Domains<'source> {
    pub(crate) fn for_rule(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<&Candidates<'source>, FormulaFailure> {
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
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
    profile: &Profile<'_>,
    location: Location,
) -> Result<Option<Domains<'source>>, FormulaFailure> {
    let Some(mut options) = options else {
        profile.domain_analysis(DomainObservation::Disabled);
        return Ok(None);
    };
    let before = counters.work;
    let eligible = PositiveSource::check(prepared, limits, counters, location);
    counters.record(Event::DomainPrepareWork(counters.work - before));
    let Some(source) = eligible? else {
        profile.domain_analysis(DomainObservation::Inapplicable);
        return Ok(None);
    };
    // This API has no caller Control. Logical populations and remaining work
    // bound the uninterruptible call; no cancellation/deadline is invented.
    counters.charge_work(0, limits, location)?;
    options.max_work = options.max_work.min(limits.max_work - counters.work);
    let analysis = zetesis_domain::analyze(&prepared.analyzed, options);
    let work = analysis.statistics().work;
    counters.charge_work(u128::from(work), limits, location)?;
    counters.record(Event::DomainPrepareWork(work));
    profile.domain_analysis(DomainObservation::Analyzed(&analysis));
    if analysis.status() != Status::FixedPoint {
        return Ok(None);
    }
    let before = counters.work;
    let mut candidates = Vec::new();
    for rule in &prepared.rules {
        candidates.push(Candidates::prepare(
            rule, &analysis, limits, budget, counters,
        )?);
    }
    counters.record(Event::DomainPrepareWork(counters.work - before));
    Ok(Some(Domains {
        analysis,
        source,
        candidates,
    }))
}
