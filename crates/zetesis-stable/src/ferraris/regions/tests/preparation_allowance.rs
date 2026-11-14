//! The root preparation's work allowance grows with its theory.

use super::super::{open_attempt, preparation_work};
use super::budget;
use crate::Cancellation;
use zetesis_ferraris::{LearningLimits, Node, PreparationLimits, PreparationStatus, Theory};

/// `atoms` independent choices and, for the first `pairs` pairs of them in
/// lexicographic order, the constraint that no answer set holds both.
fn exclusions(atoms: usize, pairs: usize) -> Theory {
    let falsum = atoms;
    let mut nodes: Vec<Node> = (0..atoms).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let mut roots = Vec::new();
    for atom in 0..atoms {
        let absent = nodes.len();
        nodes.push(Node::implies(atom, falsum));
        roots.push(nodes.len());
        nodes.push(Node::or_pair([atom, absent]));
    }
    let mut written = 0;
    'pairs: for first in 0..atoms {
        for second in first + 1..atoms {
            if written == pairs {
                break 'pairs;
            }
            let both = nodes.len();
            nodes.push(Node::and_pair([first, second]));
            roots.push(nodes.len());
            nodes.push(Node::implies(both, falsum));
            written += 1;
        }
    }
    assert_eq!(written, pairs);
    zetesis_theory_support::theories::theory(atoms, nodes, roots)
}

/// The allowance with no work limit.
fn unlimited(nodes: usize, occurrences: usize) -> u64 {
    preparation_work(nodes, occurrences, u64::MAX)
}

#[test]
fn a_small_theory_is_allowed_the_default_work() {
    assert_eq!(unlimited(0, 0), PreparationLimits::DEFAULT_WORK);
    // 390,625 nodes and occurrences are the most whose tables fit in it.
    assert_eq!(unlimited(390_625, 0), 100_000_000);
    assert_eq!(unlimited(130_000, 260_625), 100_000_000);
}

#[test]
fn a_larger_theory_is_allowed_a_pattern_table_per_read() {
    assert_eq!(unlimited(390_626, 0), 100_000_256);
    assert_eq!(unlimited(0, 390_626), 100_000_256);
    assert_eq!(unlimited(1_000_000, 2_000_000), 768_000_000);
}

#[test]
fn a_product_past_the_counter_is_bounded_by_the_limit_alone() {
    // The product saturates, and half of the largest limit is below it.
    assert_eq!(unlimited(usize::MAX, usize::MAX), u64::MAX / 2);
}

#[test]
fn a_work_limit_grants_at_most_half_of_what_it_leaves() {
    assert_eq!(
        preparation_work(1_000_000, 2_000_000, 1_000_000_000),
        500_000_000
    );
    assert_eq!(
        preparation_work(1_000_000, 2_000_000, 1_536_000_000),
        768_000_000
    );
}

#[test]
fn a_tight_work_limit_still_grants_the_default_work() {
    assert_eq!(
        preparation_work(1_000_000, 2_000_000, 150_000_000),
        100_000_000
    );
    assert_eq!(preparation_work(1_000_000, 2_000_000, 0), 100_000_000);
}

/// Seven hundred choices under 175,000 pairwise exclusions. Reading them
/// against their groups' patterns takes more than the default allowance and
/// less than the theory's own: the least such theory worth its time, since
/// the work must exceed the default for the two to differ. Opening it runs
/// over 100,000,000 metered operations, about three seconds in a test build.
fn many_exclusions() -> Theory {
    exclusions(700, 175_000)
}

#[test]
fn a_large_theory_completes_a_preparation_the_default_work_declines() {
    let theory = many_exclusions();
    let allowance = unlimited(theory.nodes().len(), theory.parts().occurrences());
    let cancellation = Cancellation::default();
    let mut opening = budget(&cancellation, u64::MAX);
    let statistics =
        open_attempt(&theory, u64::MAX, LearningLimits::DEFAULT, &mut opening).statistics;
    assert_eq!(statistics.parity.status, PreparationStatus::Prepared);
    assert_eq!(statistics.cardinality.status, PreparationStatus::Prepared);
    let spent = statistics.parity.preparation_work
        + statistics.cardinality.preparation_work
        + statistics.images.preparation_work;
    assert!(
        statistics.parity.preparation_work > PreparationLimits::DEFAULT_WORK,
        "the default allowance covers this recognition"
    );
    assert!(spent <= allowance, "spent {spent} of {allowance}");
}

#[test]
fn a_work_limit_declines_the_preparation_it_has_no_room_for() {
    // The limit leaves less than twice the default, so the preparation is
    // allowed the default and no more: it is declined, where its theory's
    // own allowance would have completed it, after more work than half the
    // limit would have allowed. A refusal leaves less than the refused
    // operation's size unspent, so the work stops short of the allowance by
    // a few operations. The opening completes within the limit all the same.
    let theory = many_exclusions();
    let cancellation = Cancellation::default();
    let limit = 180_000_000;
    let mut opening = budget(&cancellation, limit);
    let attempt = open_attempt(&theory, u64::MAX, LearningLimits::DEFAULT, &mut opening);
    assert!(attempt.result.is_ok());
    assert_eq!(
        attempt.statistics.parity.status,
        PreparationStatus::WorkDeclined
    );
    let spent = attempt.statistics.parity.preparation_work;
    assert!(
        limit / 2 < spent && spent <= PreparationLimits::DEFAULT_WORK,
        "spent {spent}"
    );
}
