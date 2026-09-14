//! Real owner/query attempts preserve failed prefixes and release every guard lease.

use std::cell::Cell;

use super::*;
use crate::formula_domains;
use crate::grounding_observer::Profile;
use crate::{AdmissionOptions, ExpansionLimits, FormulaResource, GroundingObserver, GroundingOutcome,
    GroundingPhase, GroundingWork, ParsedSource};

fn prepared() -> crate::formula_ir::Prepared {
    let parsed = ParsedSource::new("p(1).p(2).q(2).r(X):-p(X),q(X).".into(), AdmissionOptions::default()).unwrap();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut choices = crate::formula_choice_source::Catalog::default();
    let raised = crate::formula_choice_source::raise(parsed.parsed(), &mut crate::metadata::Builder::default(),
        &mut budget, &mut choices).unwrap();
    let location = Location { source: parsed.source().id(), span: parsed.source().span() };
    crate::formula_ir::prepare(&raised, &choices, AdmissionOptions::default(), &FormulaLimits::default(),
        &mut budget, location).unwrap()
}

#[derive(Default)]
struct Observer(Cell<GroundingWork>);
impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool { true }
    fn phase_exit(&self, _: GroundingPhase, _: Option<Location>, _: GroundingOutcome, work: GroundingWork) {
        self.0.set(work);
    }
}

#[test]
fn guard_refusals_keep_work_and_release_live_storage() {
    let prepared = prepared();
    let rule = prepared.rules.last().unwrap();
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut setup = Counters::default();
    let catalog = crate::formula_support::build(&prepared, &limits, &mut budget, &mut setup, rule.location).unwrap();
    let relations = catalog.snapshot(&limits, &mut setup, rule.location).unwrap();
    let support = Support::indexed(&relations, &limits, &setup, rule.location).unwrap();
    let analysis = formula_domains::analyze(&prepared, Some(crate::DomainLimits::default()), &limits,
        &mut setup, &Profile::new(None), rule.location).unwrap().unwrap();
    let domain = analysis.for_rule(prepared.rules.len() - 1, rule).unwrap();
    let base = support.live.get();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut complete = Counters::observed(profile.work());
    let guard = profile.phase(GroundingPhase::RuleInstantiation, Some(rule.location), || {
        support.domain_guards(rule, domain, &limits, &mut budget, &mut complete)
    }).unwrap().unwrap();
    assert!(support.live.get() > base);
    let work = complete.work;
    assert!(work > 0);
    assert_eq!(observer.0.get().domain_prepare_work, Some(work));
    let peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    drop(guard);
    assert_eq!(support.live.get(), base);
    for maximum in 0..work {
        let limited = FormulaLimits { max_work: maximum, ..limits };
        let mut counters = Counters::default();
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        assert!(matches!(support.domain_guards(rule, domain, &limited, &mut budget, &mut counters),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, limit, observed, .. })
                if limit == u128::from(maximum) && observed > limit));
        assert!(counters.work <= maximum);
        assert_eq!(support.live.get(), base, "work limit {maximum}");
    }
    for (maximum, accepted) in [(peak, true), (peak - 1, false)] {
        let limited = FormulaLimits { max_support_bytes: maximum, ..limits };
        let result = support.domain_guards(rule, domain, &limited, &mut budget, &mut Counters::default());
        if accepted { assert!(result.as_ref().is_ok_and(Option::is_some)); }
        else { assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, ..
        }) if observed > limit && limit == maximum as u128)); }
        drop(result);
        assert_eq!(support.live.get(), base);
    }
}

#[test]
fn analysis_and_guards_refuse_equal_but_foreign_owners() {
    let prepared = prepared();
    let foreign = self::prepared();
    let index = prepared.rules.len() - 1;
    let rule = &prepared.rules[index];
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let analysis = formula_domains::analyze(&prepared, Some(crate::DomainLimits::default()), &limits,
        &mut counters, &Profile::new(None), rule.location).unwrap().unwrap();
    assert!(matches!(analysis.for_rule(index, &foreign.rules[index]),
        Err(FormulaFailure::SupportRelation { error: Failure::Owner, .. })));
    let catalog = crate::formula_support::build(&prepared, &limits, &mut budget, &mut counters, rule.location).unwrap();
    let relations = catalog.snapshot(&limits, &mut counters, rule.location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let other = Support::indexed(&relations, &limits, &counters, rule.location).unwrap();
    let guard = support.domain_guards(rule, analysis.for_rule(index, rule).unwrap(), &limits, &mut budget,
        &mut counters).unwrap().unwrap();
    assert!(matches!(crate::formula_support::Join::domain_rule(rule, &other, Some(&guard), &mut budget),
        Err(FormulaFailure::SupportRelation { error: Failure::Owner, .. })));
}

#[test]
fn dependency_projection_cannot_certify_domain_guards() {
    let mut prepared = prepared();
    prepared.analysis_basis = crate::AnalysisBasis::DependencyProjection;
    let location = prepared.rules.last().unwrap().location;
    let mut counters = Counters::default();
    let result = formula_domains::analyze(&prepared, Some(crate::DomainLimits::default()),
        &FormulaLimits::default(), &mut counters, &Profile::new(None), location).unwrap();
    assert!(result.is_none());
    assert_eq!(counters.work, 1);
}
