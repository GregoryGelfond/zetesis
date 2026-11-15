//! Private adapter to the one root-image traversal.

use super::{Adjacency, Budget, BudgetError, Narrowing, Region, Storage};
use crate::regions::arithmetic::images::kernel::{self, Counts};

pub(super) fn close<'a>(
    groups: &(impl Iterator<Item = &'a [u64]> + Clone),
    pairs: Option<&Adjacency>,
    region: &mut Region,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_>,
) -> Result<Narrowing, BudgetError> {
    kernel::close(
        groups,
        pairs,
        region,
        storage,
        budget,
        &mut Counts::default(),
    )
}
