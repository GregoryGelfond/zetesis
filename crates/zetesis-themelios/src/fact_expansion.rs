//! Checked finite fact products; no rule binding enumeration occurs here.
//!
//! Logical expansion limits still count the complete family. Only its compact
//! scalar/range columns remain live; callers consume one head at a time through
//! their existing admission owners. No successful partial program is published.

use crate::ProgramSite;
use themelios_program::program::{DefaultNegation, Head, LiteralInner, Statement};
use themelios_program::provenance::WithProvenance;
use zetesis_core::AtomPattern;

use crate::ExpansionFailure;
use crate::expansion::Budget;

mod columns;
mod product;
use product::Product;

/// One fully checked family, in alternative order and last-argument-fastest
/// product order. An error fuses the cursor; the enclosing compilation refuses
/// the input rather than publishing or retrying its accepted prefix.
#[derive(Debug)]
pub(crate) struct Facts {
    products: Vec<Product>,
    product: usize,
    remaining: usize,
    stopped: bool,
}

impl Facts {
    pub(crate) fn len(&self) -> usize {
        self.remaining
    }

    pub(crate) fn next(
        &mut self,
        budget: &Budget,
        location: ProgramSite,
    ) -> Result<Option<AtomPattern>, ExpansionFailure> {
        if self.stopped || self.remaining == 0 {
            return Ok(None);
        }
        let product = &mut self.products[self.product];
        match product.next(budget, location) {
            Ok(head) => {
                self.remaining -= 1;
                if product.is_empty() {
                    self.product += 1;
                }
                Ok(Some(head))
            }
            Err(error) => {
                self.stopped = true;
                Err(error)
            }
        }
    }
}

pub(crate) fn facts(
    carrier: &WithProvenance<Statement>,
    budget: &mut Budget,
    location: ProgramSite,
) -> Result<Option<Facts>, ExpansionFailure> {
    let Statement::Rule(rule) = carrier.get() else {
        return Ok(None);
    };
    if rule.body().get().elements().next().is_some() {
        return Ok(None);
    }
    let Head::Literal(literal) = rule.head().get() else {
        return Ok(None);
    };
    if literal.negation != DefaultNegation::None {
        // A default-negated literal remains a formula, never a fact producer.
        return Ok(None);
    }
    let LiteralInner::Atom(atom) = &literal.inner else {
        return Ok(None);
    };
    let atom = atom.get();
    let mut facts = Facts {
        products: Vec::new(),
        product: 0,
        remaining: 0,
        stopped: false,
    };
    let mut logical_bytes = 0;
    for arguments in atom.alternatives() {
        if let Some(product) =
            Product::prepare(atom, arguments, &mut logical_bytes, budget, location)?
        {
            reserve(&mut facts.products, 1, budget, location)?;
            // The cumulative Templates charge already fits a usize ceiling.
            facts.remaining += product.len();
            facts.products.push(product);
        }
    }
    Ok(Some(facts))
}

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

/// Known one-shot payloads do not acquire unnecessary geometric spare cells.
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
mod tests;
