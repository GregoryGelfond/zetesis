//! Finite expression alternatives with an explicit live owned-payload budget.

use themelios_program::term::{BinaryOp, UnaryOp};

use super::{Bound, Error, ErrorKind, EvaluationError, Metric, Resource, Symbol, Template, Work};

#[derive(Default)]
pub(super) struct Values {
    pub values: Vec<(Symbol, Metric)>,
    pub bytes: u128,
}
impl Values {
    fn push(
        &mut self,
        metric: Metric,
        work: &mut Work<'_>,
        make: impl FnOnce(&mut Work<'_>) -> Result<Symbol, Error>,
    ) -> Result<(), Error> {
        work.check(
            Resource::LocalBytes,
            work.local_bytes + metric.payload(),
            work.limits.max_local_bytes as u128,
        )?;
        work.construction_check(metric)?;
        work.step(1)?;
        self.values
            .try_reserve(1)
            .map_err(|_| work.error(ErrorKind::Allocation))?;
        let symbol = make(work)?;
        self.bytes += metric.payload();
        work.local_bytes += metric.payload();
        self.values.push((symbol, metric));
        Ok(())
    }
    fn number(&mut self, value: i32, work: &mut Work<'_>) -> Result<(), Error> {
        let symbol = Symbol::Number(value);
        let mut metric = Metric::default();
        work.symbol_check(&symbol, 1, &mut metric)?;
        self.push(metric, work, |_| Ok(symbol))
    }
    fn append(&mut self, other: &mut Self, work: &mut Work<'_>) -> Result<(), Error> {
        work.step(other.values.len() as u128)?;
        self.values
            .try_reserve(other.values.len())
            .map_err(|_| work.error(ErrorKind::Allocation))?;
        self.bytes += other.bytes;
        other.bytes = 0;
        self.values.append(&mut other.values);
        Ok(())
    }
}
fn number(value: &Symbol, work: &Work<'_>) -> Result<i32, Error> {
    if let Symbol::Number(value) = value {
        Ok(*value)
    } else {
        Err(work.error(ErrorKind::Evaluation(EvaluationError::Undefined)))
    }
}
pub(super) fn with<T>(
    term: &Template,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
    action: impl FnOnce(&mut Values, &mut Work<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut values = collect(term, binding, work)?;
    let result = action(&mut values, work);
    work.local_bytes -= values.bytes;
    result
}
fn unary(
    operator: Option<UnaryOp>,
    argument: &Template,
    binding: &[Option<Bound<'_>>],
    out: &mut Values,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    with(argument, binding, work, |values, work| {
        for (symbol, metric) in &values.values {
            work.step(1)?;
            if operator == Some(UnaryOp::Negate) && matches!(symbol, Symbol::Function { .. }) {
                out.push(*metric, work, |work| {
                    let mut result = work.copy_symbol(symbol)?;
                    let Symbol::Function { sign, .. } = &mut result else {
                        unreachable!()
                    };
                    *sign = match sign {
                        super::Sign::Positive => super::Sign::Negative,
                        super::Sign::Negative => super::Sign::Positive,
                    };
                    Ok(result)
                })?;
            } else {
                let value = number(symbol, work)?;
                let value = match operator {
                    Some(operator) => crate::scalar_arithmetic::unary(operator, value),
                    None => crate::scalar_arithmetic::absolute(value),
                }
                .map_err(|cause| work.error(ErrorKind::Evaluation(cause)))?;
                out.number(value, work)?;
            }
        }
        Ok(())
    })
}
fn binary(
    operator: Option<BinaryOp>,
    left: &Template,
    right: &Template,
    binding: &[Option<Bound<'_>>],
    out: &mut Values,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    with(left, binding, work, |left, work| {
        with(right, binding, work, |right, work| {
            for (left, _) in &left.values {
                for (right, _) in &right.values {
                    work.step(1)?;
                    let left = number(left, work)?;
                    let right = number(right, work)?;
                    if let Some(operator) = operator {
                        let result = crate::scalar_arithmetic::binary(operator, left, right)
                            .map_err(|cause| work.error(ErrorKind::Evaluation(cause)))?;
                        out.number(result, work)?;
                    } else {
                        for value in i64::from(left)..=i64::from(right) {
                            work.step(1)?;
                            out.number(
                                i32::try_from(value).expect("within source i32 endpoints"),
                                work,
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
    term: &Template,
    arguments: &[Values],
    out: &mut Values,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    if arguments.iter().any(|values| values.values.is_empty()) {
        return Ok(());
    }
    let mut indices = work.reserve(arguments.len())?;
    indices.resize(arguments.len(), 0);
    loop {
        work.step(1 + indices.len() as u128)?;
        let mut metric = Metric { nodes: 1, bytes: 0 };
        work.check(Resource::Nodes, 1, work.limits.max_symbol_nodes as u128)?;
        work.check(Resource::Depth, 1, work.limits.max_symbol_depth as u128)?;
        if let Template::Function(_, name, _) = term {
            work.payload(name.as_str().len(), &mut metric)?;
        }
        for (values, &index) in arguments.iter().zip(&indices) {
            work.symbol_check(&values.values[index].0, 2, &mut metric)?;
        }
        out.push(metric, work, |work| {
            let mut values = work.reserve(arguments.len())?;
            for (argument, &index) in arguments.iter().zip(&indices) {
                values.push(work.copy_symbol(&argument.values[index].0)?);
            }
            Ok(if let Template::Function(sign, name, _) = term {
                Symbol::Function {
                    sign: *sign,
                    name: name.clone(),
                    arguments: values,
                }
            } else {
                Symbol::Tuple(values)
            })
        })?;
        let mut position = indices.len();
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < arguments[position].values.len() {
                break;
            }
            indices[position] = 0;
        }
    }
}
fn constructor(
    term: &Template,
    arguments: &[Template],
    binding: &[Option<Bound<'_>>],
    out: &mut Values,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let mut values = work.reserve(arguments.len())?;
    let result = (|| {
        for argument in arguments {
            values.push(collect(argument, binding, work)?);
        }
        products(term, &values, out, work)
    })();
    work.local_bytes -= values.iter().map(|values| values.bytes).sum::<u128>();
    result
}
pub(super) fn collect(
    term: &Template,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<Values, Error> {
    work.step(1)?;
    let mut out = Values::default();
    let result = (|| {
        if !term.multiple() {
            let mut metric = Metric::default();
            work.measure(term, binding, 1, &mut metric)?;
            return out.push(metric, work, |work| work.construct(term, binding));
        }
        match term {
            Template::Pool(arguments) => {
                for argument in arguments {
                    with(argument, binding, work, |values, work| {
                        out.append(values, work)
                    })?;
                }
                Ok(())
            }
            Template::Interval(left, right) => binary(None, left, right, binding, &mut out, work),
            Template::Binary(operator, left, right) => {
                binary(Some(*operator), left, right, binding, &mut out, work)
            }
            Template::Unary(operator, argument) => {
                unary(Some(*operator), argument, binding, &mut out, work)
            }
            Template::Absolute(argument) => unary(None, argument, binding, &mut out, work),
            Template::Function(_, _, arguments) | Template::Tuple(arguments) => {
                constructor(term, arguments, binding, &mut out, work)
            }
            Template::Value(_) | Template::Variable(_) => {
                unreachable!("scalar expression handled above")
            }
        }
    })();
    match result {
        Ok(()) => Ok(out),
        Err(error) => {
            work.local_bytes -= out.bytes;
            Err(error)
        }
    }
}

pub(super) fn each(
    term: &Template,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
    mut action: impl FnMut(Symbol, Metric, &mut Work<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    if !term.multiple() {
        let mut metric = Metric::default();
        work.measure(term, binding, 1, &mut metric)?;
        work.construction_check(metric)?;
        let symbol = work.construct(term, binding)?;
        return action(symbol, metric, work);
    }
    with(term, binding, work, |values, work| {
        for (symbol, metric) in values.values.drain(..) {
            values.bytes -= metric.payload();
            work.local_bytes -= metric.payload();
            action(symbol, metric, work)?;
        }
        Ok(())
    })
}
