//! Complete model-relative aggregate keys. Tuple roots retain canonical IDs;
//! atom-pattern keys retain typed wildcard topology, never synthetic ASP terms.
use super::{
    Binding, DefaultNegation, Error, ErrorKind, EvaluationError, Interpreter, Metric, ModelRows,
    Resource, anonymous, visit,
};
use crate::observation::{AggregateKey, AggregateQuery};
use std::cmp::Ordering;
use themelios_program::program::AggregateFunction;
use zetesis_core::{
    ValueNodeRef,
    catalog::{TermAssignment, TermKey},
};

pub(super) enum Value {
    Integer(i128),
    Term(TermKey),
}
impl Value {
    pub(super) fn compare(
        &self,
        bound: &TermKey,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<Ordering, Error> {
        match self {
            Self::Integer(value) => {
                ctx.work.step(1)?;
                let term = ctx.value(bound);
                match term.descriptor() {
                    ValueNodeRef::Number(bound) => Ok(value.cmp(&i128::from(bound))),
                    ValueNodeRef::Infimum => Ok(Ordering::Greater),
                    _ => Ok(Ordering::Less),
                }
            }
            Self::Term(value) => ctx.compare(value, bound),
        }
    }
}
#[derive(Clone, Copy)]
enum Key {
    Tuple(usize),
    Atom(DefaultNegation, anonymous::Key),
}
struct Keys {
    terms: TermAssignment,
    values: Vec<(Key, Metric)>,
    bytes: u128,
}
impl Keys {
    fn new(ctx: &Interpreter<'_, '_, '_>) -> Self {
        Self {
            terms: ctx.terms.read().assignment(),
            values: Vec::new(),
            bytes: 0,
        }
    }
    fn compare(
        &self,
        left: Key,
        right: Key,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<Ordering, Error> {
        match (left, right) {
            (Key::Tuple(a), Key::Tuple(b)) => {
                let a = self.terms.key(a).expect("tuple slot").expect("tuple root");
                let b = self.terms.key(b).expect("tuple slot").expect("tuple root");
                ctx.compare(&a, &b)
            }
            (Key::Atom(a, ka), Key::Atom(b, kb)) => {
                ctx.work.step(1)?;
                let order = negation(a).cmp(&negation(b));
                if order.is_eq() {
                    anonymous::compare(ka, kb, ctx)
                } else {
                    Ok(order)
                }
            }
            (Key::Tuple(_), Key::Atom(..)) => Ok(Ordering::Less),
            (Key::Atom(..), Key::Tuple(_)) => Ok(Ordering::Greater),
        }
    }
    fn tuple(
        &mut self,
        key: &TermKey,
        metric: Metric,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<(), Error> {
        let mut lower = 0;
        let mut upper = self.values.len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let order = match self.values[middle].0 {
                Key::Tuple(slot) => {
                    let other = self
                        .terms
                        .key(slot)
                        .expect("tuple slot")
                        .expect("tuple root");
                    ctx.compare(key, &other)?
                }
                Key::Atom(..) => Ordering::Less,
            };
            match order {
                Ordering::Equal => return Ok(()),
                Ordering::Less => upper = middle,
                Ordering::Greater => lower = middle + 1,
            }
        }
        self.admit(metric, lower, ctx)?;
        let slot = self.terms.len();
        self.terms
            .resize_with(slot + 1, ctx.work.limits.max_term_storage_bytes, || {
                ctx.work.step(1)
            })
            .map_err(|error| super::binding::failure(error, ctx.work))?;
        ctx.set(&mut self.terms, slot, key)?;
        self.retain(Key::Tuple(slot), metric, lower, ctx);
        Ok(())
    }
    fn insert(
        &mut self,
        key: Key,
        metric: Metric,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<(), Error> {
        let mut lower = 0;
        let mut upper = self.values.len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            match self.compare(key, self.values[middle].0, ctx)? {
                Ordering::Equal => return Ok(()),
                Ordering::Less => upper = middle,
                Ordering::Greater => lower = middle + 1,
            }
        }
        self.admit(metric, lower, ctx)?;
        self.retain(key, metric, lower, ctx);
        Ok(())
    }
    fn admit(
        &mut self,
        metric: Metric,
        lower: usize,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) -> Result<(), Error> {
        ctx.work.check(
            Resource::LocalBytes,
            ctx.work.local_bytes + metric.payload(),
            ctx.work.limits.max_local_bytes as u128,
        )?;
        ctx.work.step((self.values.len() - lower) as u128 + 1)?;
        self.values
            .try_reserve(1)
            .map_err(|_| ctx.work.error(ErrorKind::Allocation))
    }
    fn retain(
        &mut self,
        key: Key,
        metric: Metric,
        lower: usize,
        ctx: &mut Interpreter<'_, '_, '_>,
    ) {
        ctx.work.local_bytes += metric.payload();
        self.bytes += metric.payload();
        self.values.insert(lower, (key, metric));
    }
}
fn negation(negation: DefaultNegation) -> u8 {
    match negation {
        DefaultNegation::None => 0,
        DefaultNegation::Not => 1,
        DefaultNegation::NotNot => 2,
    }
}
fn eligible(
    function: AggregateFunction,
    key: &TermKey,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<bool, Error> {
    ctx.work.step(1)?;
    let tuple = ctx.value(key);
    assert!(
        matches!(tuple.descriptor(), ValueNodeRef::Tuple { .. }),
        "compiled aggregate tuple"
    );
    let first = tuple
        .child(0)
        .map(zetesis_core::catalog::TermRef::descriptor);
    Ok(match (function, first) {
        (AggregateFunction::SumPlus, Some(ValueNodeRef::Number(value))) => value > 0,
        (AggregateFunction::Count, _)
        | (AggregateFunction::Sum, Some(ValueNodeRef::Number(_)))
        | (AggregateFunction::Min | AggregateFunction::Max, Some(_)) => true,
        (AggregateFunction::Min | AggregateFunction::Max, None) => {
            unreachable!("admission rejects empty extremum keys")
        }
        _ => false,
    })
}
fn reduce(
    function: AggregateFunction,
    keys: &Keys,
    ctx: &mut Interpreter<'_, '_, '_>,
) -> Result<Value, Error> {
    if function == AggregateFunction::Count {
        ctx.work.step(keys.values.len() as u128)?;
        return Ok(Value::Integer(keys.values.len() as i128));
    }
    let mut total = 0_i128;
    let mut extremum = None;
    for (key, _) in &keys.values {
        ctx.work.step(1)?;
        let Key::Tuple(slot) = key else {
            unreachable!("atom sets measure cardinality");
        };
        let tuple = keys
            .terms
            .key(*slot)
            .expect("tuple slot")
            .expect("tuple root");
        let value = ctx
            .child(&tuple, 0)?
            .expect("eligible non-count tuple has a measure");
        match function {
            AggregateFunction::Sum | AggregateFunction::SumPlus => {
                let ValueNodeRef::Number(number) = ctx.value(&value).descriptor() else {
                    unreachable!("eligible numeric key");
                };
                total = total.checked_add(i128::from(number)).ok_or_else(|| {
                    ctx.work
                        .error(ErrorKind::Evaluation(EvaluationError::Overflow))
                })?;
            }
            AggregateFunction::Min | AggregateFunction::Max => {
                let replace = if let Some(prior) = &extremum {
                    let order = ctx.compare(&value, prior)?;
                    if function == AggregateFunction::Min {
                        order.is_lt()
                    } else {
                        order.is_gt()
                    }
                } else {
                    true
                };
                if replace {
                    extremum = Some(value);
                }
            }
            AggregateFunction::Count => unreachable!("count handled above"),
        }
    }
    match function {
        AggregateFunction::Sum | AggregateFunction::SumPlus => Ok(Value::Integer(total)),
        AggregateFunction::Min | AggregateFunction::Max => Ok(Value::Term(match extremum {
            Some(value) => value,
            None => ctx.scalar(if function == AggregateFunction::Min {
                ValueNodeRef::Supremum
            } else {
                ValueNodeRef::Infimum
            })?,
        })),
        AggregateFunction::Count => unreachable!("count handled above"),
    }
}
pub(super) fn aggregate<'input>(
    query: &AggregateQuery,
    atoms: &ModelRows<'input>,
    outer: &Binding<'input>,
    ctx: &mut Interpreter<'input, '_, '_>,
) -> Result<(Value, Metric), Error> {
    let mut keys = Keys::new(ctx);
    let result = (|| {
        for element in &query.elements {
            visit(
                &element.query,
                atoms,
                Some(outer),
                ctx,
                &mut |binding, ctx| {
                    match &element.key {
                        AggregateKey::Tuple(tuple) => {
                            super::values::each(tuple, binding, ctx, |tuple, metric, ctx| {
                                if eligible(query.function, &tuple, ctx)? {
                                    keys.tuple(&tuple, metric, ctx)?;
                                }
                                Ok(())
                            })?;
                        }
                        AggregateKey::Atom { negation, slot } => {
                            let key = binding
                                .pattern(*slot)
                                .expect("set element retains its complete atom key");
                            keys.insert(
                                Key::Atom(*negation, key),
                                anonymous::metric(key, ctx),
                                ctx,
                            )?;
                        }
                    }
                    Ok(true)
                },
            )?;
        }
        let value = reduce(query.function, &keys, ctx)?;
        let metric = match &value {
            Value::Integer(_) => Metric { nodes: 1, bytes: 0 },
            Value::Term(key) => ctx.metric(key)?,
        };
        Ok((value, metric))
    })();
    ctx.work.local_bytes -= keys.bytes;
    result
}
