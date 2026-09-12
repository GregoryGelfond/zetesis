//! Complete model-relative aggregate keys, owned only for the current scope.

use themelios_program::program::AggregateFunction;

use super::{Bound, Error, ErrorKind, EvaluationError, Metric, ModelRows, Symbol, Work, visit};
use crate::observation::{AggregateQuery, Resource};

pub(super) enum Value {
    Integer(i128),
    Symbol(Symbol),
}
impl Value {
    pub(super) fn compare(&self, bound: &Symbol) -> std::cmp::Ordering {
        match (self, bound) {
            (Self::Integer(value), Symbol::Number(bound)) => value.cmp(&i128::from(*bound)),
            (Self::Integer(_), bound) => Symbol::Number(0).cmp(bound),
            (Self::Symbol(value), bound) => value.cmp(bound),
        }
    }
}
fn eligible(function: AggregateFunction, tuple: &Symbol) -> bool {
    let Symbol::Tuple(tuple) = tuple else {
        unreachable!("compiled aggregate key is a tuple")
    };
    match (function, tuple.first()) {
        (AggregateFunction::SumPlus, Some(Symbol::Number(value))) => *value > 0,
        (AggregateFunction::Count, _)
        | (AggregateFunction::Sum, Some(Symbol::Number(_)))
        | (AggregateFunction::Min | AggregateFunction::Max, Some(_)) => true,
        (AggregateFunction::Min | AggregateFunction::Max, None) => {
            unreachable!("admission rejects missing extremum measures")
        }
        _ => false,
    }
}
fn key(
    symbol: Symbol,
    metric: Metric,
    keys: &mut Vec<(Symbol, Metric)>,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let mut lower = 0;
    let mut upper = keys.len();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        work.step(metric.payload() + keys[middle].1.payload() + 1)?;
        match symbol.cmp(&keys[middle].0) {
            std::cmp::Ordering::Equal => return Ok(()),
            std::cmp::Ordering::Less => upper = middle,
            std::cmp::Ordering::Greater => lower = middle + 1,
        }
    }
    work.check(
        Resource::LocalBytes,
        work.local_bytes + metric.payload(),
        work.limits.max_local_bytes as u128,
    )?;
    work.step((keys.len() - lower) as u128 + 1)?;
    keys.try_reserve(1)
        .map_err(|_| work.error(ErrorKind::Allocation))?;
    work.local_bytes += metric.payload();
    keys.insert(lower, (symbol, metric));
    Ok(())
}
fn reduce(
    function: AggregateFunction,
    keys: &[(Symbol, Metric)],
    work: &mut Work<'_>,
) -> Result<Value, Error> {
    if function == AggregateFunction::Count {
        work.step(keys.len() as u128)?;
        return Ok(Value::Integer(keys.len() as i128));
    }
    let mut total = 0_i128;
    let mut extremum = None;
    for (key, metric) in keys {
        work.step(metric.payload())?;
        let Symbol::Tuple(tuple) = key else {
            unreachable!("compiled aggregate key is a tuple")
        };
        let value = tuple.first().expect("eligible non-count tuple has a head");
        match function {
            AggregateFunction::Sum | AggregateFunction::SumPlus => {
                let Symbol::Number(value) = value else {
                    unreachable!("eligible sum tuple has a numeric head")
                };
                if function == AggregateFunction::Sum || *value > 0 {
                    total = total.checked_add(i128::from(*value)).ok_or_else(|| {
                        work.error(ErrorKind::Evaluation(EvaluationError::Overflow))
                    })?;
                }
            }
            AggregateFunction::Min | AggregateFunction::Max => {
                if extremum.is_none_or(|prior: &Symbol| {
                    if function == AggregateFunction::Min {
                        value < prior
                    } else {
                        value > prior
                    }
                }) {
                    extremum = Some(value);
                }
            }
            AggregateFunction::Count => unreachable!("count handled before tuple inspection"),
        }
    }
    match function {
        AggregateFunction::Sum | AggregateFunction::SumPlus => Ok(Value::Integer(total)),
        AggregateFunction::Min | AggregateFunction::Max => {
            if let Some(value) = extremum {
                let mut metric = Metric::default();
                work.symbol_check(value, 1, &mut metric)?;
                work.construction_check(metric)?;
                work.copy_symbol(value).map(Value::Symbol)
            } else if function == AggregateFunction::Min {
                Ok(Value::Symbol(Symbol::Supremum))
            } else {
                Ok(Value::Symbol(Symbol::Infimum))
            }
        }
        AggregateFunction::Count => unreachable!("count handled before tuple inspection"),
    }
}
pub(super) fn aggregate(
    query: &AggregateQuery,
    atoms: &ModelRows<'_>,
    outer: &[Option<Bound<'_>>],
    work: &mut Work<'_>,
) -> Result<(Value, Metric), Error> {
    let mut keys = Vec::new();
    let result = (|| {
        for element in &query.elements {
            visit(&element.query, atoms, outer, work, &mut |binding, work| {
                super::values::each(&element.tuple, binding, work, |tuple, metric, work| {
                    if eligible(query.function, &tuple) {
                        key(tuple, metric, &mut keys, work)?;
                    }
                    Ok(())
                })?;
                Ok(true)
            })?;
        }
        let value = reduce(query.function, &keys, work)?;
        let mut metric = Metric::default();
        match &value {
            Value::Symbol(value) => work.symbol_check(value, 1, &mut metric)?,
            Value::Integer(_) => metric.nodes = 1,
        }
        Ok((value, metric))
    })();
    work.local_bytes -= keys
        .iter()
        .map(|(_, metric)| metric.payload())
        .sum::<u128>();
    result
}
