//! Checked finite fact products; no rule binding enumeration occurs here.
//!
//! Logical expansion limits still count the complete family. Only its compact
//! scalar/range columns remain live; admission owners consume their products
//! directly. No successful partial program is published.

use crate::ProgramSite;
use themelios_program::program::{Atom, DefaultNegation, Head, LiteralInner, Statement};
use themelios_program::provenance::WithProvenance;

use crate::ExpansionFailure;
use crate::expansion::Budget;

mod columns;
mod product;
use product::Product;

/// One fully checked family, in alternative order and last-argument-fastest
/// product order. The enclosing compilation refuses a failed transfer rather
/// than publishing its accepted prefix.
///
/// A buffer reused across statements keeps its vectors: [`facts_into`] moves
/// the last statement's products to `spare` and refills them in place, so a
/// run of plain facts compiles without allocating a product, its columns or
/// their segments per statement. The spare shells hold no value, and a shell
/// keeps only a small column's capacity, so a large pool's storage is not
/// retained past its statement.
#[derive(Debug, Default)]
pub(crate) struct Facts {
    products: Vec<Product>,
    rows: usize,
    spare: Vec<Product>,
    sizes: Vec<(u128, u128)>,
}

impl Facts {
    pub(crate) fn len(&self) -> usize {
        self.rows
    }

    /// The checked products in alternative order; each admission owner reads
    /// the compact columns once.
    pub(crate) fn products(&self) -> impl Iterator<Item = ProductView<'_>> {
        self.products.iter().map(|product| ProductView { product })
    }
}

/// One checked product of a family: its predicate, its columns and its row
/// count, with the last argument varying fastest in the Cartesian product.
#[derive(Clone, Copy)]
pub(crate) struct ProductView<'a> {
    product: &'a Product,
}

impl<'a> ProductView<'a> {
    pub(crate) fn predicate(self) -> &'a zetesis_core::Predicate {
        self.product.predicate()
    }

    /// The product's row count before any row is emitted.
    pub(crate) fn len(self) -> usize {
        self.product.len()
    }

    /// Each argument's segments, in emission order.
    pub(crate) fn columns(self) -> impl DoubleEndedIterator<Item = impl Iterator<Item = Cell<'a>>> {
        self.product.columns().iter().map(columns::Column::cells)
    }
}

pub(crate) use columns::Cell;

/// The head atom of a fact statement: a rule with an empty body and a positive
/// atom head. Any other statement is no fact producer.
pub(crate) fn fact_atom(statement: &Statement) -> Option<&Atom> {
    let Statement::Rule(rule) = statement else {
        return None;
    };
    if rule.body().get().elements().next().is_some() {
        return None;
    }
    let Head::Literal(literal) = rule.head().get() else {
        return None;
    };
    if literal.negation != DefaultNegation::None {
        // A default-negated literal remains a formula, never a fact producer.
        return None;
    }
    let LiteralInner::Atom(atom) = &literal.inner else {
        return None;
    };
    Some(atom.get())
}

#[cfg(test)]
pub(crate) fn facts(
    carrier: &WithProvenance<Statement>,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Option<Facts>, ExpansionFailure> {
    let mut facts = Facts::default();
    Ok(facts_into(carrier, budget, location, &mut facts)?.then_some(facts))
}

/// Check `carrier`'s fact family into `facts`, replacing what it held; false,
/// with `facts` emptied, when the statement is no fact producer. Charges,
/// checks and refusals are those of a fresh family: only the vectors' storage
/// is reused.
pub(crate) fn facts_into(
    carrier: &WithProvenance<Statement>,
    budget: &mut Budget,
    location: ProgramSite,
    facts: &mut Facts,
) -> Result<bool, ExpansionFailure> {
    facts.rows = 0;
    for mut product in facts.products.drain(..).rev() {
        product.recycle();
        facts.spare.push(product);
    }
    facts.spare.truncate(RETAINED_PRODUCTS);
    let Some(atom) = fact_atom(carrier.get()) else {
        return Ok(false);
    };
    let mut logical_bytes = 0;
    for arguments in atom.alternatives() {
        let mut product = facts.spare.pop().unwrap_or_default();
        if product.prepare(
            atom,
            arguments,
            &mut facts.sizes,
            &mut logical_bytes,
            budget,
            location,
        )? {
            reserve(&mut facts.products, 1, budget, location)?;
            // The cumulative Templates charge already fits a usize ceiling.
            facts.rows += product.len();
            facts.products.push(product);
        } else {
            facts.spare.push(product);
        }
    }
    Ok(true)
}

/// The product shells a buffer keeps between statements, as for columns.
const RETAINED_PRODUCTS: usize = 8;

/// Preserve the allocator's typed failure independently of logical expansion
/// limits. Source carriers and spare capacity are not new logical values.
fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    budget: &Budget,
    location: ProgramSite,
) -> Result<(), ExpansionFailure> {
    budget.poll(location)?;
    values
        .try_reserve(additional)
        .map_err(|error| ExpansionFailure::Allocation { error, location })
}

/// Room for exactly `additional` more cells in a buffer that may be reused,
/// with the allocator's typed failure; capacity already there is kept.
fn reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    budget: &Budget,
    location: ProgramSite,
) -> Result<(), ExpansionFailure> {
    budget.poll(location)?;
    values
        .try_reserve_exact(additional)
        .map_err(|error| ExpansionFailure::Allocation { error, location })
}

/// Known one-shot payloads do not acquire unnecessary geometric spare cells.
#[cfg(test)]
fn reserved<T>(
    count: usize,
    budget: &Budget,
    location: ProgramSite,
) -> Result<Vec<T>, ExpansionFailure> {
    budget.poll(location)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| ExpansionFailure::Allocation { error, location })?;
    Ok(values)
}

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
