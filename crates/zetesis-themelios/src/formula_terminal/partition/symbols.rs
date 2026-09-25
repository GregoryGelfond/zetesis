//! Typed closed constants reuse the source bridge's single borrowed traversal.

use themelios_program::symbol::Symbol;
use zetesis_core::{
    ValueError, ValueLimits, ValueNodeRef,
    catalog::{TermNodes, TermRef},
};

use super::workspace::Context;
use crate::formula_support::StorageLease;
use crate::structural_value::{self, Sink};
use crate::{AdmissionFailure, FormulaFailure};

enum Failure {
    Value(ValueError),
    Formula(FormulaFailure),
}
impl From<ValueError> for Failure {
    fn from(value: ValueError) -> Self {
        Self::Value(value)
    }
}
impl From<FormulaFailure> for Failure {
    fn from(value: FormulaFailure) -> Self {
        Self::Formula(value)
    }
}

struct Comparison<'a, 'b, 'source> {
    context: &'a mut Context<'b, 'source>,
    actual: TermNodes<'b>,
    equal: bool,
    lease: StorageLease,
}

impl Comparison<'_, '_, '_> {
    fn header() -> usize {
        size_of::<Self>() + size_of::<Vec<(&Symbol, usize)>>()
    }
}

impl Sink for Comparison<'_, '_, '_> {
    type Output = bool;
    type Error = Failure;

    fn start<'a>(&mut self, symbol: &'a Symbol) -> Result<Vec<(&'a Symbol, usize)>, Failure> {
        let mut pending = Vec::new();
        self.reserve(&mut pending, 1)?;
        pending.push((symbol, 1));
        Ok(pending)
    }

    fn before(&mut self) -> Result<(), Failure> {
        self.context.work().map_err(Into::into)
    }
    fn continues(&self) -> bool {
        self.equal
    }

    fn reserve(
        &mut self,
        pending: &mut Vec<(&Symbol, usize)>,
        additional: usize,
    ) -> Result<(), Failure> {
        self.context
            .reserve(pending, additional, &mut self.lease, Self::header())?;
        self.context.counters.charge_work(
            additional as u128,
            self.context.limits,
            self.context.location,
        )?;
        Ok(())
    }

    fn node(&mut self, expected: ValueNodeRef<'_>) -> Result<(), Failure> {
        let actual = self.actual.next_with(|| self.context.work())?;
        self.equal = match actual {
            Some(actual) => descriptors(expected, actual, self.context)?,
            None => false,
        };
        Ok(())
    }

    fn finish(mut self, _bytes: u128, _limits: ValueLimits) -> Result<bool, Failure> {
        if !self.equal {
            return Ok(false);
        }
        Ok(self.actual.next_with(|| self.context.work())?.is_none())
    }
}

pub(super) fn matches<'a>(
    symbol: &Symbol,
    actual: TermRef<'a>,
    context: &mut Context<'a, '_>,
) -> Result<bool, FormulaFailure> {
    let location = context.location;
    let mut lease = context.admission.lease();
    let header = Comparison::header();
    lease.observe(header, location)?;
    context.admission.storage_observed(
        &lease,
        0,
        header,
        context.limits,
        context.counters,
        location,
    )?;
    let sink = Comparison {
        context,
        actual: actual.nodes(),
        equal: true,
        lease,
    };
    // Already admitted values are compared, not reconstructed under fresh
    // default value limits. The current formula work and workspace remain bound.
    structural_value::traverse(
        symbol,
        ValueLimits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        },
        sink,
    )
    .map_err(|error| match error {
        Failure::Formula(error) => error,
        Failure::Value(error) => AdmissionFailure::Construction {
            error: zetesis_core::ConstructionError::Value(error),
            location,
        }
        .into(),
    })
}

fn descriptors(
    expected: ValueNodeRef<'_>,
    actual: ValueNodeRef<'_>,
    context: &mut Context<'_, '_>,
) -> Result<bool, FormulaFailure> {
    context.work()?;
    match (expected, actual) {
        (ValueNodeRef::Infimum, ValueNodeRef::Infimum)
        | (ValueNodeRef::Supremum, ValueNodeRef::Supremum) => Ok(true),
        (ValueNodeRef::Number(a), ValueNodeRef::Number(b)) => Ok(a == b),
        (ValueNodeRef::Tuple { arity: a }, ValueNodeRef::Tuple { arity: b }) => Ok(a == b),
        (ValueNodeRef::String(a), ValueNodeRef::String(b))
        | (ValueNodeRef::Symbol(a), ValueNodeRef::Symbol(b)) => context.text(a, b),
        (
            ValueNodeRef::Function {
                name: a,
                sign: sa,
                arity: aa,
            },
            ValueNodeRef::Function {
                name: b,
                sign: sb,
                arity: ab,
            },
        ) => {
            if sa != sb || aa != ab {
                return Ok(false);
            }
            context.text(a, b)
        }
        _ => Ok(false),
    }
}
