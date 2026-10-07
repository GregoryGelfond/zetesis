//! Scalar/range cursors over scoped term metadata and one immutable support.
//!
//! Backtracking rewinds an aggregate carrier while its declared inputs are
//! unchanged; publishing a changed input invalidates it. An exhausted cursor
//! can accept the next relational row under the same rule and support, keeping
//! only carriers whose required inputs are still present and equal.

#[cfg(test)]
mod tests;

use crate::ProgramSite;
use crate::formula_support::{Context, GroundingWork};
use std::ops::{Range, RangeInclusive};
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
                            required: Some(step.required(literals)),
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
        location: ProgramSite,
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
    fn state(&mut self, state: State, location: ProgramSite) -> Result<(), FormulaFailure> {
        self.state_at(self.depth, state, location)
    }

    fn state_at(
        &mut self,
        depth: usize,
        state: State,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.value_frames -= usize::from(matches!(self.states[depth], State::Values { .. }));
        self.value_frames += usize::from(matches!(state, State::Values { .. }));
        self.states[depth] = state;
        // A list Binding's own lease includes its inline header. The enclosing
        // enum buffer accounts the remaining initialized and spare cell bytes.
        self.lease.observe(self.bytes(), location)
    }

    /// A deeper carrier can observe this output only through its checked
    /// input list. Transitive consumers are invalidated when their intermediate
    /// output is published. Unplanned generators conservatively lose reuse.
    /// No new owner or input snapshot is retained; each visit is charged.
    fn invalidate_dependents(
        &mut self,
        changed: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let current = usize::from(matches!(self.states[self.depth], State::Values { .. }));
        if self.value_frames == current {
            return Ok(());
        }
        for depth in self.depth + 1..self.generators.len() {
            counters.work(limits, location)?;
            if !matches!(self.states[depth], State::Values { .. }) {
                continue;
            }
            let depends = if let Some(inputs) = self.generators[depth].required {
                counters.charge_work(inputs.len() as u128, limits, location)?;
                inputs.contains(&changed)
            } else {
                true
            };
            if depends {
                self.state_at(depth, State::Fresh, location)?;
            }
        }
        Ok(())
    }

    /// Continue this exact rule and immutable support with its next relational
    /// row. The exhausted frame still owns its relational inputs; generated
    /// outputs have been cleared, so dependent carriers conservatively expire.
    /// Only the existing carrier and frame owners remain. Every input read is
    /// charged, and a refusal leaves the old cursor exhausted with honest leases.
    pub(super) fn restart(
        &mut self,
        values: Binding<'static>,
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
        assert!(
            self.finished,
            "only an exhausted cursor accepts another row"
        );
        // Authenticate the new frame, including empty/missing input slots,
        // before reusing even an input-free carrier. Both frames coexist here.
        computation.allowance(&self.lease, limits, location)?;
        values.view(computation.read(), limits, counters, location)?;
        for depth in 0..self.states.len() {
            counters.work(limits, location)?;
            if matches!(self.states[depth], State::Values { .. })
                && self.same_inputs(depth, &values, limits, counters, location)?
            {
                let State::Values { index, .. } = &mut self.states[depth] else {
                    unreachable!("selected retained carrier")
                };
                *index = 0;
            } else {
                self.state_at(depth, State::Fresh, location)?;
            }
        }
        counters.work(limits, location)?;
        self.values = values;
        self.depth = 0;
        self.finished = false;
        self.unavailable = 0;
        self.failed_initialization = None;
        Ok(())
    }

    fn same_inputs(
        &self,
        depth: usize,
        values: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        let Some(inputs) = self.generators[depth].required else {
            return Ok(false);
        };
        self.equal_inputs(inputs, values, limits, counters, location)
    }

    /// The previous continuation was fully visited under this same rule and
    /// support. Reusing its existing frame needs no retained projected key.
    /// The caller supplies the compiler's complete support-continuation reads;
    /// this operation never authorizes eliding a formula witness.
    pub(super) fn completed_with(
        &self,
        values: &Binding<'_>,
        inputs: &[usize],
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        assert!(self.finished, "only complete continuations can be reused");
        self.matches_inputs(values, inputs, context)
    }

    /// Compare unchanged relational inputs while this cursor is active or
    /// complete. The caller's checked plan determines which inputs suffice;
    /// this operation alone never authorizes skipping a continuation.
    pub(super) fn matches_inputs(
        &self,
        values: &Binding<'_>,
        inputs: &[usize],
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<bool, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        computation.allowance(&self.lease, limits, location)?;
        // Even an empty input list authenticates both canonical owners. The
        // retained frame already passed prefix admission; its empty prefix
        // checks scope without revisiting every unchanged slot.
        self.values
            .prefix(0)
            .view(computation.read(), limits, counters, location)?;
        values.view(computation.read(), limits, counters, location)?;
        self.equal_inputs(inputs, values, limits, counters, location)
    }

    fn equal_inputs(
        &self,
        inputs: &[usize],
        values: &Binding<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        for &slot in inputs {
            counters.work(limits, location)?;
            if slot >= self.values.len()
                || slot >= values.len()
                || !self.values.is_bound(slot, location)?
            {
                return Ok(false);
            }
            let Some(key) = values
                .slots()
                .key(slot)
                .map_err(|error| crate::formula_binding::assignment(error, location))?
            else {
                return Ok(false);
            };
            // Canonical identity equality preserves whole typed values; the
            // coordinate order has no role in source comparisons or output.
            if self
                .values
                .slots()
                .compare_key(slot, &key)
                .map_err(|error| crate::formula_binding::assignment(error, location))?
                != std::cmp::Ordering::Equal
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn reject(&mut self, location: ProgramSite) -> Result<(), FormulaFailure> {
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
        location: ProgramSite,
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
        location: ProgramSite,
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
                self.invalidate_dependents(target, limits, counters, location)?;
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
                self.invalidate_dependents(target, limits, counters, location)?;
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
                if let State::Values { index, .. } = &mut self.states[self.depth] {
                    // The complete carrier and its lease stay in their original
                    // owner. A dependent predecessor invalidates it before the
                    // next descent; an unrelated predecessor only rewinds it.
                    *index = 0;
                } else {
                    self.state(State::Fresh, location)?;
                }
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
        location: ProgramSite,
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
        location: ProgramSite,
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
        location: ProgramSite,
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
