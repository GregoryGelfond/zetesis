//! Shared numeric analysis over prepared producers, with separately certified guards.

use crate::formula_support::{Context, GroundingWork};
use zetesis_domain::numeric::Status;

use crate::formula_ir::{Prepared, RuleIr};
use crate::formula_support::{Candidates, Computation};
use crate::grounding_observer::{Event, Profile};
use crate::{DomainLimits, DomainObservation, FormulaFailure};

pub(crate) mod numeric_ir;
mod positive;
pub(crate) use positive::PositiveSource;

/// Only this module constructs the certificate joining an eligible normalized
/// owner, its original rule occurrences and narrowed canonical candidates.
/// The full static analysis is temporary; guards retain only selected IDs.
pub(crate) struct Domains<'source> {
    source: PositiveSource<'source>,
    candidates: Vec<Candidates>,
}

impl Domains<'_> {
    pub(crate) fn for_rule(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<&Candidates, FormulaFailure> {
        if self.source.contains(index, rule)
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
    // The engine reads every compiled producer, independently of the narrower
    // finite-guard policy. It never needs a reconstructed source Program.
    let components = computation.static_components(location)?;
    counters.poll(location)?;
    options.max_work = options
        .max_work
        .min(limits.max_work - counters.accounting.work);
    let diagnostics = profile.domain_diagnostics_enabled();
    let result = numeric_ir::analyze(
        prepared,
        components,
        computation.read(),
        options,
        diagnostics,
    );
    let (work, site) = match &result {
        Ok(analysis) => (analysis.values.statistics().work, analysis.values.context()),
        Err(error) => (error.work, error.context),
    };
    let site = site.unwrap_or(location);
    let accounted = counters.absorb(u128::from(work), limits, site);
    counters.record(Event::DomainPrepareWork(work));
    // An owner/shape fault retains its meaning even if control arrived meanwhile.
    let semantic_failure = result.as_ref().is_err_and(|error| {
        !matches!(
            error.error,
            zetesis_domain::numeric::Error::Allocation(_)
                | zetesis_domain::numeric::Error::Limit(_)
        )
    });
    let result = result.map_err(|error| numeric_ir::failure(error.error, site));
    if semantic_failure {
        return result.map(|_| None);
    }
    accounted?;
    counters.poll(site)?;
    let mut analysis = result?;
    if diagnostics {
        let before = counters.accounting.work;
        let census = analysis.prepare_census(components, limits, counters, location);
        counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
        census?;
    }
    profile.domain_analysis(DomainObservation::Analyzed(
        analysis.view(computation.read()),
    ));
    if analysis.values.status() != Status::FixedPoint {
        return Ok(None);
    }
    let before = counters.accounting.work;
    let eligible = PositiveSource::check(prepared, Some(components), limits, counters, location);
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    let Some(source) = eligible? else {
        return Ok(None);
    };
    let before = counters.accounting.work;
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(prepared.rules_without_facts().len())
        .map_err(|error| FormulaFailure::AtomAllocation { error, location })?;
    for rule in prepared.rules_without_facts() {
        candidates.push(Candidates::prepare(
            rule,
            &analysis,
            computation,
            limits,
            counters,
        )?);
    }
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    Ok(Some(Domains { source, candidates }))
}
