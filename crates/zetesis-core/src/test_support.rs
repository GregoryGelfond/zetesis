//! Helpers the crate's unit tests share across its modules.

use std::convert::Infallible;

use crate::ValueLimits;
use crate::catalog::Limits;

/// A permission check that always permits.
pub(crate) const PERMIT: fn() -> Result<(), Infallible> = || Ok(());

/// A permission check that always permits, and whose refusal would carry no
/// reason.
pub(crate) const PERMIT_WITH_UNIT_ERROR: fn() -> Result<(), ()> = || Ok(());

/// Catalog limits that bound nothing.
pub(crate) fn unlimited() -> Limits {
    Limits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}

/// Value limits that bound nothing.
pub(crate) fn unlimited_values() -> ValueLimits {
    ValueLimits {
        max_nodes: usize::MAX,
        max_depth: usize::MAX,
        max_bytes: usize::MAX,
    }
}
