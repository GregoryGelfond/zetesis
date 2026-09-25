//! Real owner/query attempts preserve failed prefixes and release every guard lease.

use crate::formula_support::{Context, GroundingWork as WorkContext};
use std::cell::Cell;

use super::*;
use crate::formula_domains;
use crate::grounding_observer::Profile;
use crate::{
    ExpansionLimits, FormulaResource, GroundingObserver, GroundingOutcome, GroundingPhase,
    GroundingWork,
};

fn prepared() -> crate::formula::Preparation {
    crate::formula_support::testing::prepare("p(1).p(2).q(2).r(X):-p(X),q(X).")
}

// Analysis and support completion use the same prepared vocabulary.
// Domain certificates borrow the source program, independently of its append tail.
fn analyze<'source>(
    prepared: &'source crate::formula_ir::Prepared,
    catalog: &mut crate::formula_support::SupportCatalog,
    options: Option<crate::DomainLimits>,
    budget: &mut Budget,
    profile: &Profile<'_>,
    work: WorkContext<'_>,
) -> Result<Option<formula_domains::Domains<'source>>, FormulaFailure> {
    let WorkContext {
        limits,
        counters,
        location,
    } = work;
    let (relations, mut append) = catalog.split(limits, counters, location)?;
    let support = Support::indexed(&relations, limits, counters, location)?;
    let mut computation = Computation::new(&mut append, &support);
    formula_domains::analyze(
        prepared,
        options,
        budget,
        profile,
        Context::new(&mut computation, limits, counters, location),
    )
}

#[derive(Default)]
struct Observer(Cell<GroundingWork>);
impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(work);
    }
}

#[test]
fn guard_work_refusals_preserve_admitted_prefixes() {
    let crate::formula::Preparation {
        program: prepared,
        mut catalog,
        ..
    } = prepared();
    let rule = prepared.rules.last().unwrap();
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut setup = Counters::default();
    let analysis = analyze(
        &prepared,
        &mut catalog,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        WorkContext::new(&limits, &mut setup, rule.location),
    )
    .unwrap()
    .unwrap();
    let catalog = crate::formula_support::build(
        catalog,
        &prepared,
        None,
        &limits,
        &mut budget,
        &mut setup,
        rule.location,
    )
    .unwrap();
    let completed = catalog
        .snapshot(&limits, &mut setup, rule.location)
        .unwrap();
    let queries = completed
        .queries(crate::JoinStrategy::Indexed, &limits, &setup, rule.location)
        .unwrap();
    let support = queries.support();
    let computation = queries.computation(rule.location).unwrap();
    let domain = analysis.for_rule(prepared.rules.len() - 1, rule).unwrap();
    let base = support.live.get();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut complete = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    let guard = profile
        .phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || support.domain_guards(rule, domain, &computation, &limits, &mut complete),
        )
        .unwrap()
        .unwrap();
    assert!(support.live.get() > base);
    let work = complete.accounting.work;
    assert!(work > 1);
    assert_eq!(observer.0.get().domain_prepare_work, Some(work));
    drop(guard);
    assert_eq!(support.live.get(), base);
    assert_eq!(rule.variables, 1);
    for maximum in 0..work {
        let limited = FormulaLimits {
            max_work: maximum,
            ..limits
        };
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let mut counters = Counters::resume(
            crate::formula_support::Accounting::default(),
            profile.work(),
        );
        let result = profile.phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || support.domain_guards(rule, domain, &computation, &limited, &mut counters),
        );
        assert!(matches!(
            &result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work, limit, observed, ..
            }) if *limit == u128::from(maximum) && observed > limit
        ));
        assert!(counters.accounting.work <= maximum);
        assert_eq!(
            observer.0.get().domain_prepare_work,
            Some(counters.accounting.work)
        );
        assert_eq!(support.live.get(), base, "work limit {maximum}");
    }
}

#[test]
fn guard_storage_admission_is_inclusive() {
    let crate::formula::Preparation {
        program: prepared,
        mut catalog,
        ..
    } = prepared();
    let rule = prepared.rules.last().unwrap();
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut setup = Counters::default();
    let analysis = analyze(
        &prepared,
        &mut catalog,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        WorkContext::new(&limits, &mut setup, rule.location),
    )
    .unwrap()
    .unwrap();
    let catalog = crate::formula_support::build(
        catalog,
        &prepared,
        None,
        &limits,
        &mut budget,
        &mut setup,
        rule.location,
    )
    .unwrap();
    let completed = catalog
        .snapshot(&limits, &mut setup, rule.location)
        .unwrap();
    let queries = completed
        .queries(crate::JoinStrategy::Indexed, &limits, &setup, rule.location)
        .unwrap();
    let support = queries.support();
    let computation = queries.computation(rule.location).unwrap();
    let domain = analysis.for_rule(prepared.rules.len() - 1, rule).unwrap();
    let base = support.live.get();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    let guard = profile
        .phase(
            GroundingPhase::RuleInstantiation,
            Some(rule.location),
            || support.domain_guards(rule, domain, &computation, &limits, &mut counters),
        )
        .unwrap()
        .unwrap();
    assert!(support.live.get() > base);
    let peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    drop(guard);
    assert_eq!(support.live.get(), base);
    for (maximum, accepted) in [(peak, true), (peak - 1, false)] {
        let limited = FormulaLimits {
            max_support_bytes: maximum,
            ..limits
        };
        let result = support.domain_guards(
            rule,
            domain,
            &computation,
            &limited,
            &mut Counters::default(),
        );
        if accepted {
            assert!(result.as_ref().is_ok_and(Option::is_some));
        } else {
            assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes, observed, limit, ..
        }) if observed > limit && limit == maximum as u128));
        }
        drop(result);
        assert_eq!(support.live.get(), base);
    }
}

#[test]
fn analysis_refuses_an_equal_foreign_rule() {
    let crate::formula::Preparation {
        program: prepared,
        mut catalog,
        ..
    } = prepared();
    let foreign_owner = self::prepared();
    let foreign = &foreign_owner.program;
    let index = prepared.rules.len() - 1;
    let rule = &prepared.rules[index];
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut counters = Counters::default();
    let analysis = analyze(
        &prepared,
        &mut catalog,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        WorkContext::new(&limits, &mut counters, rule.location),
    )
    .unwrap()
    .unwrap();
    assert!(matches!(
        analysis.for_rule(index, &foreign.rules[index]),
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            ..
        })
    ));
}

#[test]
fn domain_guards_refuse_a_foreign_query_owner() {
    let crate::formula::Preparation {
        program: prepared,
        mut catalog,
        ..
    } = prepared();
    let index = prepared.rules.len() - 1;
    let rule = &prepared.rules[index];
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let analysis = analyze(
        &prepared,
        &mut catalog,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        WorkContext::new(&limits, &mut counters, rule.location),
    )
    .unwrap()
    .unwrap();
    let catalog = crate::formula_support::build(
        catalog,
        &prepared,
        None,
        &limits,
        &mut budget,
        &mut counters,
        rule.location,
    )
    .unwrap();
    let completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let other_queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let support = queries.support();
    let computation = queries.computation(rule.location).unwrap();
    let other = other_queries.support();
    assert!(std::ptr::eq(support.relations, other.relations));
    assert!(!std::ptr::eq(support, other));
    let guard = support
        .domain_guards(
            rule,
            analysis.for_rule(index, rule).unwrap(),
            &computation,
            &limits,
            &mut counters,
        )
        .unwrap()
        .unwrap();
    let computation = other_queries.computation(rule.location).unwrap();
    assert!(matches!(
        crate::formula_support::Join::domain_rule(
            rule,
            other,
            Some(&guard),
            &computation,
            &limits,
            &mut budget,
            &mut counters
        ),
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            ..
        })
    ));
}

#[test]
fn dependency_projection_cannot_certify_domain_guards() {
    let crate::formula::Preparation {
        program: mut prepared,
        mut catalog,
        ..
    } = prepared();
    prepared.analysis_basis = crate::AnalysisBasis::DependencyProjection;
    let location = prepared.rules.last().unwrap().location;
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut counters = Counters::default();
    let limits = FormulaLimits::default();
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    // Authenticate the source prefix before measuring the classification check.
    let before = counters.accounting.work;
    let result = formula_domains::analyze(
        &prepared,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        Context::new(&mut computation, &limits, &mut counters, location),
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(counters.accounting.work - before, 1);
}

fn with_candidates(
    text: &str,
    run: impl FnOnce(
        &crate::formula_ir::Prepared,
        &Candidates,
        &Support<'_>,
        &Computation<'_, '_>,
        &mut Counters,
        usize,
    ),
) {
    let crate::formula::Preparation {
        program,
        mut catalog,
        ..
    } = crate::formula_support::testing::prepare(text);
    let rule = program.rules.last().unwrap();
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut counters = Counters::default();
    let domains = analyze(
        &program,
        &mut catalog,
        Some(crate::DomainLimits::default()),
        &mut budget,
        &Profile::new(None),
        WorkContext::new(&limits, &mut counters, rule.location),
    )
    .unwrap()
    .unwrap();
    let admission_work = budget.usage().term_work;
    let catalog = crate::formula_support::build(
        catalog,
        &program,
        None,
        &limits,
        &mut budget,
        &mut counters,
        rule.location,
    )
    .unwrap();
    let completed = catalog
        .snapshot(&limits, &mut counters, rule.location)
        .unwrap();
    let queries = completed
        .queries(
            crate::JoinStrategy::Indexed,
            &limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let computation = queries.computation(rule.location).unwrap();
    run(
        &program,
        domains.for_rule(program.rules.len() - 1, rule).unwrap(),
        queries.support(),
        &computation,
        &mut counters,
        admission_work,
    );
}

#[test]
fn unary_filters_reuse_admitted_candidate_cells() {
    for source in [
        "p(1).p(2).p(3).r(X):-p(X),X>=1.",
        "p(1).p(2).p(3).r(X):-p(X),X>=1,X<=3.",
    ] {
        with_candidates(
            source,
            |_, candidates, _, computation, _, admission_work| {
                let selected = candidates.by_variable[0].clone().unwrap();
                let numbers: Vec<_> = selected
                    .map(|slot| {
                        let key = candidates.values.key(slot).unwrap().unwrap();
                        let zetesis_core::ValueNodeRef::Number(number) =
                            computation.read().term(&key).unwrap().descriptor()
                        else {
                            panic!("numeric domain")
                        };
                        number
                    })
                    .collect();
                assert_eq!(numbers, [1, 2, 3]);
                // One source conversion per candidate, independently of how many
                // unary filters inspect it. Ordinary evaluation work is separate.
                assert_eq!(admission_work, numbers.len());
            },
        );
    }
}

#[test]
fn candidate_text_borrows_the_admitted_source_spelling() {
    with_candidates(
        "p(\"repeated source spelling\").p(other).q(\"repeated source spelling\").r(X):-p(X),q(X).",
        |program, candidates, _, computation, counters, _| {
            // Normalization may reorder facts; locate the actual typed source
            // occurrence instead of treating its position as logical identity.
            let source = program
                .rules
                .iter()
                .find_map(|rule| {
                    let crate::formula_ir::HeadIr::Normal(Some(pattern)) = &rule.head else {
                        return None;
                    };
                    let pattern = computation
                        .static_pattern(
                            *pattern,
                            &FormulaLimits::default(),
                            counters,
                            rule.location,
                        )
                        .unwrap();
                    let Some(Term::Constant(source)) = pattern.terms().at(0) else {
                        return None;
                    };
                    matches!(source.descriptor(), zetesis_core::ValueNodeRef::String(_))
                        .then_some(source)
                })
                .expect("admitted source string");
            let selected = candidates.by_variable[0].as_ref().unwrap();
            assert_eq!(selected.len(), 1);
            let key = candidates.values.key(selected.start).unwrap().unwrap();
            let candidate = computation.read().term(&key).unwrap();
            let (
                zetesis_core::ValueNodeRef::String(source),
                zetesis_core::ValueNodeRef::String(candidate),
            ) = (source.descriptor(), candidate.descriptor())
            else {
                panic!("typed string")
            };
            assert_eq!(candidate, "repeated source spelling");
            assert_eq!(candidate.as_ptr(), source.as_ptr());
        },
    );
}

#[test]
fn candidate_resolution_refuses_a_foreign_vocabulary() {
    with_candidates(
        "p(1).p(2).q(2).r(X):-p(X),q(X).",
        |program, candidates, support, _, counters, _| {
            let rule = program.rules.last().unwrap();
            let baseline = support.live.get();
            let mut foreign = crate::formula_support::testing::Fixture::default();
            foreign.with(rule.location, |_, computation, _| {
                assert!(matches!(
                    support.domain_guards(
                        rule,
                        candidates,
                        computation,
                        &FormulaLimits::default(),
                        counters,
                    ),
                    Err(FormulaFailure::TermAssignment {
                        error: zetesis_core::catalog::AssignmentError::Read(
                            zetesis_core::catalog::ReadError::ForeignCatalog
                        ),
                        ..
                    })
                ));
            });
            assert_eq!(support.live.get(), baseline);
        },
    );
}
