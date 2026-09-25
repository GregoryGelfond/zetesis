//! Per-relational-row scalar/range cursors over scoped term metadata.

use crate::formula_support::{Context, GroundingWork};
use std::ops::{Range, RangeInclusive};
use themelios_base::span::Location;
use zetesis_core::ValueNodeRef;
use zetesis_core::catalog::TermKey;

use crate::expansion::Budget;
use crate::formula::ceiling;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr};
use crate::formula_support::{Computation, Counters, Evaluation, Failures, StorageLease, Support};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};

enum State {
    Fresh,
    Scalar {
        pending: bool,
    },
    Range(Option<RangeInclusive<i32>>),
    Values {
        values: Binding<'static>,
        index: usize,
    },
    Unavailable {
        pending: bool,
    },
}
struct Generator<'a> {
    literal: &'a LiteralIr,
    required: Option<&'a [usize]>,
}
enum Alternative {
    Value(TermKey),
    Unavailable,
    Exhausted,
}

pub(super) struct Cursor<'a, 'source> {
    generators: Vec<Generator<'a>>,
    states: Vec<State>,
    value_frames: usize,
    values: Binding<'static>,
    scalars: Binding<'static>,
    lease: StorageLease,
    support: &'a Support<'source>,
    depth: usize,
    finished: bool,
    variables: usize,
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
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let scalars = Binding::new(computation, limits, counters, location)?;
        let mut cursor = Self {
            generators: Vec::new(),
            states: Vec::new(),
            value_frames: 0,
            values,
            scalars,
            lease: computation.lease(),
            support,
            depth: 0,
            finished: false,
            variables: targets.end,
            unavailable: 0,
            failed_initialization: None,
        };
        cursor.lease.observe(cursor.bytes(), location)?;
        computation.storage_observed(
            &cursor.lease,
            0,
            Self::header(),
            limits,
            counters,
            location,
        )?;
        cursor.admit_generators(
            literals,
            plan,
            &targets,
            Context::new(computation, limits, counters, location),
        )?;
        let other = cursor.bytes() - cursor.states.capacity() * size_of::<State>();
        crate::formula_support::reserve(
            &mut cursor.states,
            cursor.generators.len(),
            &mut cursor.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        for _ in &cursor.generators {
            counters.work(limits, location)?;
            cursor.states.push(State::Fresh);
        }
        cursor.scalars.extend_scope(
            cursor.generators.len(),
            computation,
            limits,
            counters,
            location,
        )?;
        Ok(cursor)
    }

    /// Preserve the plan order, or the scalar-before-aggregate fallback order.
    fn admit_generators(
        &mut self,
        literals: &'a [LiteralIr],
        plan: Option<&'a crate::formula_assignment_plan::Plan>,
        targets: &Range<usize>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        if let Some(plan) = plan {
            for step in &plan.steps {
                counters.work(limits, location)?;
                if targets.contains(&step.produced) {
                    self.generator(
                        Generator {
                            literal: &literals[step.literal],
                            required: Some(&step.required),
                        },
                        computation,
                        limits,
                        counters,
                        location,
                    )?;
                }
            }
        } else {
            for literal in literals {
                counters.work(limits, location)?;
                if matches!(
                    literal,
                    LiteralIr::Bind { .. } | LiteralIr::Range { binder: true, .. }
                ) && target(literal).is_some_and(|target| targets.contains(&target))
                {
                    self.generator(
                        Generator {
                            literal,
                            required: None,
                        },
                        computation,
                        limits,
                        counters,
                        location,
                    )?;
                }
            }
            for literal in literals {
                counters.work(limits, location)?;
                if let LiteralIr::Aggregate(aggregate) = literal
                    && aggregate
                        .binding
                        .is_some_and(|target| targets.contains(&target))
                {
                    self.generator(
                        Generator {
                            literal,
                            required: None,
                        },
                        computation,
                        limits,
                        counters,
                        location,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn header() -> usize {
        size_of::<Self>() - 2 * size_of::<Binding<'static>>()
    }
    fn bytes(&self) -> usize {
        Self::header()
            + self.generators.capacity() * size_of::<Generator<'_>>()
            + self.states.capacity() * size_of::<State>()
            - self.value_frames * size_of::<Binding<'static>>()
    }
    fn generator(
        &mut self,
        value: Generator<'a>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let other = self.bytes() - self.generators.capacity() * size_of::<Generator<'_>>();
        crate::formula_support::reserve(
            &mut self.generators,
            1,
            &mut self.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        counters.work(limits, location)?;
        self.generators.push(value);
        Ok(())
    }
    fn state(&mut self, state: State, location: Location) -> Result<(), FormulaFailure> {
        self.value_frames -= usize::from(matches!(self.states[self.depth], State::Values { .. }));
        self.value_frames += usize::from(matches!(state, State::Values { .. }));
        self.states[self.depth] = state;
        // A list Binding's own lease includes its inline header. The enclosing
        // enum buffer accounts the remaining initialized and spare cell bytes.
        self.lease.observe(self.bytes(), location)
    }

    pub(super) fn reject(&mut self, location: Location) -> Result<(), FormulaFailure> {
        if let Some(depth) = self.failed_initialization.take() {
            self.value_frames -= usize::from(matches!(self.states[depth], State::Values { .. }));
            self.states[depth] = State::Scalar { pending: false };
            self.lease.observe(self.bytes(), location)?;
        }
        Ok(())
    }
    pub(super) fn binding(&self) -> &Binding<'static> {
        &self.values
    }

    pub fn next(
        &mut self,
        evaluation: &mut Evaluation,
        computation: &mut Computation<'_, '_>,
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
                    self.values.key(
                        target(generator.literal).expect("generator target"),
                        location,
                    )?;
                }
                return self
                    .values
                    .copied(computation, limits, counters, location)
                    .map(Some);
            }
            if matches!(self.states[self.depth], State::Fresh) {
                let initialized =
                    self.initialize(evaluation, computation, limits, budget, counters, location);
                let state = match initialized {
                    Ok(state) => state,
                    Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                        ..
                    })) if evaluation.zero_divisor() => State::Unavailable { pending: true },
                    Err(error) => {
                        self.failed_initialization = Some(self.depth);
                        return Err(error);
                    }
                };
                self.state(state, location)?;
            }
            if self.advance(computation, limits, counters, location)? {
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
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let alternative = match &mut self.states[self.depth] {
            State::Fresh => unreachable!("cursor initialized"),
            State::Scalar { pending } => {
                if std::mem::take(pending) {
                    Alternative::Value(self.scalars.key(self.depth, location)?)
                } else {
                    Alternative::Exhausted
                }
            }
            State::Range(range) => match range.as_mut().and_then(Iterator::next) {
                Some(number) => {
                    Alternative::Value(computation.number(number, limits, counters, location)?)
                }
                None => Alternative::Exhausted,
            },
            State::Values { values, index } => {
                if *index < values.len() {
                    let value = values.key(*index, location)?;
                    *index += 1;
                    Alternative::Value(value)
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
                counters.generated(&value, computation, limits, location)?;
                self.values.extend_scope(
                    self.variables,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
                self.values
                    .set(target, &value, limits, counters, location)?;
                Ok(true)
            }
            Alternative::Unavailable => {
                self.values.extend_scope(
                    self.variables,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
                self.values.clear(target, limits, counters, location)?;
                self.unavailable += 1;
                Ok(true)
            }
            Alternative::Exhausted => {
                self.values.clear(target, limits, counters, location)?;
                self.state(State::Fresh, location)?;
                Ok(false)
            }
        }
    }

    fn initialize(
        &mut self,
        evaluation: &mut Evaluation,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<State, FormulaFailure> {
        match self.generators[self.depth].literal {
            LiteralIr::Bind { value, .. } => {
                let value =
                    self.expression(value, evaluation, computation, limits, counters, location)?;
                self.scalars
                    .set(self.depth, &value, limits, counters, location)?;
                Ok(State::Scalar { pending: true })
            }
            LiteralIr::Range { lower, upper, .. } => {
                let mut failures = Failures::default();
                let result =
                    self.expression(lower, evaluation, computation, limits, counters, location);
                let lower = failures.value(result, evaluation.zero_divisor())?;
                let result =
                    self.expression(upper, evaluation, computation, limits, counters, location);
                let upper = failures.value(result, evaluation.zero_divisor())?;
                failures.finish(evaluation)?;
                let read = computation.read();
                let mut number = |key: &TermKey| -> Result<Option<i32>, FormulaFailure> {
                    counters.work(limits, location)?;
                    let value = read.term(key).map_err(|error| {
                        crate::formula_binding::assignment(
                            zetesis_core::catalog::AssignmentError::Read(error),
                            location,
                        )
                    })?;
                    Ok(match value.descriptor() {
                        ValueNodeRef::Number(number) => Some(number),
                        _ => None,
                    })
                };
                let range = crate::integer_range::inclusive(
                    number(&lower.expect("defined lower endpoint"))?,
                    number(&upper.expect("defined upper endpoint"))?,
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
                let values = crate::formula_assignment::values(
                    aggregate,
                    &self.values,
                    self.support,
                    budget,
                    Context::new(computation, limits, counters, location),
                )?;
                Ok(State::Values { values, index: 0 })
            }
            _ => unreachable!("only binding instructions enter a value cursor"),
        }
    }

    fn expression(
        &self,
        expression: &Expression,
        evaluation: &mut Evaluation,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        if self.missing_inputs(expression.inputs(), limits, counters, location)? {
            evaluation.source_partial(
                expression,
                |variable| {
                    self.values
                        .slots()
                        .key(variable)
                        .map_err(|error| crate::formula_binding::assignment(error, location))
                },
                computation,
                limits,
                counters,
                location,
            )
        } else {
            evaluation.source_expression(
                expression,
                |variable| self.values.key(variable, location),
                computation,
                limits,
                counters,
                location,
            )
        }
    }

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
            if self.values.key(variable, location).is_ok() {
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
