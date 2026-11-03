use super::*;

fn completed() -> CardinalityKnowledge {
    let system = guarded();
    let mut known = system.knowledge();
    assert_eq!(
        run(&system, &mut region(3, 1, 0), &mut known).result,
        Ok(SubjectStep::Unchanged)
    );
    known
}

#[test]
fn metered_copies_reuse_eligible_destination_capacity() {
    let source = completed();
    let mut target = completed();
    target.sums.reserve(10);
    let sums = target.sums.as_ptr();
    let sum_capacity = target.sums.capacity();
    let masks = target.snapshot.old_masks().0.as_ptr();
    let snapshot_bytes = target.snapshot.allocated_bytes();
    let mut destination = Some(target);
    let mut work = Meter::local(u64::MAX);
    source
        .copy_reserved(&mut destination, &Cancellation::default(), &mut work)
        .unwrap();
    let copied = destination.unwrap();
    assert_eq!(copied.sums, source.sums);
    assert_eq!(copied.snapshot.old_masks(), source.snapshot.old_masks());
    assert_eq!(copied.sums.as_ptr(), sums);
    assert_eq!(copied.sums.capacity(), sum_capacity);
    assert_eq!(copied.snapshot.old_masks().0.as_ptr(), masks);
    assert_eq!(copied.snapshot.allocated_bytes(), snapshot_bytes);
    assert_ne!(copied.sums.as_ptr(), source.sums.as_ptr());
    assert_ne!(
        copied.snapshot.old_masks().0.as_ptr(),
        source.snapshot.old_masks().0.as_ptr()
    );
}

#[test]
fn every_refused_copy_discards_the_destination() {
    let source = completed();
    let sums = source.sums.clone();
    let masks = source.snapshot.old_masks();
    let mut full = Meter::local(u64::MAX);
    source
        .copy_reserved(&mut None, &Cancellation::default(), &mut full)
        .unwrap();
    let total = full.charged();
    for allowance in 0..total {
        let mut destination = None;
        let mut account = Account {
            remaining: allowance,
            calls: 0,
            refunds: 0,
        };
        let mut work = Meter::reserved(&mut account);
        assert_eq!(
            source.copy_reserved(&mut destination, &Cancellation::default(), &mut work),
            Err(Failure::Refused(Refusal::Ticket(41)))
        );
        let charged = work.charged();
        // The enclosing copy attempt, not the copy helper, owns settlement.
        work.refund();
        assert_eq!(account.refunds, 1);
        assert_eq!(account.remaining + charged, allowance);
        assert!(destination.is_none());
        assert_eq!(source.sums, sums);
        assert_eq!(source.snapshot.old_masks(), masks);
    }
}

#[test]
fn cancelled_copy_does_not_read_lineage_payloads() {
    let source = completed();
    let mut destination = Some(completed());
    let cancel = Cancellation::default();
    cancel.cancel();
    let mut account = Account {
        remaining: u64::MAX,
        calls: 0,
        refunds: 0,
    };
    let mut work = Meter::reserved(&mut account);
    assert_eq!(
        source.copy_reserved(&mut destination, &cancel, &mut work),
        Err(Failure::Stopped(Stop::Cancelled))
    );
    assert_eq!(work.charged(), 0);
    assert_eq!(account.calls, 0);
    assert_eq!(account.refunds, 0);
    assert!(destination.is_none());
    assert!(source.snapshot.initialized());
}

#[test]
fn copied_siblings_do_not_share_progress() {
    let system = guarded();
    let mut parent = system.knowledge();
    assert_eq!(
        run(&system, &mut Region::all_open(3), &mut parent).result,
        Ok(SubjectStep::Unchanged)
    );
    let mut left = None;
    let mut right = None;
    for destination in [&mut left, &mut right] {
        parent
            .copy_reserved(
                destination,
                &Cancellation::default(),
                &mut Meter::local(u64::MAX),
            )
            .unwrap();
    }
    let (mut left, mut right) = (left.unwrap(), right.unwrap());
    let mut left_region = region(3, 4, 0);
    let mut right_region = region(3, 0, 4);
    assert_eq!(
        run(&system, &mut left_region, &mut left).result,
        Ok(SubjectStep::Unchanged)
    );
    assert_eq!(
        run(&system, &mut right_region, &mut right).result,
        Ok(SubjectStep::Unchanged)
    );
    assert_eq!(left.sums, fresh(&system, &left_region));
    assert_eq!(right.sums, fresh(&system, &right_region));
    assert_eq!(parent.sums, vec![PairSums { lower: 1, upper: 2 }; 4]);
    assert_ne!(left.snapshot.old_masks(), right.snapshot.old_masks());
}
