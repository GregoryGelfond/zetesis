//! One alternative's compact Cartesian product and complete copy preflight.

use themelios_program::program::Atom;
use themelios_program::term::Term as SourceTerm;
use zetesis_core::{AtomPattern, Predicate, Template, Term, Value};

use super::{
    columns::{self, Column},
    reserved,
};
use crate::expansion::Budget;
use crate::{AdmissionFailure, ExpansionFailure, ExpansionResource, ProgramSite};

#[derive(Debug)]
pub(super) struct Product {
    predicate: Predicate,
    columns: Vec<Column>,
    remaining: usize,
}

impl Product {
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.predicate.name().len()
            + self.columns.capacity() * size_of::<Column>()
            + self
                .columns
                .iter()
                .map(Column::retained_bytes)
                .sum::<usize>()
    }

    pub(super) fn prepare(
        atom: &Atom,
        arguments: &[SourceTerm],
        logical_bytes: &mut u128,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<Option<Self>, ExpansionFailure> {
        let mut sizes = reserved(arguments.len(), budget, location)?;
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
            return Ok(None);
        }
        budget.charge(
            ExpansionResource::ScalarBytes,
            (atom.name.as_str().len() as u128).saturating_mul(count),
            location,
        )?;
        logical_family(
            atom,
            &sizes,
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
        let mut columns = reserved(arguments.len(), budget, location)?;
        let mut copied_bytes = 0_u128;
        for (term, (width, _)) in arguments.iter().zip(sizes) {
            let (column, payload) = Column::prepare(term, budget, location)?;
            copied_bytes = copied_bytes.saturating_add((count / width).saturating_mul(payload));
            columns.push(column);
        }
        let mut product = Self {
            predicate,
            columns,
            remaining: usize::try_from(count).expect("Templates charge has a usize ceiling"),
        };
        product.charge_copies(copied_bytes, budget, location)?;
        Ok(Some(product))
    }

    pub(super) fn len(&self) -> usize {
        self.remaining
    }

    pub(super) fn is_empty(&self) -> bool {
        self.remaining == 0
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
        for _ in 0..self.remaining {
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

    pub(super) fn next(
        &mut self,
        budget: &Budget,
        location: ProgramSite,
    ) -> Result<AtomPattern, ExpansionFailure> {
        budget.poll(location)?;
        let mut terms = reserved(self.columns.len(), budget, location)?;
        for column in &self.columns {
            budget.poll(location)?;
            terms.push(Term::Constant(column.value()));
        }
        let head = AtomPattern::new(self.predicate.clone(), terms)
            .map_err(|error| AdmissionFailure::Construction { error, location })?;
        self.advance(budget, location)?;
        self.remaining -= 1;
        Ok(head)
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
