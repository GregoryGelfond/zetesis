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

/// Independent consumers of one completed all-producer analysis. Finite guards
/// retain their positive-flat certificate; static totality retains exact normal
/// rule occurrences. The full numeric result is temporary, not another cache.
pub(crate) struct Domains<'source> {
    finite: Option<Finite<'source>>,
    totality: Vec<(usize, numeric_ir::TotalBody<'source>)>,
}

struct Finite<'source> {
    source: PositiveSource<'source>,
    candidates: Vec<Candidates>,
}

impl Domains<'_> {
    pub(crate) fn for_rule(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<Option<&Candidates>, FormulaFailure> {
        let Some(finite) = &self.finite else {
            return Ok(None);
        };
        if finite.source.contains(index, rule)
            && let Some(candidates) = finite.candidates.get(index)
        {
            Ok(Some(candidates))
        } else {
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: rule.location,
            })
        }
    }

    pub(crate) fn total_body(
        &self,
        index: usize,
        rule: &RuleIr,
    ) -> Result<Option<&numeric_ir::TotalBody<'_>>, FormulaFailure> {
        let Ok(position) = self
            .totality
            .binary_search_by_key(&index, |(index, _)| *index)
        else {
            return Ok(None);
        };
        let certificate = &self.totality[position].1;
        if certificate.belongs_to(rule) {
            Ok(Some(certificate))
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
    let totality = analysis.total_bodies(components, limits, counters);
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    let totality = totality?;
    let before = counters.accounting.work;
    let eligible = PositiveSource::check(prepared, Some(components), limits, counters, location);
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    let source = eligible?;
    let before = counters.accounting.work;
    let mut candidates = Vec::new();
    if source.is_some() {
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
    }
    counters.record(Event::DomainPrepareWork(counters.accounting.work - before));
    let finite = source.map(|source| Finite { source, candidates });
    Ok((finite.is_some() || !totality.is_empty()).then_some(Domains { finite, totality }))
}
