use std::convert::Infallible;
use std::num::NonZeroUsize;

use zetesis_ferraris::{Interpretation, Theory};

use super::{BatchError, BatchLimits, BatchVerdict, OriginalCheck};
use crate::ferraris::fixtures::choice_beside_a_loop;
use crate::ferraris::region_proposals::Produced;
use crate::{Cancellation, Incomplete, Limits, PhaseMeasurement, StableModels};

/// A slate that filled and then stopped, as one does when a producer's step
/// fails after the filling emission: its candidates are checked and
/// committed, and the stop is reported by the next call, never exhaustion.
/// The join is handed over directly, since no schedule of real producers
/// reaches that interleaving on demand.
#[test]
fn a_full_slate_beside_a_stop_commits_its_candidates_before_the_stop() {
    let theory = choice_beside_a_loop();
    let mut search = StableModels::with_region_producers(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let limits = BatchLimits {
        max_candidates: NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 4096,
    };
    // Two of the theory's answer sets fill the slate, admitted as the proposal
    // step admits a joined slate's output, after building the original index
    // its producers read and its completion borrows.
    let available = search.prepare_pending(limits).unwrap();
    search.batch.original_checked = true;
    search.walk_index().unwrap();
    for atoms in [[1], [2]] {
        search
            .batch
            .pending
            .push(Interpretation::new(&theory, atoms).unwrap());
    }
    let produced = Produced {
        exhausted: false,
        stopped: Some(Incomplete::WorkLimit),
        original_validation: PhaseMeasurement::default(),
    };
    search.admit_produced(&produced, available, OriginalCheck::Independent);
    let residual = |_: &Theory, candidates: &[Interpretation]| {
        Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()])
    };
    assert_eq!(search.next_batch(limits, residual).unwrap().len(), 2);
    assert!(matches!(
        search.next_batch(limits, residual),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    assert!(!search.exhausted());
}
