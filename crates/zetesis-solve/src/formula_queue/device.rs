//! The device side of the queue's two-phase slates. A device route checks a
//! slate in submissions taken in prefix order. Each submission is prepared
//! (admitted against the route's limits and planned, with no device effect),
//! then checked, alone or while the host proposes the next slate, and then
//! decoded into one ordered verdict per candidate it admitted.
//!
//! A prepared submission holds its device until it is checked, so no other
//! submission can be prepared, and no operation can run on the device, in
//! between: the plan it read at preparation is the plan its check uses.

use std::time::Duration;

pub(crate) use super::regional::CheckedRows;
use zetesis_algebra::Cancellation;
use zetesis_backend::BatchComputeTime;
use zetesis_ferraris::{SlateId, SlateRef, Theory};

use crate::formula_execution::Failure;

/// A device route that checks the candidates of a stream's slates.
pub(crate) trait SlateDevice {
    /// Only the general formula primitive returns narrowed residual roots.
    const REGIONAL: bool = false;

    /// Record the accounted envelope of a regional submission or joined completion.
    fn record_slate_bytes(&mut self, _total: u64) {}

    fn retained_batch_bytes(&self) -> Result<u64, Failure> {
        Ok(0)
    }

    /// A submission admitted and planned, holding the device until it is checked.
    type Prepared<'d>: PreparedSubmission<Checked = Self::Checked>
    where
        Self: 'd;
    /// The device's receipt for one checked submission, or its failure.
    type Checked;

    /// Admit and plan the submission at `offset` of `slate`. It admits at
    /// least one candidate, or fails.
    ///
    /// # Errors
    /// A stop of the control or a device failure, with nothing submitted.
    fn prepare<'d>(
        &'d mut self,
        slate: SlateRef<'_>,
        offset: usize,
        max_bytes: u64,
        cancellation: &Cancellation,
    ) -> Result<Self::Prepared<'d>, Failure>;

    /// Record one checked submission's receipts and decode its checks: one
    /// ordered verdict per candidate it admitted at `offset` of `slate`, out of
    /// the `offered` left there.
    ///
    /// # Errors
    /// The submission's own failure, a receipt that belongs to another theory,
    /// slate or offset, or a count outside the candidates offered.
    fn decode(
        &mut self,
        checked: Self::Checked,
        theory: &Theory,
        slate: SlateId,
        offset: usize,
        offered: usize,
        rows: &mut CheckedRows,
    ) -> Result<Decoded, Failure>;
}

/// One prepared submission's checks.
pub(crate) trait PreparedSubmission {
    /// The device's receipt for the checked submission, or its failure.
    type Checked;

    /// The candidates this submission admitted.
    fn count(&self) -> usize;

    /// Accounted by this plan, including the worst case of new decoded roots.
    fn accounted_bytes(&self) -> u64 {
        0
    }

    /// Submit and finish the submission with nothing run beside it.
    fn check(self, cancellation: &Cancellation) -> Self::Checked;

    /// Submit the submission, run `during` while the device checks it, then
    /// finish it. When the control fired during `during`, the finish observes
    /// completion without polling the control, so the device stays reusable;
    /// `during`'s result is returned beside the receipt.
    fn check_overlapping<R>(
        self,
        cancellation: &Cancellation,
        during: impl FnOnce() -> R,
    ) -> (Self::Checked, Option<R>);
}

/// An authenticated submission's checks are appended in order. These timing
/// observations retain the compute interval the device
/// reported with it, which belongs to the submission before it; and, for a
/// submission checked beside the host's production, the finish's wait from
/// the end of that production to the submission's observed completion.
pub(crate) struct Decoded {
    pub(crate) compute: Option<BatchComputeTime>,
    pub(crate) finish_wait: Option<Duration>,
}
