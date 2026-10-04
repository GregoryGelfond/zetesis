use std::cell::RefCell;

use crate::ProgramSite;
use crate::test_support::location;

use super::*;
use crate::formula::Preparation;
use crate::formula_support::{Accounting, CompletedCatalog, build, testing};
use crate::grounding_observer::Profile;
use crate::{
    FormulaFailure, FormulaLimits, FormulaResource, GroundingObserver, GroundingOutcome,
    GroundingPhase, GroundingWork,
};

#[derive(Default)]
struct Observer(RefCell<Vec<(GroundingOutcome, GroundingWork)>>);

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.borrow_mut().push((outcome, work));
    }
}

fn amounts(work: &GroundingWork) -> [Option<u64>; 7] {
    [
        work.support_construction_work,
        work.support_production_work,
        work.support_join_work,
        work.support_head_work,
        work.support_order_work,
        work.support_wake_work,
        work.support_publication_work,
    ]
}

#[test]
fn every_work_scope_retains_only_accepted_charges() {
    let events: [fn(u64) -> Event; 7] = [
        Event::SupportConstructionWork,
        Event::SupportProductionWork,
        Event::SupportJoinWork,
        Event::SupportHeadWork,
        Event::SupportOrderWork,
        Event::SupportWakeWork,
        Event::SupportPublicationWork,
    ];
    let limits = FormulaLimits {
        max_work: 19,
        ..Default::default()
    };
    for (index, event) in events.into_iter().enumerate() {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let mut counters = Counters::resume(
            Accounting {
                work: 17,
                ..Default::default()
            },
            profile.work(),
        );
        let result = profile.phase(GroundingPhase::SupportCompletion, None, || {
            counters.observe_work(event, |counters| {
                counters.charge_work(2, &limits, location())?;
                counters.work(&limits, location())
            })
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                observed: 20,
                limit: 19,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, 19);
        let records = observer.0.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, GroundingOutcome::Failed);
        let mut expected = [Some(0); 7];
        expected[index] = Some(2);
        assert_eq!(amounts(&records[0].1), expected);
    }
}

#[test]
fn unwinding_records_inner_and_total_accepted_work() {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(
        Accounting {
            work: 17,
            ..Default::default()
        },
        profile.work(),
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        profile.phase(
            GroundingPhase::SupportCompletion,
            None,
            || -> Result<(), FormulaFailure> {
                counters.observe_work(Event::SupportConstructionWork, |counters| {
                    counters.observe_work(Event::SupportProductionWork, |counters| {
                        counters.observe_work(Event::SupportJoinWork, |counters| {
                            counters
                                .work(&FormulaLimits::default(), location())
                                .unwrap();
                        });
                        counters.observe_work(Event::SupportHeadWork, |counters| {
                            counters
                                .work(&FormulaLimits::default(), location())
                                .unwrap();
                            panic!("controlled support unwind");
                        })
                    })
                })
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(counters.accounting.work, 19);
    let records = observer.0.borrow();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, GroundingOutcome::Unwound);
    assert_eq!(
        amounts(&records[0].1),
        [
            Some(2),
            Some(2),
            Some(1),
            Some(1),
            Some(0),
            Some(0),
            Some(0)
        ]
    );
}

fn construct(
    maximum: u64,
    observer: Option<&dyn GroundingObserver>,
) -> (Result<CompletedCatalog, FormulaFailure>, u64, u64) {
    let Preparation {
        catalog,
        accounting,
        program,
        mut budget,
        ..
    } = testing::prepare("p(3).p(1).p(2).q(X):-p(X).");
    let profile = Profile::new(observer);
    let mut counters = Counters::resume(accounting, profile.work());
    let start = counters.accounting.work;
    let limits = FormulaLimits {
        max_work: maximum,
        ..Default::default()
    };
    let result = profile.phase(GroundingPhase::SupportCompletion, None, || {
        build(
            catalog,
            &program,
            None,
            &limits,
            &mut budget,
            &mut counters,
            location(),
        )
    });
    (result, start, counters.accounting.work)
}

fn assert_same_support(measured: &CompletedCatalog, plain: &CompletedCatalog) {
    // Read only after the observed construction scope, under independent work
    // accounts. Cross-owner AtomRef equality compares the actual typed tuples.
    let limits = FormulaLimits {
        max_work: u64::MAX,
        ..Default::default()
    };
    let measured = measured
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let plain = plain
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let measured = measured
        .relations
        .source_atoms()
        .flat_map(|(_, atoms)| atoms.iter())
        .collect::<Vec<_>>();
    let plain = plain
        .relations
        .source_atoms()
        .flat_map(|(_, atoms)| atoms.iter())
        .collect::<Vec<_>>();
    assert_eq!(measured, plain);
}

#[test]
fn construction_attribution_preserves_the_exact_work_cutoff() {
    let baseline = Observer::default();
    let (complete, start, required) = construct(u64::MAX, Some(&baseline));
    assert!(complete.is_ok());
    let work = baseline.0.borrow()[0].1;
    assert_eq!(work.support_construction_work, Some(required - start));
    let operations = [
        work.support_production_work,
        work.support_order_work,
        work.support_wake_work,
        work.support_publication_work,
    ]
    .map(Option::unwrap);
    assert!(operations.iter().all(|amount| *amount > 0));
    // Plan, initial scheduling and snapshot work are measured in the total,
    // but intentionally not assigned to one of the four operation subsets.
    assert!(operations.iter().sum::<u64>() < required - start);
    let join = work.support_join_work.unwrap();
    let head = work.support_head_work.unwrap();
    assert!(join > 0 && head > 0);
    assert!(join + head < work.support_production_work.unwrap());
    for maximum in [start, required - 1, required] {
        let observer = Observer::default();
        let (measured, initial, charged) = construct(maximum, Some(&observer));
        let (plain, plain_initial, plain_charged) = construct(maximum, None);
        assert_eq!(initial, plain_initial);
        assert_eq!(charged, plain_charged);
        assert_eq!(measured.is_ok(), maximum == required);
        match (measured, plain) {
            (Err(measured), Err(plain)) => assert_eq!(measured.to_string(), plain.to_string()),
            (Ok(measured), Ok(plain)) => assert_same_support(&measured, &plain),
            _ => panic!("observation changed the construction result"),
        }
        let records = observer.0.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].0,
            if maximum == required {
                GroundingOutcome::Completed
            } else {
                GroundingOutcome::Failed
            }
        );
        assert_eq!(
            records[0].1.support_construction_work,
            Some(charged - initial)
        );
        let work = records[0].1;
        assert!(
            work.support_join_work.unwrap() + work.support_head_work.unwrap()
                <= work.support_production_work.unwrap()
        );
    }
}
