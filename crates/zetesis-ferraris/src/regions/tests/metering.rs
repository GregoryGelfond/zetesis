//! Batched work reservation stops a narrowing at the read per-read charging
//! stops it at, records the same work, and returns every unspent permit; a
//! fixed ceiling stops it at the same read.

use zetesis_algebra::{Cancellation, Stop};

use crate::regions::{NARROWING_BATCH, Work};
use crate::{Narrower, NarrowingQuota, Region, RegionLimits, Theory};

use super::implication_chain;

/// A finite allowance granted in batches of at most `batch`.
struct Allowance {
    remaining: u64,
    batch: u64,
    granted: u64,
    refunded: u64,
}

impl NarrowingQuota for Allowance {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let grant = wanted.min(self.batch).min(self.remaining);
        if grant == 0 {
            return Err(Stop::WorkLimit);
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

/// Outcome, recorded work and region of one narrowing.
type Run = (Result<bool, Stop>, u64, Region);

/// Per-read charging of the same allowance: one permit per reservation.
fn per_read(theory: &Theory, limit: u64) -> Run {
    reserved(theory, limit, 1).0
}

fn reserved(theory: &Theory, limit: u64, batch: u64) -> (Run, Allowance) {
    let narrower = Narrower::new(theory);
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    let mut allowance = Allowance {
        remaining: limit,
        batch,
        granted: 0,
        refunded: 0,
    };
    let attempt = narrower.narrow_known_reserved(
        crate::OriginalSubject::new(theory, None),
        &mut region,
        &mut knowledge,
        &mut crate::NarrowingScratch::default(),
        &Cancellation::default(),
        &mut allowance,
    );
    let outcome = attempt
        .result
        .map(|narrowing| matches!(narrowing, crate::Narrowing::Refuted));
    ((outcome, attempt.statistics.work, region), allowance)
}

#[test]
fn batched_reservation_stops_at_the_per_read_point() {
    let theory = implication_chain(40);
    let needed = per_read(&theory, u64::MAX).1;
    assert!(needed > 64, "the chain needs many charged reads: {needed}");
    for limit in [0, 1, 2, needed / 3, needed - 1, needed, needed + 7] {
        let (outcome, work, region) = per_read(&theory, limit);
        for batch in [1, 2, 7, 64, 256] {
            let ((batched, batched_work, batched_region), _) = reserved(&theory, limit, batch);
            assert_eq!(batched, outcome, "limit {limit}, batch {batch}");
            assert_eq!(batched_work, work, "limit {limit}, batch {batch}");
            assert_eq!(batched_region, region, "limit {limit}, batch {batch}");
        }
    }
}

#[test]
fn unspent_permits_are_refunded() {
    let theory = implication_chain(40);
    for limit in [0, 5, 1_000] {
        for batch in [1, 64, 256] {
            let ((_, work, _), allowance) = reserved(&theory, limit, batch);
            assert_eq!(
                allowance.granted - allowance.refunded,
                work,
                "limit {limit}, batch {batch}"
            );
        }
    }
}

/// A quota granting a fixed sequence of batches, then refusing, which
/// records every request and every refund.
struct Grants {
    batches: Vec<u64>,
    requests: Vec<u64>,
    refunded: u64,
}

impl Grants {
    fn new(grants: &[u64]) -> Self {
        Self {
            batches: grants.to_vec(),
            requests: Vec::new(),
            refunded: 0,
        }
    }
}

impl NarrowingQuota for Grants {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        self.requests.push(wanted);
        if self.batches.is_empty() {
            return Err(Stop::WorkLimit);
        }
        Ok(self.batches.remove(0))
    }
    fn refund(&mut self, unspent: u64) {
        self.refunded += unspent;
    }
}

#[test]
fn work_spends_each_grant_before_reserving_again() {
    let mut quota = Grants::new(&[3, 2]);
    let mut work = Work::reserved(&mut quota);
    for _ in 0..5 {
        work.tick().unwrap();
    }
    assert_eq!(work.tick(), Err(Stop::WorkLimit));
    assert_eq!(work.spent(), 5);
    assert_eq!(quota.requests, [NARROWING_BATCH; 3]);
}

#[test]
fn refund_returns_exactly_the_unspent_permits() {
    let mut quota = Grants::new(&[10, 4]);
    let mut work = Work::reserved(&mut quota);
    for _ in 0..3 {
        work.tick().unwrap();
    }
    work.refund();
    work.tick().unwrap();
    work.refund();
    assert_eq!(quota.refunded, 7 + 3);
}

#[test]
fn refund_keeps_the_charged_reads() {
    let mut quota = Grants::new(&[10, 4]);
    let mut work = Work::reserved(&mut quota);
    for _ in 0..3 {
        work.tick().unwrap();
    }
    work.refund();
    assert_eq!(work.spent(), 3);
    work.tick().unwrap();
    work.refund();
    assert_eq!(work.spent(), 4);
}

#[test]
fn a_fixed_ceiling_admits_exactly_its_reads() {
    for ceiling in [0, 1, 5] {
        let mut work = Work::new(ceiling);
        for _ in 0..ceiling {
            work.tick().unwrap();
        }
        assert_eq!(work.tick(), Err(Stop::WorkLimit), "ceiling {ceiling}");
        assert_eq!(work.spent(), ceiling, "ceiling {ceiling}");
    }
}

#[test]
fn a_fixed_ceiling_stops_at_the_per_read_point() {
    let theory = implication_chain(40);
    let needed = per_read(&theory, u64::MAX).1;
    for limit in [0, 1, 2, needed / 3, needed - 1, needed, needed + 7] {
        let (outcome, work, region) = per_read(&theory, limit);
        let narrower = Narrower::new(&theory);
        let mut knowledge = narrower.knowledge();
        let mut ceiling_region = Region::all_open(theory.atom_count());
        let ceiling = narrower.narrow_known(
            crate::OriginalSubject::new(&theory, None),
            &mut ceiling_region,
            &mut knowledge,
            &mut crate::NarrowingScratch::default(),
            RegionLimits { max_work: limit },
            &Cancellation::default(),
        );
        let ceiling_outcome = ceiling
            .as_ref()
            .map(|(narrowing, _)| matches!(narrowing, crate::Narrowing::Refuted))
            .map_err(|stop| *stop);
        assert_eq!(ceiling_outcome, outcome, "limit {limit}");
        if let Ok((_, statistics)) = &ceiling {
            assert_eq!(statistics.work, work, "limit {limit}");
        }
        assert_eq!(ceiling_region, region, "limit {limit}");
    }
}
