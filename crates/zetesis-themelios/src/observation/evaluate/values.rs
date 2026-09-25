//! Finite expression alternatives with one scoped ID frame and logical charges.

use themelios_program::term::{BinaryOp, UnaryOp};
use zetesis_core::{
    ValueNodeRef,
    catalog::{TermAssignment, TermKey},
};

use super::{Binding, Error, ErrorKind, EvaluationError, Interpreter, Metric, Resource, Template};

pub(super) struct Values {
    terms: TermAssignment,
    metrics: Vec<Metric>,
    pub bytes: u128,
}
impl Values {
    fn new(context: &Interpreter<'_, '_, '_>) -> Self {
        Self {
            terms: context.terms.read().assignment(),
            metrics: Vec::new(),
            bytes: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.metrics.len()
    }
    pub fn metric(&self, index: usize) -> Metric {
        self.metrics[index]
    }
    pub fn key(
        &self,
        index: usize,
        context: &mut Interpreter<'_, '_, '_>,
    ) -> Result<TermKey, Error> {
        context.work.step(1)?;
        self.terms
            .key(index)
            .map_err(|error| context.work.error(ErrorKind::TermAssignment(error)))?
            .ok_or_else(|| context.work.error(ErrorKind::InvalidSymbol))
    }
    fn push<'input>(
        &mut self,
        metric: Metric,
        context: &mut Interpreter<'input, '_, '_>,
        make: impl FnOnce(&mut Interpreter<'input, '_, '_>) -> Result<TermKey, Error>,
    ) -> Result<(), Error> {
        context.work.check(
            Resource::LocalBytes,
            context.work.local_bytes + metric.payload(),
            context.work.limits.max_local_bytes as u128,
        )?;
        context.work.construction_check(metric)?;
        context.work.step(1)?;
        self.metrics
            .try_reserve(1)
            .map_err(|_| context.work.error(ErrorKind::Allocation))?;
        let next = self
            .len()
            .checked_add(1)
            .ok_or_else(|| context.work.error(ErrorKind::Allocation))?;
        self.terms
            .resize_with(next, context.work.limits.max_term_storage_bytes, || {
                context.work.step(1)
            })
            .map_err(|error| super::binding::failure(error, context.work))?;
        let key = make(context)?;
        self.terms
            .set_with(self.len(), &key, || context.work.step(1))
            .map_err(|error| super::binding::failure(error, context.work))?;
        self.bytes += metric.payload();
        context.work.local_bytes += metric.payload();
        self.metrics.push(metric);
        Ok(())
    }
    fn number(&mut self, value: i32, context: &mut Interpreter<'_, '_, '_>) -> Result<(), Error> {
        context.work.step(1)?;
        let mut metric = Metric::default();
        context.work.node(&mut metric)?;
        context.work.check(
            Resource::Depth,
            1,
            context.work.limits.max_symbol_depth as u128,
        )?;
        self.push(metric, context, |context| context.number(value))
    }
    fn append(
        &mut self,
        other: &mut Self,
        context: &mut Interpreter<'_, '_, '_>,
    ) -> Result<(), Error> {
        context.work.step(other.len() as u128)?;
        self.metrics
            .try_reserve(other.len())
            .map_err(|_| context.work.error(ErrorKind::Allocation))?;
        let start = self.len();
        let end = start
            .checked_add(other.len())
            .ok_or_else(|| context.work.error(ErrorKind::Allocation))?;
        self.terms
            .resize_with(end, context.work.limits.max_term_storage_bytes, || {
                context.work.step(1)
            })
            .map_err(|error| super::binding::failure(error, context.work))?;
        for index in 0..other.len() {
            let key = other.key(index, context)?;
            self.terms
                .set_with(start + index, &key, || context.work.step(1))
                .map_err(|error| super::binding::failure(error, context.work))?;
        }
        // Until every ID copy succeeds, the original alternative owner retains
        // all logical charges. Partly filled inactive slots are metadata only.
        self.bytes += other.bytes;
        other.bytes = 0;
        self.metrics.append(&mut other.metrics);
        Ok(())
    }
}
fn number(key: &TermKey, context: &Interpreter<'_, '_, '_>) -> Result<i32, Error> {
    match context.value(key).descriptor() {
        ValueNodeRef::Number(value) => Ok(value),
        _ => Err(context
            .work
            .error(ErrorKind::Evaluation(EvaluationError::Undefined))),
    }
}
pub(super) fn with<'input, T>(
    term: &Template,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
    action: impl FnOnce(&mut Values, &mut Interpreter<'input, '_, '_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut values = collect(term, binding, context)?;
    let result = action(&mut values, context);
    context.work.local_bytes -= values.bytes;
    result
}
fn unary<'input>(
    operator: Option<UnaryOp>,
    argument: &Template,
    binding: &Binding<'input>,
    out: &mut Values,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<(), Error> {
    with(argument, binding, context, |values, context| {
        for index in 0..values.len() {
            context.work.step(1)?;
            let key = values.key(index, context)?;
            if operator == Some(UnaryOp::Negate)
                && matches!(
                    context.value(&key).descriptor(),
                    ValueNodeRef::Function { .. } | ValueNodeRef::Symbol(_)
                )
            {
                out.push(values.metric(index), context, |context| {
                    context.negate(&key)
                })?;
            } else {
                let value = number(&key, context)?;
                let value = match operator {
                    Some(operator) => crate::scalar_arithmetic::unary(operator, value),
                    None => crate::scalar_arithmetic::absolute(value),
                }
                .map_err(|cause| context.work.error(ErrorKind::Evaluation(cause)))?;
                out.number(value, context)?;
            }
        }
        Ok(())
    })
}
fn binary<'input>(
    operator: Option<BinaryOp>,
    left: &Template,
    right: &Template,
    binding: &Binding<'input>,
    out: &mut Values,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<(), Error> {
    with(left, binding, context, |left, context| {
        with(right, binding, context, |right, context| {
            for left_index in 0..left.len() {
                for right_index in 0..right.len() {
                    context.work.step(1)?;
                    let left_key = left.key(left_index, context)?;
                    let right_key = right.key(right_index, context)?;
                    let left = number(&left_key, context)?;
                    let right = number(&right_key, context)?;
                    if let Some(operator) = operator {
                        let result = crate::scalar_arithmetic::binary(operator, left, right)
                            .map_err(|cause| context.work.error(ErrorKind::Evaluation(cause)))?;
                        out.number(result, context)?;
                    } else {
                        for value in i64::from(left)..=i64::from(right) {
                            context.work.step(1)?;
                            out.number(
                                i32::try_from(value).expect("within source i32 endpoints"),
                                context,
                            )?;
                        }
                    }
                }
            }
            Ok(())
        })
    })
}
fn products(
    shape: crate::metadata::Constructor,
    arguments: &[Values],
    out: &mut Values,
    context: &mut Interpreter<'_, '_, '_>,
) -> Result<(), Error> {
    if arguments.iter().any(|values| values.len() == 0) {
        return Ok(());
    }
    let mut indices = context.work.reserve(arguments.len())?;
    indices.resize(arguments.len(), 0);
    let mut children = context.terms.read().assignment();
    children
        .resize_with(
            arguments.len(),
            context.work.limits.max_term_storage_bytes,
            || context.work.step(1),
        )
        .map_err(|error| super::binding::failure(error, context.work))?;
    let mut slots = context.work.reserve(arguments.len())?;
    slots.extend(0..arguments.len());
    loop {
        context.work.step(1 + indices.len() as u128)?;
        let mut metric = Metric { nodes: 1, bytes: 0 };
        context.work.check(
            Resource::Nodes,
            1,
            context.work.limits.max_symbol_nodes as u128,
        )?;
        context.work.check(
            Resource::Depth,
            1,
            context.work.limits.max_symbol_depth as u128,
        )?;
        context.work.step(1)?;
        let descriptor = context
            .metadata
            .constructor(shape)
            .ok_or_else(|| context.work.error(ErrorKind::InvalidSymbol))?;
        if let ValueNodeRef::Function { name, .. } = descriptor {
            context.work.payload(name.len(), &mut metric)?;
        }
        context.work.step(1)?;
        let declaration = context
            .metadata
            .constructor_declaration(shape)
            .ok_or_else(|| context.work.error(ErrorKind::InvalidSymbol))?;
        for (position, (values, &index)) in arguments.iter().zip(&indices).enumerate() {
            let key = values.key(index, context)?;
            context.measure_key(&key, 2, &mut metric)?;
            children
                .set_with(position, &key, || context.work.step(1))
                .map_err(|error| super::binding::failure(error, context.work))?;
        }
        out.push(metric, context, |context| {
            context.build(&declaration, children.as_slice(), &slots)
        })?;
        let mut position = indices.len();
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < arguments[position].len() {
                break;
            }
            indices[position] = 0;
        }
    }
}
fn constructor<'input>(
    shape: crate::metadata::Constructor,
    arguments: &[Template],
    binding: &Binding<'input>,
    out: &mut Values,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<(), Error> {
    let mut values = context.work.reserve(arguments.len())?;
    let result = (|| {
        for argument in arguments {
            values.push(collect(argument, binding, context)?);
        }
        products(shape, &values, out, context)
    })();
    context.work.local_bytes -= values.iter().map(|values| values.bytes).sum::<u128>();
    result
}
pub(super) fn collect<'input>(
    term: &Template,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<Values, Error> {
    context.work.step(1)?;
    let mut out = Values::new(context);
    let result = (|| {
        if !term.multiple() {
            let mut metric = Metric::default();
            context.measure(term, binding, 1, &mut metric)?;
            return out.push(metric, context, |context| context.construct(term, binding));
        }
        match term {
            Template::Pool(arguments) => {
                for argument in arguments {
                    with(argument, binding, context, |values, context| {
                        out.append(values, context)
                    })?;
                }
                Ok(())
            }
            Template::Interval(left, right) => {
                binary(None, left, right, binding, &mut out, context)
            }
            Template::Binary(operator, left, right) => {
                binary(Some(*operator), left, right, binding, &mut out, context)
            }
            Template::Unary(operator, argument) => {
                unary(Some(*operator), argument, binding, &mut out, context)
            }
            Template::Absolute(argument) => unary(None, argument, binding, &mut out, context),
            Template::Construct(shape, arguments) => {
                constructor(*shape, arguments, binding, &mut out, context)
            }
            Template::Constant(_) | Template::Variable(_) => {
                unreachable!("scalar expression handled above")
            }
        }
    })();
    match result {
        Ok(()) => Ok(out),
        Err(error) => {
            context.work.local_bytes -= out.bytes;
            Err(error)
        }
    }
}

pub(super) fn each<'input>(
    term: &Template,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
    mut action: impl FnMut(TermKey, Metric, &mut Interpreter<'input, '_, '_>) -> Result<(), Error>,
) -> Result<(), Error> {
    if !term.multiple() {
        let mut metric = Metric::default();
        context.measure(term, binding, 1, &mut metric)?;
        context.work.construction_check(metric)?;
        let key = context.construct(term, binding)?;
        return action(key, metric, context);
    }
    with(term, binding, context, |values, context| {
        for index in 0..values.len() {
            let key = values.key(index, context)?;
            let metric = values.metric(index);
            values.bytes -= metric.payload();
            context.work.local_bytes -= metric.payload();
            action(key, metric, context)?;
        }
        Ok(())
    })
}
