//! Bounded nested matching binds only explicit constructor/tuple variable positions.

use super::{Bound, Error, Metric, Operand, Reference, Resource, Symbol, Value, Work, resolve};

pub(super) fn slots(operand: &Operand) -> usize {
    match operand {
        Operand::Variable(_) => 1,
        Operand::Function(_, _, arguments) | Operand::Tuple(arguments) => {
            arguments.iter().map(slots).sum()
        }
        _ => 0,
    }
}
fn symbol(value: &Value, work: &mut Work<'_>) -> Result<Symbol, Error> {
    let mut metric = Metric::default();
    work.measure_reference(Reference::Value(value), 1, &mut metric)?;
    work.construction_check(metric)?;
    work.copy_reference(Reference::Value(value))
}
pub(super) fn own(value: &Symbol, work: &mut Work<'_>) -> Result<(Symbol, Metric), Error> {
    let mut metric = Metric::default();
    work.symbol_check(value, 1, &mut metric)?;
    work.construction_check(metric)?;
    work.check(
        Resource::LocalBytes,
        work.local_bytes + metric.payload(),
        work.limits.max_local_bytes as u128,
    )?;
    let value = work.copy_symbol(value)?;
    work.local_bytes += metric.payload();
    Ok((value, metric))
}
type Children<'a> = (&'a [Operand], &'a [Symbol]);

fn children<'a>(
    pattern: &'a Operand,
    value: &'a Symbol,
    work: &mut Work<'_>,
) -> Result<Option<Children<'a>>, Error> {
    Ok(match (pattern, value) {
        (
            Operand::Function(sign, name, arguments),
            Symbol::Function {
                sign: actual_sign,
                name: actual_name,
                arguments: values,
            },
        ) => {
            work.step(name.as_str().len() as u128 + actual_name.as_str().len() as u128)?;
            (*sign == *actual_sign && name == actual_name && arguments.len() == values.len())
                .then_some((arguments, values))
        }
        (Operand::Tuple(arguments), Symbol::Tuple(values)) if arguments.len() == values.len() => {
            Some((arguments, values))
        }
        _ => None,
    })
}
fn matches_symbol(
    pattern: &Operand,
    value: &Symbol,
    binding: &mut [Option<Bound<'_>>],
    undo: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    if let Some(expected) = resolve(pattern, binding) {
        return Ok(work
            .compare_reference(expected, Reference::Symbol(value))?
            .is_eq());
    }
    match pattern {
        Operand::Variable(slot) => {
            let (value, metric) = own(value, work)?;
            binding[*slot] = Some(Bound::Owned(value, metric));
            undo.push(*slot);
            Ok(true)
        }
        Operand::Any => Ok(true),
        Operand::Expression(expression) => expression_matches(expression, value, binding, work),
        Operand::Function(_, _, _) | Operand::Tuple(_) => {
            let Some((arguments, values)) = children(pattern, value, work)? else {
                return Ok(false);
            };
            for (argument, value) in arguments.iter().zip(values) {
                if !matches_symbol(argument, value, binding, undo, work)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Operand::Value(_) => unreachable!("constant was resolved"),
    }
}
fn expression_matches(
    expression: &super::Template,
    value: &Symbol,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    let mut metric = Metric::default();
    work.measure(expression, binding, 1, &mut metric)?;
    let mut actual = Metric::default();
    work.symbol_check(value, 1, &mut actual)?;
    work.construction_check(Metric {
        nodes: metric.nodes + actual.nodes,
        bytes: metric.bytes + actual.bytes,
    })?;
    let expected = work.construct(expression, binding)?;
    work.step(metric.payload() + actual.payload())?;
    Ok(expected == *value)
}
fn test_symbol(
    pattern: &Operand,
    value: &Symbol,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    work.step(1)?;
    if let Some(expected) = resolve(pattern, binding) {
        return Ok(work
            .compare_reference(expected, Reference::Symbol(value))?
            .is_eq());
    }
    match pattern {
        Operand::Any => Ok(true),
        Operand::Expression(expression) => expression_matches(expression, value, binding, work),
        Operand::Function(_, _, _) | Operand::Tuple(_) => {
            let Some((arguments, values)) = children(pattern, value, work)? else {
                return Ok(false);
            };
            for (argument, value) in arguments.iter().zip(values) {
                if !test_symbol(argument, value, binding, work)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Operand::Variable(_) | Operand::Value(_) => unreachable!("safe operand was resolved"),
    }
}
pub(super) fn matches_value(
    pattern: &Operand,
    value: &Value,
    binding: &mut [Option<Bound<'_>>],
    undo: &mut Vec<usize>,
    bind: bool,
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    if matches!(pattern, Operand::Any) {
        return Ok(true);
    }
    let value = symbol(value, work)?;
    if bind {
        matches_symbol(pattern, &value, binding, undo, work)
    } else {
        test_symbol(pattern, &value, binding, work)
    }
}
pub(super) fn test_value(
    pattern: &Operand,
    value: &Value,
    binding: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<bool, Error> {
    if let Some(expected) = resolve(pattern, binding) {
        return Ok(work
            .compare_reference(expected, Reference::Value(value))?
            .is_eq());
    }
    if matches!(pattern, Operand::Any) {
        return Ok(true);
    }
    let value = symbol(value, work)?;
    test_symbol(pattern, &value, binding, work)
}
