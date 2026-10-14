//! Helpers the crate's unit tests share.

use zetesis_ferraris::Theory;

use crate::{Cancellation, PreparedReduct, ReductPreparationLimits};

/// The prepared reduct of `theory` under the default preparation limits,
/// which it must fit.
pub(crate) fn prepare(theory: &Theory) -> PreparedReduct {
    PreparedReduct::prepare(
        theory,
        ReductPreparationLimits::default(),
        &Cancellation::default(),
    )
    .result
    .unwrap()
}
