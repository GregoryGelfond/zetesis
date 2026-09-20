//! Per-relational-row scalar/range cursors; no global value-domain products.

use std::ops::{Range, RangeInclusive};

use themelios_base::span::Location;
use zetesis_core::Value;

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr};
use crate::formula_support::{Counters, Evaluation, Failures, Support, copy};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

enum State {
    Fresh,
    Scalar(Option<Value>),
    Range(Option<RangeInclusive<i32>>),
    Values {
        values: Vec<Value>,
        index: usize,
    },
    /// One missing arithmetic output, retained while independent steps run.
    Unavailable {
        pending: bool,
    },
}

struct Generator<'a> {
    literal: &'a LiteralIr,
    required: Option<&'a [usize]>,
}

enum Alternative {
    Value(Value),
    Unavailable,
    Exhausted,
}

pub(super) struct Cursor<'a, 'source> {
    generators: Vec<Generator<'a>>,
    states: Vec<State>,
    values: Binding<'static>,
    support: &'a Support<'source>,
    depth: usize,
    finished: bool,
    variables: usize,
    /// Missing outputs in the active prefix; absent values never enter Binding.
    unavailable: usize,
    failed_initialization: Option<usize>,
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

impl<'a, 'source> Cursor<'a, 'source> {
    pub fn new(
        literals: &'a [LiteralIr],
        values: Binding<'static>,
        support: &'a Support<'source>,
        plan: Option<&'a crate::formula_assignment_plan::Plan>,
        targets: Range<usize>,
    ) -> Self {
        let generators = if let Some(plan) = plan {
            plan.steps
                .iter()
                .filter(|step| targets.contains(&step.produced))
                .map(|step| Generator {
                    literal: &literals[step.literal],
                    required: Some(&step.required),
                })
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
                .map(|literal| Generator {
                    literal,
                    required: None,
                })
                .collect();
            generators.extend(
                literals
                    .iter()
                    .filter(|literal| match literal {
                        LiteralIr::Aggregate(aggregate) => aggregate
                            .binding
                            .is_some_and(|target| targets.contains(&target)),
                        _ => false,
                    })
                    .map(|literal| Generator {
                        literal,
                        required: None,
                    }),
            );
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
            unavailable: 0,
            failed_initialization: None,
        }
    }

    /// Discard the generator alternative whose initialization just failed.
    /// Earlier range alternatives remain available on the next call.
    pub(super) fn reject(&mut self) {
        if let Some(depth) = self.failed_initialization.take() {
            self.states[depth] = State::Scalar(None);
        }
    }

    pub(super) fn binding(&self) -> &Binding<'static> {
        &self.values
    }

    /// At depth d, earlier steps have populated or explicitly withheld every
    /// input of step d. Missing arithmetic inputs propagate only to dependent
    /// operations; independent alternatives still undergo checked evaluation.
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
                if self.unavailable != 0 {
                    return Err(evaluation.zero_divisor_failure(location));
                }
                for generator in &self.generators {
                    counters.work(limits, location)?;
                    self.values.read(
                        target(generator.literal).expect("generator target"),
                        location,
                    )?;
                }
                return self
                    .values
                    .copied(limits, counters, budget, location)
                    .map(Some);
            }
            if matches!(self.states[self.depth], State::Fresh) {
                self.states[self.depth] =
                    match self.initialize(evaluation, limits, budget, counters, location) {
                        Ok(state) => state,
                        Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                            ..
                        })) if evaluation.zero_divisor() => State::Unavailable { pending: true },
                        Err(error) => {
                            self.failed_initialization = Some(self.depth);
                            return Err(error);
                        }
                    };
            }
            if self.advance(limits, budget, counters, location)? {
                self.depth += 1;
            } else if self.depth == 0 {
                self.finished = true;
            } else {
                self.depth -= 1;
            }
        }
        Ok(None)
    }

    fn advance(
        &mut self,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let alternative = match &mut self.states[self.depth] {
            State::Fresh => unreachable!("cursor initialized"),
            State::Scalar(value) => value
                .take()
                .map_or(Alternative::Exhausted, Alternative::Value),
            State::Range(range) => range
                .as_mut()
                .and_then(Iterator::next)
                .map(Value::Number)
                .map_or(Alternative::Exhausted, Alternative::Value),
            State::Values { values, index } => {
                if let Some(value) = values.get(*index) {
                    *index += 1;
                    Alternative::Value(copy(value, budget, location)?)
                } else {
                    Alternative::Exhausted
                }
            }
            State::Unavailable { pending } => {
                if std::mem::take(pending) {
                    Alternative::Unavailable
                } else {
                    self.unavailable -= 1;
                    Alternative::Exhausted
                }
            }
        };
        let target = target(self.generators[self.depth].literal).expect("generator target");
        match alternative {
            Alternative::Value(value) => {
                counters.generated(&value, limits, budget, location)?;
                self.values.extend_scope(self.variables, location)?;
                self.values.set(target, value, location)?;
                Ok(true)
            }
            Alternative::Unavailable => {
                self.values.extend_scope(self.variables, location)?;
                self.values.clear(target);
                self.unavailable += 1;
                Ok(true)
            }
            Alternative::Exhausted => {
                self.values.clear(target);
                self.states[self.depth] = State::Fresh;
                Ok(false)
            }
        }
    }

    fn initialize(
        &mut self,
        evaluation: &mut Evaluation,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<State, FormulaFailure> {
        match self.generators[self.depth].literal {
            LiteralIr::Bind { value, .. } => Ok(State::Scalar(Some(
                self.expression(value, evaluation, limits, budget, counters, location)?,
            ))),
            LiteralIr::Range { lower, upper, .. } => {
                let mut failures = Failures::default();
                let result = self.expression(lower, evaluation, limits, budget, counters, location);
                let lower = failures.value(result, evaluation.zero_divisor())?;
                let result = self.expression(upper, evaluation, limits, budget, counters, location);
                let upper = failures.value(result, evaluation.zero_divisor())?;
                failures.finish(evaluation)?;
                let number = |value| match value {
                    Value::Number(number) => Some(number),
                    _ => None,
                };
                let range = crate::integer_range::inclusive(
                    number(lower.expect("defined lower endpoint")),
                    number(upper.expect("defined upper endpoint")),
                );
                let width = range.as_ref().map_or(0, crate::integer_range::width);
                ceiling(
                    FormulaResource::AssignmentValues,
                    u128::from(width),
                    limits.max_assignment_values as u128,
                    location,
                )?;
                Ok(State::Range(range))
            }
            LiteralIr::Aggregate(aggregate) => {
                if self.missing_inputs(
                    self.generators[self.depth]
                        .required
                        .expect("aggregate producers have a checked assignment plan")
                        .iter()
                        .copied(),
                    limits,
                    counters,
                    location,
                )? {
                    return Ok(State::Unavailable { pending: true });
                }
                Ok(State::Values {
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
                })
            }
            _ => unreachable!("only binding instructions enter a value cursor"),
        }
    }

    fn expression(
        &self,
        expression: &Expression,
        evaluation: &mut Evaluation,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        if self.missing_inputs(expression.inputs(), limits, counters, location)? {
            evaluation.source_partial(
                expression,
                |variable| Ok(self.values.slots()[variable].as_ref()),
                limits,
                budget,
                counters,
                location,
            )
        } else {
            evaluation.source_expression(
                expression,
                |variable| self.values.read(variable, location),
                limits,
                budget,
                counters,
                location,
            )
        }
    }

    /// Absence is inherited only from an unavailable earlier producer. An
    /// ordinary unbound variable still violates the compiler's binding contract.
    fn missing_inputs(
        &self,
        inputs: impl Iterator<Item = usize>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        if self.unavailable == 0 {
            return Ok(false);
        }
        let mut missing = false;
        for variable in inputs {
            counters.work(limits, location)?;
            if self.values.read(variable, location).is_ok() {
                continue;
            }
            let mut inherited = false;
            for (generator, state) in self.generators[..self.depth].iter().zip(&self.states) {
                counters.work(limits, location)?;
                if target(generator.literal) == Some(variable)
                    && matches!(state, State::Unavailable { .. })
                {
                    inherited = true;
                    break;
                }
            }
            if !inherited {
                return Err(FormulaFailure::UnsafeVariable { variable, location });
            }
            missing = true;
        }
        Ok(missing)
    }
}
