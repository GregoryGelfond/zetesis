use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Refused {
    observed: u64,
    limit: u64,
}

struct Allowance {
    used: u64,
    limit: u64,
}

impl Allowance {
    fn charge(&mut self, amount: usize) -> Result<(), Refused> {
        let observed = self.used + u64::try_from(amount).unwrap();
        if observed > self.limit {
            return Err(Refused {
                observed,
                limit: self.limit,
            });
        }
        self.used = observed;
        Ok(())
    }
}

fn fixture() -> (Predicate, Vec<Atom>) {
    let predicate = Predicate::new("edges", 3).unwrap();
    let rows = atoms(
        &predicate,
        &[
            numbers(&[1, 2, 1]),
            numbers(&[2, 3, 2]),
            numbers(&[1, 2, 1]),
            numbers(&[1, 2, 3]),
            numbers(&[3, 2, 3]),
        ],
    );
    (predicate, rows)
}

#[test]
fn metered_preparation_preserves_the_complete_table() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let ordinary = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let mut allowance = Allowance {
        used: 7,
        limit: u64::MAX,
    };
    let metered = Table::prepare_with(&relation, &[0, 1, 0], Limits::default(), |amount| {
        allowance.charge(amount)
    })
    .unwrap();
    assert!(std::ptr::eq(metered.relation(), ordinary.relation()));
    assert_eq!(metered.scope(), ordinary.scope());
    assert_eq!(metered.statistics(), ordinary.statistics());
    assert_eq!(metered.support_entries(), ordinary.support_entries());
    assert_eq!(allowance.used, 7 + metered.statistics().work);
    let domains = [numbers(&[1, 3]), numbers(&[2])];
    let inputs = [domains[0].as_slice(), domains[1].as_slice()];
    assert_reference(&metered, &source, &[0, 1, 2, 3, 4], &inputs);
}

#[test]
fn every_preparation_permit_refusal_retains_its_prefix() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let expected = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    for remaining in 0..expected.statistics().work {
        let mut allowance = Allowance {
            used: 7,
            limit: 7 + remaining,
        };
        let failure = Table::prepare_with(&relation, &[0, 1, 0], Limits::default(), |amount| {
            allowance.charge(amount)
        })
        .err()
        .unwrap();
        let MeteredCause::Stopped(Refused { observed, limit }) = failure.cause else {
            panic!("original caller refusal");
        };
        assert_eq!(limit, 7 + remaining);
        assert!(observed > limit);
        assert_eq!(failure.work, allowance.used - 7);
        assert!(failure.work <= remaining);
        assert!(failure.peak_bytes > relation.storage().retained_bytes);
        assert!(failure.peak_bytes <= expected.statistics().peak_bytes);
    }
    let mut allowance = Allowance {
        used: 7,
        limit: 7 + expected.statistics().work,
    };
    let exact = Table::prepare_with(&relation, &[0, 1, 0], Limits::default(), |amount| {
        allowance.charge(amount)
    })
    .unwrap();
    assert_eq!(exact.statistics(), expected.statistics());
    assert_eq!(allowance.used, allowance.limit);
}

#[test]
fn every_selection_permit_refusal_retains_its_prefix() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let left = numbers(&[1, 3]);
    let right = Value::Number(2);
    let domains = [
        Domain::Finite(left.as_slice().into()),
        Domain::Singleton((&right).into()),
    ];
    let expected = table
        .select(&domains, Limits::default(), &Cancellation::default())
        .unwrap();
    for remaining in 0..expected.statistics().work {
        let mut allowance = Allowance {
            used: 11,
            limit: 11 + remaining,
        };
        let failure = table
            .select_with(&domains, Limits::default(), |amount| {
                allowance.charge(amount)
            })
            .err()
            .unwrap();
        let MeteredCause::Stopped(Refused { observed, limit }) = failure.cause else {
            panic!("original caller refusal");
        };
        assert!(observed > limit);
        assert_eq!(limit, 11 + remaining);
        assert_eq!(failure.work, allowance.used - 11);
        assert!(failure.work <= remaining);
        assert!(failure.peak_bytes >= table.statistics().retained_bytes);
        assert!(failure.peak_bytes <= expected.statistics().peak_bytes);
    }
    let mut allowance = Allowance {
        used: 11,
        limit: 11 + expected.statistics().work,
    };
    let exact = table
        .select_with(&domains, Limits::default(), |amount| {
            allowance.charge(amount)
        })
        .unwrap();
    assert_eq!(exact.rows().collect::<Vec<_>>(), vec![0, 2, 4]);
    assert_eq!(exact.words(), expected.words());
    assert_eq!(exact.statistics(), expected.statistics());
    assert_eq!(allowance.used, allowance.limit);
}

#[test]
fn local_work_refusal_requests_no_caller_charge() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let mut amounts = Vec::new();
    let failure = Table::prepare_with(
        &relation,
        &[0, 1, 0],
        Limits {
            max_work: 0,
            ..Limits::default()
        },
        |amount| {
            amounts.push(amount);
            Ok::<(), Refused>(())
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        failure.cause,
        MeteredCause::Table(Cause::Limit {
            resource: Resource::Work,
            observed: 1,
            limit: 0
        })
    );
    assert_eq!(failure.work, 0);
    assert!(!amounts.is_empty());
    assert!(amounts.into_iter().all(|amount| amount == 0));
}

#[test]
fn nullary_operations_poll_before_admission() {
    let predicate = Predicate::new("empty", 0).unwrap();
    let relation = Relation::from_atoms(&predicate, &[], RelationLimits::default()).unwrap();
    let limits = Limits {
        max_bytes: 0,
        max_work: 0,
        ..Limits::default()
    };
    let failure = Table::prepare_with(&relation, &[], limits, |amount| {
        assert_eq!(amount, 0);
        Err("entry control")
    })
    .err()
    .unwrap();
    assert_eq!(failure.cause, MeteredCause::Stopped("entry control"));
    assert_eq!(failure.work, 0);
    assert_eq!(failure.peak_bytes, 0);
    let table =
        Table::prepare(&relation, &[], Limits::default(), &Cancellation::default()).unwrap();
    let failure = table
        .select_with(&[], limits, |amount| {
            assert_eq!(amount, 0);
            Err("entry control")
        })
        .err()
        .unwrap();
    assert_eq!(failure.cause, MeteredCause::Stopped("entry control"));
    assert_eq!(failure.work, 0);
    assert_eq!(failure.peak_bytes, 0);
}

#[test]
fn invalid_scope_preserves_the_table_failure_kind() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let mut allowance = Allowance { used: 7, limit: 7 };
    let failure = Table::prepare_with(&relation, &[0], Limits::default(), |amount| {
        allowance.charge(amount)
    })
    .err()
    .unwrap();
    assert_eq!(failure.cause, MeteredCause::Table(Cause::Scope));
    assert_eq!(failure.work, 0);
    assert_eq!(allowance.used, 7);
    assert!(failure.peak_bytes > relation.storage().retained_bytes);
}

#[test]
fn legacy_cancellation_keeps_precedence_over_work() {
    let (predicate, source) = fixture();
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table = Table::prepare(
        &relation,
        &[0, 1, 0],
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let limits = Limits {
        max_bytes: 0,
        max_work: 0,
        ..Limits::default()
    };
    let failed_prepare = Table::prepare(&relation, &[0, 1, 0], limits, &cancellation)
        .err()
        .unwrap();
    let failed_select = table
        .select(&[Domain::Unrestricted; 2], limits, &cancellation)
        .err()
        .unwrap();
    for failure in [failed_prepare, failed_select] {
        assert_eq!(failure.cause, Cause::Interrupted(crate::Stop::Cancelled));
        assert_eq!(failure.work, 0);
        assert_eq!(failure.peak_bytes, 0);
    }
}
