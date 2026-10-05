//! Batched work reservation stops a narrowing at the read per-read charging
//! stops it at, records the same work, and returns every unspent permit.

use zetesis_cpu::{Cancellation, Stop};

use crate::{AdmissionLimits, Narrower, NarrowingQuota, Node, Region, Theory};

/// A held root forces a chain of implications `a0 → a1 → … → a{n-1}`, so the
/// closure reads every node and parent: a predictable number of charges.
fn implication_chain(atoms: usize) -> Theory {
    let mut nodes: Vec<Node> = (0..atoms).map(Node::Atom).collect();
    let mut roots = vec![0];
    for atom in 0..atoms - 1 {
        roots.push(nodes.len());
        nodes.push(Node::Implies(atom, atom + 1));
    }
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}

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

fn per_read(theory: &Theory, limit: u64) -> Run {
    let narrower = Narrower::new(theory);
    let mut knowledge = narrower.knowledge();
    let mut region = Region::all_open(theory.atom_count());
    let mut spent = 0;
    let attempt = narrower.narrow_known_metered(
        theory,
        None,
        &mut region,
        &mut knowledge,
        &Cancellation::default(),
        || {
            if spent == limit {
                return Err(Stop::WorkLimit);
            }
            spent += 1;
            Ok(())
        },
    );
    let outcome = attempt
        .result
        .map(|narrowing| matches!(narrowing, crate::Narrowing::Refuted));
    (outcome, attempt.statistics.work, region)
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
        theory,
        None,
        &mut region,
        &mut knowledge,
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
