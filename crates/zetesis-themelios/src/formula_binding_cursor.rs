//! Per-relational-row scalar/range cursors; no global value-domain products.

use std::ops::Range;

use themelios_base::span::Location;
use zetesis_core::Value;

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::LiteralIr;
use crate::formula_support::{Counters, Evaluation, Support, copy};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

enum State {
    Fresh,
    Scalar(Option<Value>),
    Range { next: i64, end: i64 },
    Values { values: Vec<Value>, index: usize },
}

pub(super) struct Cursor<'a> {
    generators: Vec<&'a LiteralIr>,
    states: Vec<State>,
    values: Binding<'static>,
    support: &'a Support<'a>,
    depth: usize,
    finished: bool,
    variables: usize,
}

pub(super) fn target(literal: &LiteralIr) -> Option<usize> {
    match literal {
        LiteralIr::Bind { target, .. }
        | LiteralIr::Range {
            target,
            binder: true,
            ..
        } => Some(*target),
        LiteralIr::Aggregate(aggregate) => aggregate.binding,
        _ => None,
    }
}

impl<'a> Cursor<'a> {
    pub fn new(
        literals: &'a [LiteralIr],
        values: Binding<'static>,
        support: &'a Support<'a>,
        plan: Option<&'a crate::formula_assignment_plan::Plan>,
        targets: Range<usize>,
    ) -> Self {
        let generators = if let Some(plan) = plan {
            plan.steps
                .iter()
                .filter(|step| targets.contains(&step.produced))
                .map(|step| &literals[step.literal])
                .collect()
        } else {
            let mut generators: Vec<_> = literals
                .iter()
                .filter(|literal| {
                    matches!(
                        literal,
                        LiteralIr::Bind { .. } | LiteralIr::Range { binder: true, .. }
                    ) && target(literal).is_some_and(|target| targets.contains(&target))
                })
                .collect();
            generators.extend(literals.iter().filter(|literal| {
                match literal {
                    LiteralIr::Aggregate(aggregate) => aggregate
                        .binding
                        .is_some_and(|target| targets.contains(&target)),
                    _ => false,
                }
            }));
            generators
        };
        let states = (0..generators.len()).map(|_| State::Fresh).collect();
        Self {
            generators,
            states,
            values,
            support,
            depth: 0,
            finished: false,
            variables: targets.end,
        }
    }

    /// At depth d, earlier steps have populated every input of step d.
    /// Backtracking resets later states before changing an earlier value.
    /// Aggregate consumers use the compiler's checked plan; established local
    /// scopes retain their existing scalar dependency order.
    pub fn next(
        &mut self,
        evaluation: &mut Evaluation,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Binding<'static>>, FormulaFailure> {
        while !self.finished {
            counters.work(limits, location)?;
            if self.depth == self.generators.len() {
                assert_eq!(
                    self.values.len(),
                    self.variables,
                    "complete generated frame"
                );
                if self.depth == 0 {
                    self.finished = true;
                } else {
                    self.depth -= 1;
                }
                counters.substitution(limits, location)?;
                for generator in &self.generators {
                    counters.work(limits, location)?;
                    self.values
                        .read(target(generator).expect("generator target"), location)?;
                }
                return self.values.copied(budget, location).map(Some);
            }
            if matches!(self.states[self.depth], State::Fresh) {
                self.states[self.depth] =
                    self.initialize(evaluation, limits, budget, counters, location)?;
            }
            let value = match &mut self.states[self.depth] {
                State::Fresh => unreachable!("cursor initialized"),
                State::Scalar(value) => value.take(),
                State::Range { next, end } if *next <= *end => {
                    let value = i32::try_from(*next).expect("range endpoints are i32");
                    *next += 1;
                    Some(Value::Number(value))
                }
                State::Range { .. } => None,
                State::Values { values, index } => {
                    if let Some(value) = values.get(*index) {
                        *index += 1;
                        Some(copy(value, budget, location)?)
                    } else {
                        None
                    }
                }
            };
            if let Some(value) = value {
                counters.generated(&value, limits, budget, location)?;
                let target = target(self.generators[self.depth]).expect("generator target");
                self.values.extend_scope(self.variables, budget, location)?;
                self.values.set(target, value, location)?;
                self.depth += 1;
            } else {
                self.values
                    .clear(target(self.generators[self.depth]).expect("generator target"));
                self.states[self.depth] = State::Fresh;
                if self.depth == 0 {
                    self.finished = true;
                } else {
                    self.depth -= 1;
                }
            }
        }
        Ok(None)
    }

    fn initialize(
        &mut self,
        evaluation: &mut Evaluation,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<State, FormulaFailure> {
        match self.generators[self.depth] {
            LiteralIr::Bind { value, .. } => Ok(State::Scalar(Some(evaluation.expression(
                value,
                |variable| self.values.read(variable, location),
                limits,
                budget,
                counters,
                location,
            )?))),
            LiteralIr::Range { lower, upper, .. } => {
                let lower = evaluation.expression(
                    lower,
                    |variable| self.values.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let upper = evaluation.expression(
                    upper,
                    |variable| self.values.read(variable, location),
                    limits,
                    budget,
                    counters,
                    location,
                )?;
                let (Value::Number(lower), Value::Number(upper)) = (lower, upper) else {
                    return Ok(State::Range { next: 1, end: 0 });
                };
                let width = (i64::from(upper) - i64::from(lower) + 1).max(0);
                ceiling(
                    FormulaResource::AssignmentValues,
                    u128::try_from(width).expect("nonnegative range width"),
                    limits.max_assignment_values as u128,
                    location,
                )?;
                Ok(State::Range {
                    next: i64::from(lower),
                    end: i64::from(upper),
                })
            }
            LiteralIr::Aggregate(aggregate) => Ok(State::Values {
                values: crate::formula_assignment::values(
                    aggregate,
                    &self.values,
                    self.support,
                    limits,
                    budget,
                    counters,
                    location,
                )?,
                index: 0,
            }),
            _ => unreachable!("only binding instructions enter a value cursor"),
        }
    }
}
