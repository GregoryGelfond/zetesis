//! Complete selective rounds retain exact carriers while avoiding producer work.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt::Write;

use super::{atom, location, plan, prepare};
use crate::expansion::Budget;
use crate::formula_ir::{HeadIr, Prepared};
use crate::formula_support::producers::ProducerPlan;
use crate::formula_support::{Counters, complete, row_values};
use crate::grounding_observer::Profile;
use crate::{
    AdmissionOptions, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource,
    GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork,
};
use zetesis_core::{Atom, Sign};

#[derive(Default)]
struct Observer {
    work: Cell<GroundingWork>,
    support: Cell<GroundingWork>,
}

impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<themelios_base::span::Location>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.work.set(work);
        if phase == GroundingPhase::SupportCompletion {
            self.support.set(self.support.get().checked_sum(work));
        }
    }
}

struct Measured {
    atoms: Vec<Atom>,
    work: u64,
    operations: GroundingWork,
}

fn measure(
    prepared: &Prepared,
    scheduled: bool,
    limits: &FormulaLimits,
) -> Result<Measured, FormulaFailure> {
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::observed(profile.work());
    let catalog = profile.phase(GroundingPhase::SupportCompletion, None, || {
        let plan = if scheduled {
            ProducerPlan::prepare(prepared, limits, &mut counters, location(prepared))?
        } else {
            None
        };
        complete(
            prepared,
            plan,
            None,
            limits,
            &mut Budget::new(ExpansionLimits::default(), usize::MAX),
            &mut counters,
            location(prepared),
        )
    })?;
    let work = counters.work;
    // Readback is outside the measured completion. It borrows its published
    // catalog under fresh limits and contributes no schedule events.
    let snapshot = catalog.snapshot(
        &FormulaLimits::default(),
        &mut Counters::default(),
        location(prepared),
    )?;
    let mut atoms = Vec::new();
    for predicate in snapshot.relations.predicates() {
        for row in snapshot.relations.rows(predicate) {
            atoms.push(Atom::new(predicate.clone(), row_values(row).cloned().collect()).unwrap());
        }
    }
    atoms.sort();
    Ok(Measured {
        atoms,
        work,
        operations: observer.work.get(),
    })
}

fn chain(steps: i32) -> Prepared {
    let mut source = String::new();
    for start in 0..steps {
        write!(source, "edge({start},{}).", start + 1).unwrap();
    }
    source.push_str("reach(0).reach(Y):-reach(X),edge(X,Y).left(X):-right(X).right(X):-left(X).");
    prepare(&source)
}

#[test]
fn affected_rounds_avoid_repeated_producer_visits() {
    let steps = 64;
    let prepared = chain(steps);
    let reference = measure(&prepared, false, &FormulaLimits::default()).unwrap();
    let scheduled = measure(&prepared, true, &FormulaLimits::default()).unwrap();
    let mut expected: Vec<_> = (0..steps)
        .map(|start| atom("edge", Sign::Positive, &[start, start + 1]))
        .chain((0..=steps).map(|value| atom("reach", Sign::Positive, &[value])))
        .collect();
    expected.sort();
    assert_eq!(scheduled.atoms, expected);
    assert_eq!(scheduled.atoms, reference.atoms);
    // 65 bootstrap facts; the recursive producer runs once per new reach row,
    // including the final row without a successor. The disconnected cycle is
    // never enabled. Full traversal instead visits all 68 producers in 66 rounds.
    assert_eq!(reference.operations.support_producer_visits, Some(4_488));
    assert_eq!(scheduled.operations.support_producer_visits, Some(130));
    assert_eq!(reference.operations.support_rounds, Some(66));
    assert_eq!(scheduled.operations.support_rounds, Some(66));
    assert_eq!(
        reference.operations.binding_snapshots,
        scheduled.operations.binding_snapshots
    );
    // Includes applicability, graph validation, all wake preparation and scans.
    assert!(
        scheduled.work < reference.work,
        "scheduled={} full={}",
        scheduled.work,
        reference.work
    );
    assert!(scheduled.operations.support_peak_bytes.unwrap() > 0);
}

#[test]
fn terminal_predicates_avoid_an_unused_final_snapshot() {
    let prepared = prepare("p(1).p(2).:-p(9).");
    let reference = measure(&prepared, false, &FormulaLimits::default()).unwrap();
    let scheduled = measure(&prepared, true, &FormulaLimits::default()).unwrap();
    assert_eq!(
        scheduled.atoms,
        vec![
            atom("p", Sign::Positive, &[1]),
            atom("p", Sign::Positive, &[2])
        ]
    );
    assert_eq!(scheduled.atoms, reference.atoms);
    assert_eq!(scheduled.operations.support_rounds, Some(2));
    assert_eq!(reference.operations.support_rounds, Some(2));
    assert_eq!(scheduled.operations.support_snapshot_preparations, Some(1));
    assert_eq!(reference.operations.support_snapshot_preparations, Some(2));
    assert_eq!(scheduled.operations.support_producer_visits, Some(2));
    assert_eq!(reference.operations.support_producer_visits, Some(4));
}

#[test]
fn reverse_postings_preserve_signed_predicate_identity() {
    let prepared =
        prepare("p(1).-p(2).p(3,4).a(X):-p(X).b(X):- -p(X).c(X,Y):-p(X,Y).twice(X,Y):-p(X),p(Y).");
    let mut plan = plan(&prepared);
    for (changed, expected) in [
        (atom("p", Sign::Negative, &[2]), vec!["b"]),
        (atom("p", Sign::Positive, &[3, 4]), vec!["c"]),
        (atom("p", Sign::Positive, &[1]), vec!["a", "twice"]),
    ] {
        plan.advance(
            &BTreeSet::from([changed]),
            &FormulaLimits::default(),
            &mut Counters::default(),
            location(&prepared),
        )
        .unwrap();
        let mut schedule = plan.schedule();
        let mut actual = Vec::new();
        let mut previous = None;
        while let Some(index) = schedule
            .next(
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(&prepared),
            )
            .unwrap()
        {
            assert!(previous.is_none_or(|previous| previous < index));
            previous = Some(index);
            let HeadIr::Normal(Some(head)) = &prepared.rules[index].head else {
                panic!("a posting must name a producer")
            };
            actual.push(head.predicate().name());
        }
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }
}

#[test]
fn interrupted_rounds_do_not_publish_a_partial_carrier() {
    let prepared = chain(4);
    let reference = measure(&prepared, false, &FormulaLimits::default()).unwrap();
    let complete = measure(&prepared, true, &FormulaLimits::default()).unwrap();
    for maximum in [0, complete.work / 2, complete.work - 1] {
        let limits = FormulaLimits {
            max_work: maximum,
            ..FormulaLimits::default()
        };
        let Err(error) = measure(&prepared, true, &limits) else {
            panic!("an unfinished support round cannot return its catalog");
        };
        assert!(
            matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. } if observed > limit && limit == u128::from(maximum))
        );
        let retry = measure(&prepared, true, &FormulaLimits::default()).unwrap();
        assert_eq!(retry.atoms, reference.atoms);
        assert_eq!(retry.operations, complete.operations);
    }
}

#[test]
fn wake_capacity_shares_the_support_allowance() {
    let prepared = chain(3);
    let complete = measure(&prepared, true, &FormulaLimits::default()).unwrap();
    let peak = usize::try_from(complete.operations.support_peak_bytes.unwrap()).unwrap();
    assert!(peak > 0);
    let inclusive = measure(
        &prepared,
        true,
        &FormulaLimits {
            max_support_bytes: peak,
            ..FormulaLimits::default()
        },
    )
    .unwrap();
    assert_eq!(inclusive.atoms, complete.atoms);
    assert_eq!(inclusive.operations, complete.operations);
    let Err(error) = measure(
        &prepared,
        true,
        &FormulaLimits {
            max_support_bytes: peak - 1,
            ..FormulaLimits::default()
        },
    ) else {
        panic!("one byte below the observed named peak cannot complete");
    };
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        }
    ));
}

#[test]
fn empty_wake_sets_do_not_discharge_original_constraints() {
    for (source, expected) in [
        ("p(1).:-p(1).", [false, false]),
        ("p(1).:-p(2).", [false, true]),
    ] {
        let observer = Observer::default();
        let admitted = crate::prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_with_observer(Some(&observer))
        .unwrap();
        assert_eq!(admitted.atoms(), &[atom("p", Sign::Positive, &[1])]);
        assert_eq!(observer.support.get().support_rounds, Some(2));
        assert_eq!(
            observer.support.get().support_snapshot_preparations,
            Some(1)
        );
        for (mask, expected) in expected.into_iter().enumerate() {
            let interpretation =
                zetesis_ferraris::Interpretation::new(admitted.theory(), (mask == 1).then_some(0))
                    .unwrap();
            assert_eq!(
                zetesis_ferraris::models(
                    admitted.theory(),
                    &interpretation,
                    zetesis_ferraris::Limits::default(),
                    &zetesis_cpu::Cancellation::default()
                )
                .unwrap(),
                expected
            );
        }
    }
}
