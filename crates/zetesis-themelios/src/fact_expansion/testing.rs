//! Test-only materialization for the independent facts-as-rules comparison.
//!
//! Read the checked columns directly. Neither core fact admission nor the
//! formula arena participates, so their order and identity bugs cannot define
//! the reference result. At most one owned head is live at a time.

use super::{Cell, Facts, ProductView, reserved};
use crate::expansion::Budget;
use crate::{AdmissionFailure, ExpansionFailure, ProgramSite};
use zetesis_core::{AtomPattern, Term, Value};

pub(crate) struct Rows {
    facts: Facts,
    product: usize,
    row: usize,
}

impl Facts {
    pub(crate) fn into_rows(self) -> Rows {
        Rows {
            facts: self,
            product: 0,
            row: 0,
        }
    }
}

impl Rows {
    pub(crate) fn next(
        &mut self,
        budget: &Budget,
        site: ProgramSite,
    ) -> Result<Option<AtomPattern>, ExpansionFailure> {
        let Some(product) = self.facts.products.get(self.product) else {
            return Ok(None);
        };
        let product = ProductView { product };
        budget.poll(site)?;
        let mut terms = reserved(product.predicate().arity(), budget, site)?;
        let mut position = self.row;
        for column in product.columns().rev() {
            budget.poll(site)?;
            let width = column.map(|cell| width(&cell)).sum::<usize>();
            terms.push(position % width);
            position /= width;
        }
        terms.reverse();
        let mut values = reserved(terms.len(), budget, site)?;
        for (mut column, mut position) in product.columns().zip(terms) {
            budget.poll(site)?;
            let value = column
                .find_map(|cell| {
                    let count = width(&cell);
                    if position < count {
                        Some(value(cell, position))
                    } else {
                        position -= count;
                        None
                    }
                })
                .expect("position belongs to a checked nonempty column");
            values.push(Term::Constant(value));
        }
        let head = AtomPattern::new(product.predicate().clone(), values).map_err(|error| {
            AdmissionFailure::Construction {
                error,
                location: site,
            }
        })?;
        self.row += 1;
        if self.row == product.len() {
            self.row = 0;
            self.product += 1;
        }
        Ok(Some(head))
    }
}

fn width(cell: &Cell<'_>) -> usize {
    match cell {
        Cell::Scalar(_) => 1,
        Cell::Interval(range) => usize::try_from(crate::integer_range::width(range))
            .expect("checked product width fits usize"),
    }
}

fn value(cell: Cell<'_>, position: usize) -> Value {
    match cell {
        Cell::Scalar(value) => value.clone(),
        Cell::Interval(range) => Value::Number(
            i32::try_from(
                i64::from(*range.start()) + i64::try_from(position).expect("checked i32 width"),
            )
            .expect("position belongs to the checked interval"),
        ),
    }
}
