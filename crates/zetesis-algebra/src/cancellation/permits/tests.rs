use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

fn grant(lease: &PermitLease, amount: u64) {
    assert!(
        lease
            .renew::<()>(|prior| {
                assert_eq!((prior.granted(), prior.used(), prior.unused()), (0, 0, 0));
                Ok(Some(amount))
            })
            .unwrap()
    );
}

#[test]
fn both_views_consume_the_same_prefix_from_the_actual_owner() {
    for admitted in 0..8 {
        for wanted in 0..10 {
            let mut charged = 0;
            let mut limit = admitted;
            let lease = PermitLease {
                granted: Cell::new(admitted),
                remaining: Cell::new(admitted),
            };
            let expected = admitted.min(wanted);
            assert_eq!(
                take_available(
                    &mut Counted {
                        charged: &mut charged,
                        admitted: &mut limit,
                    },
                    wanted
                ),
                expected
            );
            assert_eq!(take_available(&mut Leased(&lease), wanted), expected);
            assert_eq!((charged, limit), (expected, admitted));
            assert_eq!(
                (lease.granted.get(), lease.remaining.get()),
                (admitted, admitted - expected)
            );
        }
    }
}

#[test]
fn refused_ticks_preserve_both_empty_views_and_the_error_payload() {
    let error = (91, "source refusal");
    let mut charged = 5;
    let mut limit = 5;
    let mut calls = 0;
    assert_eq!(
        tick(
            &mut Counted {
                charged: &mut charged,
                admitted: &mut limit,
            },
            |_| {
                calls += 1;
                Err(error)
            }
        ),
        Err(error)
    );
    let lease = PermitLease::new();
    assert_eq!(
        lease.tick(|| {
            calls += 1;
            Err(error)
        }),
        Err(error)
    );
    assert_eq!(calls, 2);
    assert_eq!((charged, limit), (5, 5));
    assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
}

#[test]
fn settlement_retains_counted_history_but_detaches_the_local_grant() {
    let mut charged = 3;
    let mut limit = 5;
    let mut counted = Vec::new();
    for _ in 0..2 {
        settle(
            &mut Counted {
                charged: &mut charged,
                admitted: &mut limit,
            },
            |prior| counted.push((prior.granted(), prior.used(), prior.unused())),
        );
    }
    assert_eq!(counted, [(5, 3, 2), (3, 3, 0)]);
    assert_eq!((charged, limit), (3, 3));

    let lease = PermitLease::new();
    grant(&lease, 5);
    lease
        .take::<()>(3, || panic!("existing permits suffice"))
        .unwrap();
    let mut leased = Vec::new();
    for _ in 0..2 {
        lease.settle(|prior| {
            assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
            leased.push((prior.granted(), prior.used(), prior.unused()));
        });
    }
    assert_eq!(leased, [(5, 3, 2)]);
}

#[test]
fn zero_take_skips_refill_but_zero_take_up_to_observes_an_empty_grant() {
    let lease = PermitLease::new();
    lease
        .take::<()>(0, || panic!("zero take does not refill"))
        .unwrap();
    let error = (7, "no grant");
    assert_eq!(lease.take_up_to(0, || Err(error)), Err(error));
    assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
    assert_eq!(
        lease.take_up_to::<()>(0, || {
            grant(&lease, 4);
            Ok(())
        }),
        Ok(0)
    );
    assert_eq!((lease.granted.get(), lease.remaining.get()), (4, 4));
    assert_eq!(
        lease.take_up_to::<()>(0, || panic!("existing grant needs no refill")),
        Ok(0)
    );
}

#[test]
fn a_failed_bulk_refill_retains_the_consumed_prefix() {
    let lease = PermitLease::new();
    let mut attempts = 0;
    let mut published = Vec::new();
    let error = (17, "end of allowance");
    assert_eq!(
        lease.take(6, || {
            attempts += 1;
            lease
                .renew(|prior| {
                    published.push((prior.granted(), prior.used(), prior.unused()));
                    match attempts {
                        1 => Ok(Some(3)),
                        2 => Ok(Some(2)),
                        _ => Err(error),
                    }
                })
                .map(|renewed| assert!(renewed))
        }),
        Err(error)
    );
    assert_eq!(attempts, 3);
    assert_eq!(published, [(0, 0, 0), (3, 3, 0), (2, 2, 0)]);
    assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
    lease.settle(|_| panic!("refused renewal already detached its grant"));
}

#[test]
fn returned_borrowed_permits_remain_in_their_original_grant() {
    let lease = PermitLease::new();
    grant(&lease, u64::MAX);
    assert_eq!(
        lease.take_up_to::<()>(u64::MAX, || panic!("grant is present")),
        Ok(u64::MAX)
    );
    lease.give_back(u64::MAX - 2);
    lease
        .tick::<()>(|| panic!("returned permits suffice"))
        .unwrap();
    lease.settle(|prior| {
        assert_eq!(
            (prior.granted(), prior.used(), prior.unused()),
            (u64::MAX, 3, u64::MAX - 3)
        );
    });
}

#[test]
fn renewal_publishes_the_old_grant_before_installing_a_replacement() {
    let lease = PermitLease::new();
    grant(&lease, 2);
    lease
        .take::<()>(2, || panic!("existing grant suffices"))
        .unwrap();
    assert_eq!(
        lease.renew::<()>(|prior| {
            assert_eq!((prior.granted(), prior.used(), prior.unused()), (2, 2, 0));
            assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
            Ok(None)
        }),
        Ok(false)
    );
    assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
    assert_eq!(
        lease.renew::<()>(|prior| {
            assert_eq!(prior.granted(), 0);
            Ok(Some(7))
        }),
        Ok(true)
    );
    assert_eq!((lease.granted.get(), lease.remaining.get()), (7, 7));
}

#[test]
fn a_panicking_publication_does_not_restore_or_republish_a_grant() {
    let lease = PermitLease::new();
    grant(&lease, 3);
    lease.tick::<()>(|| panic!("grant is present")).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| lease.settle(|prior| {
            assert_eq!((prior.granted(), prior.used(), prior.unused()), (3, 1, 2));
            assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
            panic!("publication unwinds");
        })))
        .is_err()
    );
    lease.settle(|_| panic!("the detached transfer is not published twice"));
}

#[test]
fn a_panicking_renewal_detaches_the_exhausted_grant_without_installing_one() {
    let lease = PermitLease::new();
    grant(&lease, 3);
    lease
        .take::<()>(3, || panic!("existing grant suffices"))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = lease.renew::<()>(|prior| {
                assert_eq!((prior.granted(), prior.used(), prior.unused()), (3, 3, 0));
                assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
                panic!("source transition unwinds");
            });
        }))
        .is_err()
    );
    assert_eq!((lease.granted.get(), lease.remaining.get()), (0, 0));
    lease.settle(|_| panic!("no replacement was installed"));
}

#[test]
fn default_and_new_own_identical_empty_cells() {
    let explicit = PermitLease::new();
    let default = PermitLease::default();
    assert_eq!((explicit.granted.get(), explicit.remaining.get()), (0, 0));
    assert_eq!((default.granted.get(), default.remaining.get()), (0, 0));
}

#[test]
fn the_local_grant_retains_its_two_cell_header_and_send_contract() {
    fn assert_send<T: Send>() {}

    assert_eq!(
        std::mem::size_of::<PermitLease>(),
        2 * std::mem::size_of::<Cell<u64>>()
    );
    assert_eq!(
        std::mem::align_of::<PermitLease>(),
        std::mem::align_of::<Cell<u64>>()
    );
    assert_send::<PermitLease>();
}

#[test]
fn a_panicking_counted_refund_keeps_the_detached_limit_and_charged_prefix() {
    let mut charged = 3;
    let mut limit = 5;
    assert!(
        catch_unwind(AssertUnwindSafe(|| settle(
            &mut Counted {
                charged: &mut charged,
                admitted: &mut limit,
            },
            |prior| {
                assert_eq!((prior.granted(), prior.used(), prior.unused()), (5, 3, 2));
                panic!("quota refund unwinds");
            }
        )))
        .is_err()
    );
    assert_eq!((charged, limit), (3, 3));
}
