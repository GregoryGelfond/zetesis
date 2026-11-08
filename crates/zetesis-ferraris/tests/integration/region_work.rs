//! Injected quotas admit each original or frozen read and retain failed prefixes,
//! with the quota's typed refusal carried by the common meter.

use zetesis_algebra::{Cancellation, Failure, Meter, Quota, Stop};
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
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::or_pair([1, 2]),
                Node::implies(0, 3),
                Node::falsum(),
                Node::implies(1, 5),
                Node::atom(3),
                Node::implies(6, 7),
                Node::and_pair([2, 7]),
                Node::implies(9, 5),
            ],
            vec![],
        )
        .unwrap(),
        vec![0, 4, 8, 10],
        AdmissionLimits::default(),
    )
    .unwrap()
}

impl zetesis_algebra::Refusal for Refusal {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Quota => true,
            Self::Stopped(stop) => zetesis_algebra::Refusal::is_resource_refusal(stop),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

/// A caller's per-read charge grants one permit or retains its exact refusal.
struct PerRead<F> {
    charge: F,
}

impl<F: FnMut() -> Result<(), Refusal>> Quota for PerRead<F> {
    type Error = Refusal;

    fn reserve(&mut self, _wanted: u64) -> Result<u64, Refusal> {
        (self.charge)().map(|()| 1)
    }

    fn refund(&mut self, unspent: u64) {
        assert_eq!(unspent, 0, "a single permit is spent as it is granted");
    }
}

fn run(
    theory: &Theory,
    narrower: &Narrower,
    frozen: Option<&[bool]>,
    cancellation: &Cancellation,
    charge: impl FnMut() -> Result<(), Refusal>,
) -> (NarrowingAttempt<Failure<Refusal>>, Region) {
    let mut region = Region::all_open(theory.atom_count());
    let mut quota = PerRead { charge };
    let mut meter = Meter::reserved(&mut quota);
    let attempt = if let Some(truth) = frozen {
        let mut knowledge = narrower.frozen_knowledge();
        narrower.narrow_frozen_known_reserved(
            zetesis_ferraris::FrozenSubject::new(theory, truth),
            &mut region,
            &mut knowledge,
            &mut zetesis_ferraris::NarrowingScratch::default(),
            cancellation,
            &mut meter,
        )
    } else {
        let mut knowledge = narrower.knowledge();
        let extracted =
            producers(theory, RegionLimits::default(), &Cancellation::default()).unwrap();
        narrower.narrow_known_reserved(
            zetesis_ferraris::OriginalSubject::new(theory, extracted.producers.as_ref()),
            &mut region,
            &mut knowledge,
            &mut zetesis_ferraris::NarrowingScratch::default(),
            cancellation,
            &mut meter,
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
        let local = if let Some(mask) = frozen {
            let mut knowledge = narrower.frozen_knowledge();
            narrower.narrow_frozen_known(
                zetesis_ferraris::FrozenSubject::new(&theory, mask),
                &mut local_region,
                &mut knowledge,
                &mut zetesis_ferraris::NarrowingScratch::default(),
                RegionLimits::default(),
                &cancellation,
            )
        } else {
            let mut knowledge = narrower.knowledge();
            let extracted = producers(&theory, RegionLimits::default(), &cancellation).unwrap();
            narrower.narrow_known(
                zetesis_ferraris::OriginalSubject::new(&theory, extracted.producers.as_ref()),
                &mut local_region,
                &mut knowledge,
                &mut zetesis_ferraris::NarrowingScratch::default(),
                RegionLimits::default(),
                &cancellation,
            )
        };
        assert_eq!(attempt.result.unwrap(), local.result.unwrap());
        assert_eq!(attempt.statistics, local.statistics);
        assert_eq!(attempt.preferred, local.preferred);
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
            assert_eq!(attempt.result, Err(Failure::Refused(Refusal::Quota)));
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
        assert_eq!(
            attempt.result,
            Err(Failure::Refused(Refusal::Stopped(Stop::Cancelled)))
        );
        assert_eq!(attempt.statistics.work, 2);
        assert_eq!(permits, 2);
        let (stopped, _) = run(&theory, &narrower, frozen, &cancellation, || {
            panic!("entry cancellation must precede any quota acquisition");
        });
        assert_eq!(stopped.result, Err(Failure::Stopped(Stop::Cancelled)));
        assert_eq!(stopped.statistics.work, 0);
    }
}
