//! Injected quotas admit each original or frozen read and retain failed prefixes.

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, Interpretation, Narrower,
    NarrowingAttempt, Node, Region, RegionLimits, Theory, producers,
};

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Stopped(Stop),
    Quota,
}

impl From<Stop> for Refusal {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

fn theory() -> Theory {
    // d. p | q :- d. r :- not p. :- q,r.
    Theory::new(
        4,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Or(1, 2),
            Node::Implies(0, 3),
            Node::False,
            Node::Implies(1, 5),
            Node::Atom(3),
            Node::Implies(6, 7),
            Node::And(2, 7),
            Node::Implies(9, 5),
        ],
        vec![0, 4, 8, 10],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn run(
    theory: &Theory,
    narrower: &Narrower,
    frozen: Option<&[bool]>,
    cancellation: &Cancellation,
    charge: impl FnMut() -> Result<(), Refusal>,
) -> (NarrowingAttempt<Refusal>, Region) {
    let mut region = Region::all_open(theory.atom_count());
    let mut knowledge = narrower.knowledge();
    let attempt = if let Some(truth) = frozen {
        narrower.narrow_frozen_known_metered(
            theory,
            truth,
            &mut region,
            &mut knowledge,
            cancellation,
            charge,
        )
    } else {
        let extracted =
            producers(theory, RegionLimits::default(), &Cancellation::default()).unwrap();
        narrower.narrow_known_metered(
            theory,
            extracted.producers.as_ref(),
            &mut region,
            &mut knowledge,
            cancellation,
            charge,
        )
    };
    (attempt, region)
}

#[test]
fn metered_narrowing_matches_local_wrappers() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    let evaluated = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
    let truth = evaluated.result.unwrap();
    assert!(truth.is_model());
    for frozen in [None, Some(truth.truth())] {
        let mut permits = 0;
        let (attempt, region) = run(&theory, &narrower, frozen, &cancellation, || {
            permits += 1;
            Ok(())
        });
        let mut local_region = Region::all_open(theory.atom_count());
        let mut knowledge = narrower.knowledge();
        let local = if let Some(mask) = frozen {
            narrower.narrow_frozen_known(
                &theory,
                mask,
                &mut local_region,
                &mut knowledge,
                RegionLimits::default(),
                &cancellation,
            )
        } else {
            let extracted = producers(&theory, RegionLimits::default(), &cancellation).unwrap();
            narrower.narrow_known(
                &theory,
                extracted.producers.as_ref(),
                &mut local_region,
                &mut knowledge,
                RegionLimits::default(),
                &cancellation,
            )
        }
        .unwrap();
        assert_eq!(attempt.result.unwrap(), local.0);
        assert_eq!(attempt.statistics, local.1);
        assert_eq!(attempt.statistics.work, permits);
        assert_eq!(region, local_region);
    }
}

#[test]
fn every_refused_prefix_stops_before_the_next_read() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    let evaluated = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
    let truth = evaluated.result.unwrap();
    for frozen in [None, Some(truth.truth())] {
        let (complete, _) = run(&theory, &narrower, frozen, &cancellation, || Ok(()));
        assert!(complete.result.is_ok());
        assert!(complete.statistics.work > 2);
        for allowance in 0..complete.statistics.work {
            let mut permits = 0;
            let (attempt, _) = run(&theory, &narrower, frozen, &cancellation, || {
                if permits == allowance {
                    return Err(Refusal::Quota);
                }
                permits += 1;
                Ok(())
            });
            assert_eq!(attempt.result, Err(Refusal::Quota));
            assert_eq!(permits, allowance);
            assert_eq!(attempt.statistics.work, allowance);
        }
    }
}

#[test]
fn cancellation_retains_the_admitted_prefix() {
    let theory = theory();
    let narrower = Narrower::new(&theory);
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace.evaluate(
        &candidate,
        EvaluationLimits::default(),
        &Cancellation::default(),
    );
    let truth = evaluated.result.unwrap();
    for frozen in [None, Some(truth.truth())] {
        let cancellation = Cancellation::default();
        let mut permits = 0;
        let (attempt, _) = run(&theory, &narrower, frozen, &cancellation, || {
            cancellation.poll()?;
            permits += 1;
            if permits == 2 {
                cancellation.cancel();
            }
            Ok(())
        });
        assert_eq!(attempt.result, Err(Refusal::Stopped(Stop::Cancelled)));
        assert_eq!(attempt.statistics.work, 2);
        assert_eq!(permits, 2);
        let (stopped, _) = run(&theory, &narrower, frozen, &cancellation, || {
            panic!("entry cancellation must precede any quota acquisition");
        });
        assert_eq!(stopped.result, Err(Refusal::Stopped(Stop::Cancelled)));
        assert_eq!(stopped.statistics.work, 0);
    }
}
