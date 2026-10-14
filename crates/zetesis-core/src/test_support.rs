//! Helpers the crate's unit tests share across its modules.

use std::cmp::Ordering;
use std::convert::Infallible;

use crate::ValueLimits;
use crate::catalog::{Limits, TermRef};

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

/// The order of two terms: the ASP term order when `asp`, else the reference
/// order, consulting `before` as the comparison proceeds.
pub(crate) fn compare_terms<E>(
    left: TermRef<'_>,
    right: TermRef<'_>,
    asp: bool,
    before: impl FnMut() -> Result<(), E>,
) -> Result<Ordering, E> {
    if asp {
        left.compare_terms_with(right, before)
    } else {
        left.compare_ref_with(right, before)
    }
}
