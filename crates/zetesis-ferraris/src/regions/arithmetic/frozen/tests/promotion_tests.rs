//! Exact atomic boundaries for retaining one query's completed probe tables.
use super::super::super::budget::BudgetError;
use super::super::{ProbedGroup, probes::Table, retain_refutations};
use super::*;

const BEFORE: [[u64; 4]; 3] = [[1; 4]; 3];
const AFTER: [[u64; 4]; 3] = [[3, 5, 9, 17], [33, 65, 129, 257], [513, 1025, 2049, 4097]];
const PENDING: [usize; 3] = [2, 0, 1];

struct Attempt {
    result: Result<(), BudgetError<Stop>>,
    known: [[u64; 4]; 3],
    work: u64,
    charged: u64,
}

/// Read-only source tables contain each previous known bit plus additional
/// completed exclusions. Different words expose a partial-copy error.
fn retained(
    pending: &[usize],
    allowance: u64,
    optional_limit: u64,
    cancellation: &Cancellation,
) -> Attempt {
    let support = Support {
        atoms: std::array::from_fn(|index| index),
        len: MAX_SUPPORT,
    };
    let tables = AFTER.map(|words| ProbedGroup {
        support,
        forbidden: Table::new(words),
    });
    let mut known = BEFORE.map(Table::new);
    let mut quota = Allowance {
        remaining: allowance,
        failure: Stop::WorkLimit,
    };
    let (result, work) = {
        let mut meter = Meter::reserved(&mut quota);
        let mut budget = Budget::new(&mut meter, optional_limit, usize::MAX, cancellation);
        let result = retain_refutations(pending, &tables, &mut known, &mut budget);
        budget.finish();
        (result, budget.work())
    };
    Attempt {
        result,
        known: known.map(|table| *table.storage()),
        work,
        charged: allowance - quota.remaining,
    }
}

fn expected_prefix(completed: usize) -> [[u64; 4]; 3] {
    let mut expected = BEFORE;
    for &index in &PENDING[..completed] {
        expected[index] = AFTER[index];
    }
    expected
}

#[test]
fn refused_promotion_retains_only_fully_granted_groups() {
    for allowance in 0..12 {
        let Attempt {
            result,
            known,
            work,
            charged,
        } = retained(&PENDING, allowance, u64::MAX, &Cancellation::default());
        assert_eq!(
            result,
            Err(BudgetError::Refused(Stop::WorkLimit)),
            "{allowance}"
        );
        let completed = usize::try_from(allowance / 4).unwrap();
        assert_eq!(known, expected_prefix(completed), "{allowance}");
        assert_eq!(
            (work, charged),
            (4 * completed as u64, 4 * completed as u64),
            "{allowance}"
        );
    }
}

#[test]
fn exact_promotion_allowance_copies_each_word_once() {
    let Attempt {
        result,
        known,
        work,
        charged,
    } = retained(&PENDING, 12, u64::MAX, &Cancellation::default());
    assert_eq!(result, Ok(()));
    assert_eq!(known, AFTER);
    assert_eq!((work, charged), (12, 12));
}

#[test]
fn optional_promotion_refusal_preserves_the_performed_prefix() {
    for limit in 0..12 {
        let Attempt {
            result,
            known,
            work,
            charged,
        } = retained(&PENDING, u64::MAX, limit, &Cancellation::default());
        assert_eq!(result, Err(BudgetError::WorkDeclined), "{limit}");
        let completed = usize::try_from(limit / 4).unwrap();
        assert_eq!(known, expected_prefix(completed), "{limit}");
        assert_eq!(
            (work, charged),
            (4 * completed as u64, 4 * completed as u64),
            "{limit}"
        );
    }
}

#[test]
fn observed_cancellation_precedes_promotion_refusal() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    // The existing Budget observes cancellation before reporting either
    // optional exhaustion or a global refusal. Neither first group executes.
    for (allowance, limit) in [(0, u64::MAX), (u64::MAX, 0)] {
        let Attempt {
            result,
            known,
            work,
            charged,
        } = retained(&PENDING, allowance, limit, &cancellation);
        assert_eq!(result, Err(BudgetError::Stopped(Stop::Cancelled)));
        assert_eq!(known, BEFORE);
        assert_eq!((work, charged), (0, 0));
    }
}

#[test]
fn empty_pending_work_does_not_touch_known_tables() {
    let Attempt {
        result,
        known,
        work,
        charged,
    } = retained(&[], 0, 0, &Cancellation::default());
    assert_eq!(result, Ok(()));
    assert_eq!(known, BEFORE);
    assert_eq!((work, charged), (0, 0));
}
