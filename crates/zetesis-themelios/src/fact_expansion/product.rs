//! One alternative's compact Cartesian product and complete copy preflight.

use themelios_program::program::Atom;
use themelios_program::term::Term as SourceTerm;
use zetesis_core::{Predicate, Template, Term, Value};

use super::{
    columns::{self, Column},
    reserve_exact,
};
use crate::expansion::Budget;
use crate::{AdmissionFailure, ExpansionFailure, ExpansionResource, ProgramSite};

#[derive(Debug, Default)]
pub(super) struct Product {
    predicate: Option<Predicate>,
    columns: Vec<Column>,
    rows: usize,
    /// Column shells kept from an earlier use of this product.
    spare: Vec<Column>,
}

impl Product {
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.predicate().name().len()
            + self.columns.capacity() * size_of::<Column>()
            + self
                .columns
                .iter()
                .map(Column::retained_bytes)
                .sum::<usize>()
    }

    /// Check one alternative into this product, reusing its storage; false,
    /// with nothing kept, when the alternative has no row.
    pub(super) fn prepare(
        &mut self,
        atom: &Atom,
        arguments: &[SourceTerm],
        sizes: &mut Vec<(u128, u128)>,
        logical_bytes: &mut u128,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<bool, ExpansionFailure> {
        self.recycle();
        sizes.clear();
        reserve_exact(sizes, arguments.len(), budget, location)?;
        for term in arguments {
            sizes.push(columns::size(term, budget, location)?);
        }
        // Check every argument, including siblings of an empty interval.
        let count = sizes
            .iter()
            .fold(1_u128, |count, (size, _)| count.saturating_mul(*size));
        budget.charge(ExpansionResource::Templates, count, location)?;
        let intermediate = sizes
            .iter()
            .fold(0_u128, |sum, (size, _)| sum.saturating_add(*size));
        budget.charge(
            ExpansionResource::Values,
            intermediate.saturating_add(count.saturating_mul(arguments.len() as u128)),
            location,
        )?;
        if count == 0 {
            return Ok(false);
        }
        budget.charge(
            ExpansionResource::ScalarBytes,
            (atom.name.as_str().len() as u128).saturating_mul(count),
            location,
        )?;
        logical_family(
            atom,
            sizes,
            count,
            intermediate,
            logical_bytes,
            budget,
            location,
        )?;
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction { error, location })?;
        reserve_exact(&mut self.columns, arguments.len(), budget, location)?;
        let mut copied_bytes = 0_u128;
        for (term, (width, _)) in arguments.iter().zip(sizes.iter()) {
            let mut column = self.spare.pop().unwrap_or_default();
            let payload = column.prepare(term, budget, location)?;
            copied_bytes = copied_bytes.saturating_add((count / width).saturating_mul(payload));
            self.columns.push(column);
        }
        self.predicate = Some(predicate);
        self.rows = usize::try_from(count).expect("Templates charge has a usize ceiling");
        self.charge_copies(copied_bytes, budget, location)?;
        Ok(true)
    }

    /// Drop this product's values and keep its storage for the next use.
    pub(super) fn recycle(&mut self) {
        self.predicate = None;
        self.rows = 0;
        for mut column in self.columns.drain(..).rev() {
            column.recycle();
            self.spare.push(column);
        }
    }

    pub(super) fn len(&self) -> usize {
        self.rows
    }

    pub(super) fn predicate(&self) -> &Predicate {
        self.predicate
            .as_ref()
            .expect("a kept product has its predicate")
    }

    pub(super) fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Every scalar occurrence is selected count / column.width times. When
    /// that complete traffic fits, no product traversal is needed to admit it.
    /// The refusal path uses the same positions as emission to retain the old
    /// accepted prefix and first offending per-argument charge.
    fn charge_copies(
        &mut self,
        bytes: u128,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(), ExpansionFailure> {
        if bytes <= budget.remaining_scalar_bytes() {
            return budget.charge(ExpansionResource::ScalarBytes, bytes, location);
        }
        for _ in 0..self.rows {
            budget.poll(location)?;
            for column in &self.columns {
                budget.charge(
                    ExpansionResource::ScalarBytes,
                    column.value_bytes(),
                    location,
                )?;
            }
            self.advance(budget, location)?;
        }
        unreachable!("complete copy traffic exceeded the remaining allowance")
    }

    fn advance(&mut self, budget: &Budget, location: ProgramSite) -> Result<(), ExpansionFailure> {
        for column in self.columns.iter_mut().rev() {
            budget.poll(location)?;
            if column.advance() {
                break;
            }
        }
        Ok(())
    }
}

/// Retain the existing logical family envelope even though it is no longer
/// materialized. It is independent of actual Vec capacity and process RSS.
fn logical_family(
    atom: &Atom,
    sizes: &[(u128, u128)],
    count: u128,
    intermediate: u128,
    retained: &mut u128,
    budget: &Budget,
    location: ProgramSite,
) -> Result<(), ExpansionFailure> {
    let payload = sizes
        .iter()
        .fold(0_u128, |sum, (_, bytes)| sum.saturating_add(*bytes));
    let output = count.saturating_mul(
        size_of::<Template>() as u128
            + sizes.len() as u128 * size_of::<Term>() as u128
            + atom.name.as_str().len() as u128
            + payload,
    );
    *retained = retained.saturating_add(output);
    let columns = intermediate
        .saturating_mul(size_of::<Value>() as u128)
        .saturating_add(payload)
        .saturating_add(
            sizes.len() as u128 * (size_of::<Vec<Value>>() + size_of::<usize>()) as u128,
        );
    budget.check_family(retained.saturating_add(columns), location)
}
