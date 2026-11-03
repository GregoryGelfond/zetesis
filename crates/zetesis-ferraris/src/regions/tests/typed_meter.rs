//! Reserved operations retain a caller's owned error and their exact prefix.

use zetesis_algebra::{Cancellation, Failure, Meter, Quota, Refusal, Stop};

use crate::regions::ledger::Literals;
use crate::test_support::{learning, theory};
use crate::{
    ChoiceHeads, GeneratorOrder, GeneratorOrderError, LearningStore, Narrower, NarrowingScratch,
    Node, OriginalSubject, Region, RegionLimits,
};

#[derive(Debug, PartialEq, Eq)]
enum AccountError {
    Exhausted { label: String, granted: u64 },
    Control(Stop),
}

impl Refusal for AccountError {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Exhausted { .. } => true,
            Self::Control(stop) => stop.is_resource_refusal(),
        }
    }

    fn from_stop(stop: Stop) -> Self {
        Self::Control(stop)
    }
}

struct Account {
    remaining: u64,
    granted: u64,
    refunded: u64,
}

impl Account {
    fn new(remaining: u64) -> Self {
        Self {
            remaining,
            granted: 0,
            refunded: 0,
        }
    }
}

impl Quota for Account {
    type Error = AccountError;

    fn reserve(&mut self, wanted: u64) -> Result<u64, Self::Error> {
        let grant = wanted.min(self.remaining);
        if grant == 0 {
            return Err(AccountError::Exhausted {
                label: String::from("original account"),
                granted: self.granted,
            });
        }
        self.remaining -= grant;
        self.granted += grant;
        Ok(grant)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
        self.refunded += unspent;
    }
}

fn exhausted(granted: u64) -> Failure<AccountError> {
    Failure::Refused(AccountError::Exhausted {
        label: String::from("original account"),
        granted,
    })
}

#[test]
fn narrowing_retains_owned_refusal_and_prefix_state() {
    let theory = super::implication_chain(40);
    let index = Narrower::new(&theory);
    let mut expected = Region::all_open(40);
    let local = index.narrow_known(
        OriginalSubject::new(&theory, None),
        &mut expected,
        &mut index.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits { max_work: 3 },
        &Cancellation::default(),
    );
    assert_eq!(local.result, Err(Stop::WorkLimit));
    let mut actual = Region::all_open(40);
    let mut account = Account::new(3);
    let attempt = index.narrow_known_reserved(
        OriginalSubject::new(&theory, None),
        &mut actual,
        &mut index.knowledge(),
        &mut NarrowingScratch::default(),
        &Cancellation::default(),
        &mut Meter::reserved(&mut account),
    );
    assert_eq!(attempt.result, Err(exhausted(3)));
    assert_eq!(attempt.statistics, local.statistics);
    assert_eq!(actual, expected);
    assert_eq!(account.granted - account.refunded, 3);
}

#[test]
fn reused_preference_meter_reports_each_attempt_delta() {
    let theory = theory(3, vec![], vec![]);
    let index = Narrower::new(&theory);
    let region = Region::all_open(3);
    let mut account = Account::new(1000);
    let mut meter = Meter::reserved(&mut account);
    meter.tick().unwrap();
    for _ in 0..2 {
        let attempt = index.knowledge().preferred_atom_reserved(
            &region,
            &ChoiceHeads::default(),
            &Cancellation::default(),
            &mut meter,
        );
        assert_eq!(attempt.result, Ok(Some(0)));
        assert_eq!(attempt.work, 3);
    }
    assert_eq!(meter.charged(), 7);
    assert_eq!(meter.remaining(), 0);
    assert_eq!(account.granted - account.refunded, 7);
}

#[test]
fn initial_control_refunds_without_charging_an_attempt() {
    let theory = theory(1, vec![], vec![]);
    let index = Narrower::new(&theory);
    let cancellation = Cancellation::default();
    let mut account = Account::new(1000);
    let mut meter = Meter::reserved(&mut account);
    meter.tick().unwrap();
    cancellation.cancel();
    let attempt = index.knowledge().preferred_atom_reserved(
        &Region::all_open(1),
        &ChoiceHeads::default(),
        &cancellation,
        &mut meter,
    );
    assert_eq!(attempt.result, Err(Failure::Stopped(Stop::Cancelled)));
    assert_eq!(attempt.work, 0);
    assert_eq!(meter.charged(), 1);
    assert_eq!(meter.remaining(), 0);
    assert_eq!(account.granted - account.refunded, 1);
}

#[test]
fn generator_construction_preserves_each_typed_boundary() {
    let theory = theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::or_pair([0, 2]),
        ],
        vec![3],
    );
    let extraction =
        crate::producers(&theory, RegionLimits::default(), &Cancellation::default()).unwrap();
    let original = OriginalSubject::new(&theory, extraction.producers.as_ref());
    let mut account = Account::new(0);
    let attempt = GeneratorOrder::prepare_reserved(
        original,
        usize::MAX,
        &Cancellation::default(),
        &mut Meter::reserved(&mut account),
    );
    assert!(matches!(attempt.result,
        Err(GeneratorOrderError::Stopped(error)) if error == exhausted(0)));
    assert_eq!(attempt.work, 0);
    assert_eq!(account.granted, 0);

    let attempt = GeneratorOrder::prepare_reserved(
        original,
        0,
        &Cancellation::default(),
        &mut Meter::reserved(&mut account),
    );
    assert!(matches!(attempt.result,
        Err(GeneratorOrderError::Storage { required, limit: 0 }) if required > 0));
    assert_eq!(attempt.work, 0);
    assert_eq!(account.granted, 0);
}

#[test]
fn learning_delivery_retains_owned_refusal_without_publication() {
    let theory = theory(2, vec![Node::atom(0), Node::atom(1)], vec![]);
    let owner = learning(&theory);
    let mut scratch = NarrowingScratch::default();
    scratch.learning.explanations.prepare(&owner);
    assert!(
        scratch
            .learning
            .explanations
            .retain_founded(
                Literals {
                    conditions: vec![(0, true)],
                    outputs: vec![(1, false)]
                },
                &mut Meter::local(u64::MAX),
                &mut 0,
            )
            .unwrap()
    );
    let mut store = LearningStore::new();
    let mut account = Account::new(1);
    let attempt = scratch.deliver_learning(&owner, &mut store, &mut Meter::reserved(&mut account));
    assert_eq!(attempt.result, Err(exhausted(1)));
    assert_eq!(attempt.work, 1);
    assert_eq!(store.len(), 0);
    assert_eq!(scratch.learned_rows(), 1);
    assert_eq!(account.granted - account.refunded, 1);
}
