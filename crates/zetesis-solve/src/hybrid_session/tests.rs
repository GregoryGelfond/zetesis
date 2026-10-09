//! A recorded worker fault wins over a requested prefix without a timing race.

use super::HybridSession;
use crate::execution_observation::Ignore;
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, Completion, ExecutionResources, Grounder, Interruption, SearchMethod,
    SearchState, SolveConfig, SolveError,
};
use zetesis_cpu::{Cancellation, Stop, regions::Region};
use zetesis_sat::{Incomplete, RegionFilter};
use zetesis_themelios::{
    AdmissionOptions, ConstraintCheckCause, ExpansionLimits, FormulaLimits, HybridFormula,
    prepare_formula,
};

struct Fixture {
    owner: HybridFormula,
    config: SolveConfig,
    cancellation: Cancellation,
    phases: Recorder,
}

impl Fixture {
    fn new() -> Self {
        let owner = prepare_formula(
            "{p;q}. :-p,not q.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_hybrid()
        .unwrap();
        Self {
            owner,
            config: SolveConfig {
                backend: Backend::Cpu,
                grounder: Grounder::Lazy,
                search: SearchMethod::Regions,
                workers: std::num::NonZeroUsize::new(4).unwrap(),
                models: 1,
                ..SolveConfig::default()
            },
            cancellation: Cancellation::default(),
            phases: Recorder::new(false),
        }
    }

    fn session(&self) -> HybridSession<'_> {
        let resources = ExecutionResources::default();
        let mut session = HybridSession::new(
            super::HybridInput {
                core: self.owner.core(),
                subject: crate::Subject::Hybrid(self.owner.clone()),
            },
            &self.config,
            &resources,
            &mut Ignore,
            &self.cancellation,
            &self.phases,
            AnswerSelection::All,
        )
        .unwrap();
        // Exercise actual candidate enumeration, reduct membership and final
        // source checking before introducing the controlled terminal event.
        session
            .next(&self.config, &mut Ignore, &self.cancellation, &self.phases)
            .unwrap()
            .unwrap();
        session
    }
}

#[test]
fn a_recorded_worker_cancellation_supersedes_requested_models() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let source = session.regions.as_ref().unwrap().clone();
    let mut worker = source
        .worker(fixture.owner.core_theory(), &fixture.cancellation)
        .unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    // Invoke the real source worker callback at a known boundary instead of
    // relying on a native worker being interrupted at a particular instant.
    assert_eq!(
        worker.check(
            fixture.owner.core_theory(),
            &Region::all_open(fixture.owner.atom_catalog().atoms().len()),
            &cancelled,
        ),
        Err(Incomplete::RegionFilter)
    );
    drop(worker);
    assert!(
        session
            .next(
                &fixture.config,
                &mut Ignore,
                &fixture.cancellation,
                &fixture.phases
            )
            .is_none()
    );
    let outcome = session.outcome(&fixture.phases);
    assert_eq!(
        outcome.search_state(),
        Some(SearchState::Interrupted(Interruption::Constraint(
            Stop::Cancelled
        ),))
    );
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    let receipt = outcome.hybrid_execution().unwrap();
    assert_eq!(receipt.accepted, 1);
    assert_eq!(receipt.pending, 0);
    assert!(fixture.cancellation.poll().is_ok());
    assert!(
        session
            .next(
                &fixture.config,
                &mut Ignore,
                &fixture.cancellation,
                &fixture.phases
            )
            .is_none()
    );
    assert_eq!(
        session.outcome(&fixture.phases).hybrid_execution(),
        Some(receipt)
    );
}

#[test]
fn a_recorded_worker_failure_supersedes_requested_models() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let source = session.regions.as_ref().unwrap().clone();
    let mut worker = source
        .worker(fixture.owner.core_theory(), &fixture.cancellation)
        .unwrap();
    let expected = fixture.owner.atom_catalog().atoms().len();
    assert_eq!(
        worker.check(
            fixture.owner.core_theory(),
            &Region::all_open(expected + 1),
            &fixture.cancellation,
        ),
        Err(Incomplete::RegionFilter)
    );
    drop(worker);
    let error = session
        .next(
            &fixture.config,
            &mut Ignore,
            &fixture.cancellation,
            &fixture.phases,
        )
        .unwrap()
        .unwrap_err();
    let SolveError::Constraint(failure) = error else {
        panic!("unexpected error: {error:?}")
    };
    assert!(matches!(failure.cause,
        ConstraintCheckCause::WrongRegionSize { expected: size, actual } if size == expected && actual == expected + 1
    ));
    let outcome = session.outcome(&fixture.phases);
    assert_eq!(outcome.completion(), None);
    let receipt = outcome.hybrid_execution().unwrap();
    assert_eq!(receipt.accepted, 1);
    assert_eq!(receipt.pending, 0);
    assert!(receipt.constraints.work >= failure.statistics.work);
    assert!(fixture.cancellation.poll().is_ok());
    assert!(
        session
            .next(
                &fixture.config,
                &mut Ignore,
                &fixture.cancellation,
                &fixture.phases
            )
            .is_none()
    );
    assert_eq!(
        session.outcome(&fixture.phases).hybrid_execution(),
        Some(receipt)
    );
}

#[test]
fn a_construction_stop_precedes_a_later_source_worker_failure() {
    let mut fixture = Fixture::new();
    fixture.config.max_model_work = 0;
    let resources = ExecutionResources::default();
    let mut session = HybridSession::new(
        super::HybridInput {
            core: fixture.owner.core(),
            subject: crate::Subject::Hybrid(fixture.owner.clone()),
        },
        &fixture.config,
        &resources,
        &mut Ignore,
        &fixture.cancellation,
        &fixture.phases,
        AnswerSelection::All,
    )
    .unwrap();
    let primary = Some(SearchState::Interrupted(Interruption::ModelConstruction(
        crate::ModelConstructionStop::Work {
            observed: 1,
            limit: 0,
        },
    )));
    assert_eq!(
        session.core.outcome(&fixture.phases).search_state(),
        primary
    );
    let construction = *session
        .core
        .outcome(&fixture.phases)
        .model_construction()
        .unwrap();
    let source = session.regions.as_ref().unwrap().clone();
    let mut worker = source
        .worker(fixture.owner.core_theory(), &fixture.cancellation)
        .unwrap();
    assert_eq!(
        worker.check(
            fixture.owner.core_theory(),
            &Region::all_open(fixture.owner.atom_catalog().atoms().len() + 1),
            &fixture.cancellation,
        ),
        Err(Incomplete::RegionFilter),
    );
    drop(worker);
    assert!(
        session
            .next(
                &fixture.config,
                &mut Ignore,
                &fixture.cancellation,
                &fixture.phases
            )
            .is_none()
    );
    let outcome = session.outcome(&fixture.phases);
    assert_eq!(outcome.search_state(), primary);
    assert_eq!(outcome.model_construction(), Some(&construction));
    let receipt = *outcome.hybrid_execution().unwrap();
    assert_eq!(receipt.accepted, 0);
    assert_eq!(receipt.pending, 0);
    assert!(source.take_failure().is_none());
    assert!(fixture.cancellation.poll().is_ok());
    assert!(
        session
            .next(
                &fixture.config,
                &mut Ignore,
                &fixture.cancellation,
                &fixture.phases
            )
            .is_none()
    );
    assert_eq!(
        session.outcome(&fixture.phases).hybrid_execution(),
        Some(&receipt)
    );
}

mod objective_tests;
