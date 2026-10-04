//! Generated-root history is scoped membership metadata, not another value owner.

use crate::ProgramSite;
use zetesis_core::catalog::{
    AssignmentError, AssignmentFailure, CatalogRead, Error, TermKey, TermSet,
};

use super::StorageLease;
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

pub(super) struct Generated {
    pub(super) values: TermSet,
    pub(super) lease: StorageLease,
}

impl Generated {
    pub(super) fn new(read: CatalogRead<'_>, lease: StorageLease) -> Self {
        Self {
            values: read.term_set(),
            lease,
        }
    }
    pub(super) fn bytes(&self) -> usize {
        size_of::<Self>() + self.values.retained_bytes() - size_of::<TermSet>()
    }
    pub(super) fn select(
        &mut self,
        key: &TermKey,
        max_bytes: usize,
        limits: &FormulaLimits,
        mut before: impl FnMut() -> Result<(), FormulaFailure>,
        location: ProgramSite,
    ) -> Result<(), AssignmentFailure<FormulaFailure>> {
        if self.values.contains_with(key, &mut before)? {
            return Ok(());
        }
        crate::formula::ceiling(
            FormulaResource::GeneratedValues,
            self.values.len() as u128 + 1,
            limits.max_generated_values as u128,
            location,
        )
        .map_err(AssignmentFailure::Stopped)?;
        let extra = size_of::<Self>() - size_of::<TermSet>();
        self.values
            .insert_with(key, max_bytes.saturating_sub(extra), before)
            .map(|_| ())
            .map_err(|error| match error {
                AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required,
                    limit,
                })) => AssignmentFailure::Assignment(AssignmentError::Storage(Error::Storage {
                    required: required + extra as u128,
                    limit: limit + extra,
                })),
                error => error,
            })
    }
}
