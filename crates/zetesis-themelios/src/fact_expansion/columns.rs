//! Compact source alternatives and the shared checked product position.

use std::ops::RangeInclusive;

use themelios_program::symbol::Symbol;
use themelios_program::term::{EvalError, Term as SourceTerm};
use zetesis_core::Value;

use super::reserve;
use crate::diagnostic::unsupported;
use crate::expansion::Budget;
use crate::{ExpansionFailure, ExpansionResource, ProfileFeature, ProgramSite, compile};

#[derive(Debug)]
pub(super) struct Column {
    segments: Vec<Segment>,
    segment: usize,
    offset: u64,
}

#[derive(Debug)]
enum Segment {
    Scalar(Value),
    Interval(RangeInclusive<i32>),
}

impl Segment {
    fn width(&self) -> u64 {
        match self {
            Self::Scalar(_) => 1,
            Self::Interval(range) => crate::integer_range::width(range),
        }
    }
}

impl Column {
    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.segments.capacity() * size_of::<Segment>()
            + self
                .segments
                .iter()
                .map(|segment| match segment {
                    Segment::Scalar(value) => match value {
                        Value::Structured(value) => value.payload_bytes(),
                        Value::Symbol(value) | Value::String(value) => value.capacity(),
                        Value::Number(_) | Value::Infimum | Value::Supremum => 0,
                    },
                    Segment::Interval(_) => 0,
                })
                .sum::<usize>()
    }

    pub(super) fn prepare(
        term: &SourceTerm,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(Self, u128), ExpansionFailure> {
        let mut column = Self {
            segments: Vec::new(),
            segment: 0,
            offset: 0,
        };
        let mut payload = 0_u128;
        let mut pending = Vec::new();
        reserve(&mut pending, 1, budget, location)?;
        pending.push(term);
        while let Some(term) = pending.pop() {
            budget.poll(location)?;
            let segment = match term {
                SourceTerm::Pool(items) => {
                    reserve(&mut pending, items.len(), budget, location)?;
                    pending.extend(items.iter().rev());
                    continue;
                }
                SourceTerm::Interval { lower, upper } => {
                    let Some(range) = interval(lower, upper, location)? else {
                        continue;
                    };
                    Segment::Interval(range)
                }
                SourceTerm::Symbolic(symbol) => {
                    budget.charge(
                        ExpansionResource::ScalarBytes,
                        crate::structural_value::symbol_bytes(symbol),
                        location,
                    )?;
                    let value = compile::scalar(symbol, location)?;
                    payload = payload.saturating_add(value_bytes(&value));
                    Segment::Scalar(value)
                }
                _ => return Err(unsupported(ProfileFeature::Term, location).into()),
            };
            reserve(&mut column.segments, 1, budget, location)?;
            column.segments.push(segment);
        }
        Ok((column, payload))
    }

    pub(super) fn value(&self) -> Value {
        match &self.segments[self.segment] {
            Segment::Scalar(value) => value.clone(),
            Segment::Interval(range) => Value::Number(
                i32::try_from(
                    i64::from(*range.start())
                        + i64::try_from(self.offset)
                            .expect("an i32 interval has at most 2^32 positions"),
                )
                .expect("the current position belongs to the checked interval"),
            ),
        }
    }

    pub(super) fn value_bytes(&self) -> u128 {
        match &self.segments[self.segment] {
            Segment::Scalar(value) => value_bytes(value),
            Segment::Interval(_) => 0,
        }
    }

    /// Advance once, resetting to the first alternative when carrying left.
    pub(super) fn advance(&mut self) -> bool {
        self.offset += 1;
        if self.offset < self.segments[self.segment].width() {
            return true;
        }
        self.offset = 0;
        self.segment += 1;
        if self.segment < self.segments.len() {
            return true;
        }
        self.segment = 0;
        false
    }
}

pub(super) fn size(
    term: &SourceTerm,
    budget: &Budget,
    location: ProgramSite,
) -> Result<(u128, u128), ExpansionFailure> {
    let mut pending = Vec::new();
    reserve(&mut pending, 1, budget, location)?;
    pending.push(term);
    let mut size = 0_u128;
    let mut bytes = 0_u128;
    while let Some(term) = pending.pop() {
        budget.poll(location)?;
        match term {
            SourceTerm::Pool(items) => {
                reserve(&mut pending, items.len(), budget, location)?;
                // Validation historically visits pool leaves in reverse order;
                // emission has its own forward order in Column::prepare.
                pending.extend(items);
            }
            SourceTerm::Interval { lower, upper } => {
                let range = interval(lower, upper, location)?;
                let width = range.as_ref().map_or(0, crate::integer_range::width);
                size = size.saturating_add(u128::from(width));
            }
            SourceTerm::Symbolic(symbol) => {
                compile::validate_scalar(symbol, location)?;
                size = size.saturating_add(1);
                bytes = bytes.saturating_add(crate::structural_value::symbol_bytes(symbol));
            }
            SourceTerm::Variable(variable) => {
                return Err(ExpansionFailure::Evaluation {
                    error: EvalError::NotGround {
                        variable: variable.clone(),
                    },
                    location,
                });
            }
            _ => return Err(unsupported(ProfileFeature::Term, location).into()),
        }
    }
    Ok((size, bytes))
}

fn interval(
    lower: &SourceTerm,
    upper: &SourceTerm,
    location: ProgramSite,
) -> Result<Option<RangeInclusive<i32>>, ExpansionFailure> {
    // Neither an empty lower range nor a nonnumeric value hides a bad endpoint.
    let lower = endpoint(lower, location)?;
    let upper = endpoint(upper, location)?;
    Ok(crate::integer_range::inclusive(lower, upper))
}

fn endpoint(term: &SourceTerm, location: ProgramSite) -> Result<Option<i32>, ExpansionFailure> {
    match term {
        SourceTerm::Symbolic(symbol) => {
            compile::validate_scalar(symbol, location)?;
            Ok(match symbol {
                Symbol::Number(number) => Some(*number),
                _ => None,
            })
        }
        SourceTerm::Variable(variable) => Err(ExpansionFailure::Evaluation {
            error: EvalError::NotGround {
                variable: variable.clone(),
            },
            location,
        }),
        _ => Err(unsupported(ProfileFeature::Term, location).into()),
    }
}

fn value_bytes(value: &Value) -> u128 {
    match value {
        Value::Infimum | Value::Supremum | Value::Number(_) => 0,
        Value::Structured(value) => value.payload_bytes() as u128,
        Value::String(value) | Value::Symbol(value) => value.len() as u128,
    }
}
