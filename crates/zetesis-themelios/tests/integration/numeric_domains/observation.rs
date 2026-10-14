//! Capture only owned observer results; never ask for a full Program view.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use zetesis_domain::numeric::{Bound, Status};
use zetesis_themelios::{
    DomainObservation, GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
    ProgramSite,
};

#[derive(Clone, Debug)]
pub(super) struct Argument {
    pub bound: Bound,
    pub values: Option<Vec<String>>,
    pub producers: Vec<ProgramSite>,
}

#[derive(Default)]
pub(super) struct Observation {
    pub status: Cell<Option<Status>>,
    pub callbacks: Cell<usize>,
    pub work: Cell<GroundingWork>,
    pub formula_phases: Cell<usize>,
    arguments: RefCell<BTreeMap<(String, usize, usize), Argument>>,
}

impl Observation {
    pub(super) fn argument(&self, name: &str, arity: usize, column: usize) -> Argument {
        self.arguments.borrow()[&(name.to_owned(), arity, column)].clone()
    }

    pub(super) fn argument_count(&self) -> usize {
        self.arguments.borrow().len()
    }

    pub(super) fn completed(&self) {
        assert_eq!(self.callbacks.get(), 1);
        assert_eq!(self.status.get(), Some(Status::FixedPoint));
        assert!(self.work.get().domain_prepare_work.unwrap() > 0);
    }
}

impl GroundingObserver for Observation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }

    fn domain_analysis(&self, observation: DomainObservation<'_, '_>) {
        let DomainObservation::Analyzed(view) = observation else {
            return;
        };
        self.callbacks.set(self.callbacks.get() + 1);
        let analysis = view.analysis();
        self.status.set(Some(analysis.status()));
        let mut arguments = self.arguments.borrow_mut();
        for argument in view.arguments() {
            let (predicate, column, coordinate) = argument.unwrap();
            let values = analysis.values(coordinate).map(|values| {
                values
                    .map(|key| view.catalog().term(key).unwrap().to_string())
                    .collect()
            });
            let producers = analysis.producers(coordinate).unwrap().to_vec();
            assert!(producers.iter().all(|site| site.location().is_some()));
            let key = (predicate.name().to_owned(), predicate.arity(), column);
            assert!(
                arguments
                    .insert(
                        key,
                        Argument {
                            bound: analysis.argument(coordinate).unwrap(),
                            values,
                            producers,
                        }
                    )
                    .is_none()
            );
        }
    }

    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.work.set(self.work.get().checked_sum(work));
        if phase == GroundingPhase::FormulaInitialization {
            self.formula_phases.set(self.formula_phases.get() + 1);
        }
    }
}
