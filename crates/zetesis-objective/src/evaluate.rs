use std::cmp::Ordering;
use std::collections::btree_set;

use zetesis_core::{Atom, AtomPattern, Filter, Model, Term, Value};
use zetesis_cpu::Control;

use crate::{
    Condition, ConditionNode, Contribution, Error, ErrorKind, Evaluation, Limits, ObjectiveProgram,
    ObjectiveTemplate, Score, Statistics, Stop,
};

struct Work<'a> {
    limits: Limits,
    control: &'a Control,
    statistics: Statistics,
    template: Option<usize>,
}
impl Work<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            template: self.template,
            statistics: self.statistics,
        }
    }
    fn stop(&self, reason: Stop) -> Error {
        self.error(ErrorKind::Stopped(reason))
    }
    fn tick(&mut self) -> Result<(), Error> {
        self.control.poll().map_err(|reason| {
            self.stop(match reason {
                zetesis_cpu::Stop::Cancelled => Stop::Cancelled,
                zetesis_cpu::Stop::Deadline => Stop::Deadline,
                other => Stop::Control(other),
            })
        })?;
        if self.statistics.work >= self.limits.max_work {
            return Err(self.stop(Stop::WorkLimit));
        }
        self.statistics.work += 1;
        Ok(())
    }
    fn reserve<T>(&self, count: usize) -> Result<Vec<T>, Error> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| self.stop(Stop::Allocation))?;
        Ok(result)
    }
    fn increment(&self, count: u64) -> Result<u64, Error> {
        count
            .checked_add(1)
            .ok_or_else(|| self.stop(Stop::ArithmeticOverflow))
    }
    fn add_size(&self, left: usize, right: usize) -> Result<usize, Error> {
        left.checked_add(right)
            .ok_or_else(|| self.stop(Stop::ArithmeticOverflow))
    }
    fn bytes(&mut self, left: &[u8], right: &[u8]) -> Result<Ordering, Error> {
        for (&a, &b) in left.iter().zip(right) {
            self.tick()?;
            let order = a.cmp(&b);
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        Ok(left.len().cmp(&right.len()))
    }
    fn compare(&mut self, left: &Value, right: &Value) -> Result<Ordering, Error> {
        self.tick()?;
        if matches!(left, Value::Structured(_)) || matches!(right, Value::Structured(_)) {
            for value in [left, right] {
                for _ in 0..value.payload_bytes() {
                    self.tick()?;
                }
            }
            return Ok(left.cmp(right));
        }
        Ok(match (left, right) {
            (Value::Structured(_), _) | (_, Value::Structured(_)) => {
                unreachable!("structural branch handled")
            }
            (Value::Infimum, Value::Infimum) | (Value::Supremum, Value::Supremum) => {
                Ordering::Equal
            }
            (Value::Infimum, _) | (_, Value::Supremum) => Ordering::Less,
            (Value::Supremum, _) | (_, Value::Infimum) => Ordering::Greater,
            (Value::Number(a), Value::Number(b)) => a.cmp(b),
            (Value::String(a), Value::String(b)) | (Value::Symbol(a), Value::Symbol(b)) => {
                return self.bytes(a.as_bytes(), b.as_bytes());
            }
            (Value::Number(_), _) | (Value::String(_), Value::Symbol(_)) => Ordering::Less,
            (_, Value::Number(_)) | (Value::Symbol(_), Value::String(_)) => Ordering::Greater,
        })
    }
    fn resolve<'a>(
        &mut self,
        term: &'a Term,
        binding: &[Option<&'a Value>],
    ) -> Result<&'a Value, Error> {
        self.tick()?;
        match term {
            Term::Constant(value) => Ok(value),
            Term::Variable(variable) => {
                binding.get(*variable).copied().flatten().ok_or_else(|| {
                    self.error(ErrorKind::UnboundVariable {
                        variable: *variable,
                    })
                })
            }
        }
    }
    fn copy_value(&mut self, value: &Value) -> Result<Value, Error> {
        self.tick()?;
        match value {
            Value::Infimum => Ok(Value::Infimum),
            Value::Supremum => Ok(Value::Supremum),
            Value::Number(number) => Ok(Value::Number(*number)),
            Value::Structured(value) => {
                for _ in 0..value.payload_bytes() {
                    self.tick()?;
                }
                Ok(Value::Structured(value.clone()))
            }
            Value::String(text) | Value::Symbol(text) => {
                let mut owned = String::new();
                owned
                    .try_reserve_exact(text.len())
                    .map_err(|_| self.stop(Stop::Allocation))?;
                for character in text.chars() {
                    self.tick()?;
                    owned.push(character);
                }
                Ok(if matches!(value, Value::String(_)) {
                    Value::String(owned)
                } else {
                    Value::Symbol(owned)
                })
            }
        }
    }

    fn contains(&mut self, model: &Model, query: &Atom) -> Result<bool, Error> {
        // A charged scan avoids hiding key comparisons in an opaque set lookup.
        for atom in model.atoms() {
            self.tick()?;
            if query.predicate().sign() != atom.predicate().sign()
                || query.predicate().arity() != atom.predicate().arity()
                || self.bytes(
                    query.predicate().name().as_bytes(),
                    atom.predicate().name().as_bytes(),
                )? != Ordering::Equal
            {
                continue;
            }
            let mut equal = true;
            for (left, right) in query.values().iter().zip(atom.values()) {
                if self.compare(left, right)? != Ordering::Equal {
                    equal = false;
                    break;
                }
            }
            if equal {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn condition(&mut self, condition: &Condition, model: &Model) -> Result<bool, Error> {
        let mut values: Vec<bool> = self.reserve(condition.nodes().len())?;
        for node in condition.nodes() {
            self.tick()?;
            let value = match node {
                ConditionNode::Boolean(value) => *value,
                ConditionNode::Atom(atom) => self.contains(model, atom)?,
                ConditionNode::Not(operand) => !values[*operand],
                ConditionNode::And(left, right) => values[*left] && values[*right],
                ConditionNode::Or(left, right) => values[*left] || values[*right],
            };
            values.push(value);
        }
        Ok(values.last().copied().unwrap_or(true))
    }
}

struct Evaluator<'a> {
    work: Work<'a>,
    keys: Vec<Contribution>,
    totals: Vec<i128>,
}

/// Evaluate objective conditions and joins against the supplied exact true-atom set.
/// The caller alone establishes that this set is a complete stable model.
/// Numeric costs never contribute to support or acceptance.
/// Nonnumeric weights contribute no key or cost. Program priority slots stay
/// fixed; the frontend establishes which slots have possible numeric bindings.
///
/// # Errors
/// Returns typed numeric weight-normalization or cost overflow, invalid bindings,
/// cancellation/deadlines or explicit budget/allocation refusals. No partial
/// score is returned. Source origins remain in the caller's template catalog.
pub fn evaluate(
    program: &ObjectiveProgram,
    model: &Model,
    limits: Limits,
    control: &Control,
) -> Result<Evaluation, Error> {
    let mut work = Work {
        limits,
        control,
        statistics: Statistics::default(),
        template: None,
    };
    work.tick()?;
    let mut totals = work.reserve(program.priorities().len())?;
    for _ in program.priorities() {
        work.tick()?;
        totals.push(0);
    }
    let mut evaluator = Evaluator {
        work,
        keys: Vec::new(),
        totals,
    };
    for (index, template) in program.templates().iter().enumerate() {
        evaluator.work.template = Some(index);
        evaluator.join(
            template,
            model,
            program.variables(index),
            program.slot(index),
        )?;
    }
    evaluator.work.template = None;
    let mut costs = evaluator.work.reserve(program.priorities().len())?;
    for (&priority, &total) in program.priorities().iter().zip(&evaluator.totals) {
        evaluator.work.tick()?;
        costs.push((
            priority,
            final_cost(priority, total).map_err(|kind| evaluator.work.error(kind))?,
        ));
    }
    Ok(Evaluation {
        score: Score {
            present: program.is_present(),
            costs,
        },
        contributions: evaluator.keys,
        statistics: evaluator.work.statistics,
    })
}

fn final_cost(priority: i32, total: i128) -> Result<i64, ErrorKind> {
    i64::try_from(total).map_err(|_| ErrorKind::CostOverflow { priority })
}

struct Frame<'a> {
    atoms: btree_set::Iter<'a, Atom>,
    trail_start: usize,
}

impl Evaluator<'_> {
    fn join<'a>(
        &mut self,
        template: &'a ObjectiveTemplate,
        model: &'a Model,
        variables: usize,
        slot: usize,
    ) -> Result<(), Error> {
        self.work.tick()?;
        if !self.work.condition(template.condition(), model)? {
            return Ok(());
        }
        let mut binding = self.work.reserve(variables)?;
        for _ in 0..variables {
            self.work.tick()?;
            binding.push(None);
        }
        if template.positive().is_empty() {
            return self.active(template, &binding, slot);
        }
        let mut frames = self.work.reserve(template.positive().len())?;
        let mut trail = self.work.reserve(variables)?;
        frames.push(Frame {
            atoms: model.atoms().iter(),
            trail_start: 0,
        });
        while !frames.is_empty() {
            self.work.tick()?;
            let depth = frames.len() - 1;
            while trail.len() > frames[depth].trail_start {
                self.work.tick()?;
                let variable = trail.pop().expect("nonempty undo trail");
                binding[variable] = None;
            }
            let Some(atom) = frames[depth].atoms.next() else {
                frames.pop();
                continue;
            };
            if !self.matches(&template.positive()[depth], atom, &mut binding, &mut trail)? {
                continue;
            }
            if depth + 1 == template.positive().len() {
                self.active(template, &binding, slot)?;
            } else {
                frames.push(Frame {
                    atoms: model.atoms().iter(),
                    trail_start: trail.len(),
                });
            }
        }
        Ok(())
    }

    fn matches<'a>(
        &mut self,
        pattern: &'a AtomPattern,
        atom: &'a Atom,
        binding: &mut [Option<&'a Value>],
        trail: &mut Vec<usize>,
    ) -> Result<bool, Error> {
        self.work.tick()?;
        if pattern.predicate().sign() != atom.predicate().sign()
            || pattern.predicate().arity() != atom.predicate().arity()
            || self.work.bytes(
                pattern.predicate().name().as_bytes(),
                atom.predicate().name().as_bytes(),
            )? != Ordering::Equal
        {
            return Ok(false);
        }
        for (term, value) in pattern.terms().iter().zip(atom.values()) {
            self.work.tick()?;
            match term {
                Term::Constant(constant) => {
                    if self.work.compare(constant, value)? != Ordering::Equal {
                        return Ok(false);
                    }
                }
                Term::Variable(variable) => {
                    if let Some(previous) = binding[*variable] {
                        if self.work.compare(previous, value)? != Ordering::Equal {
                            return Ok(false);
                        }
                    } else {
                        binding[*variable] = Some(value);
                        trail.push(*variable);
                    }
                }
            }
        }
        Ok(true)
    }

    fn active<'a>(
        &mut self,
        template: &'a ObjectiveTemplate,
        binding: &[Option<&'a Value>],
        slot: usize,
    ) -> Result<(), Error> {
        self.work.tick()?;
        if self.work.statistics.bindings >= self.work.limits.max_bindings {
            return Err(self.work.stop(Stop::BindingLimit));
        }
        self.work.statistics.bindings += 1;
        for filter in template.filters() {
            let (left, right) = filter.terms();
            let left = self.work.resolve(left, binding)?;
            let right = self.work.resolve(right, binding)?;
            let equal = self.work.compare(left, right)? == Ordering::Equal;
            if equal != matches!(filter, Filter::Eq(..)) {
                return Ok(());
            }
        }
        self.work.statistics.active_bindings =
            self.work.increment(self.work.statistics.active_bindings)?;
        let Value::Number(weight) = self.work.resolve(template.weight(), binding)? else {
            return Ok(());
        };
        let weight = template
            .weight_polarity()
            .normalize(*weight)
            .ok_or_else(|| self.work.error(ErrorKind::WeightNormalizationOverflow))?;
        let mut tuple = self.work.reserve(template.tuple().len())?;
        for term in template.tuple() {
            tuple.push(self.work.resolve(term, binding)?);
        }
        self.contribute(template.priority(), weight, &tuple, slot)
    }

    fn compare_key(
        &mut self,
        index: usize,
        priority: i32,
        weight: i32,
        tuple: &[&Value],
    ) -> Result<Ordering, Error> {
        self.work.tick()?;
        let key = &self.keys[index];
        let prefix = key.priority.cmp(&priority).then(key.weight.cmp(&weight));
        if prefix != Ordering::Equal {
            return Ok(prefix);
        }
        for (left, right) in key.tuple.iter().zip(tuple) {
            let order = self.work.compare(left, right)?;
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        Ok(key.tuple.len().cmp(&tuple.len()))
    }

    fn contribute(
        &mut self,
        priority: i32,
        weight: i32,
        tuple: &[&Value],
        slot: usize,
    ) -> Result<(), Error> {
        let mut low = 0;
        let mut high = self.keys.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match self.compare_key(middle, priority, weight, tuple)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    self.work.statistics.duplicates =
                        self.work.increment(self.work.statistics.duplicates)?;
                    return Ok(());
                }
            }
        }
        if self.keys.len() >= self.work.limits.max_keys {
            return Err(self.work.stop(Stop::KeyLimit));
        }
        let bytes = self.key_bytes(tuple)?;
        let total_bytes = self.work.add_size(self.work.statistics.key_bytes, bytes)?;
        if total_bytes > self.work.limits.max_key_bytes {
            return Err(self.work.stop(Stop::KeyBytesLimit));
        }
        let mut owned = self.work.reserve(tuple.len())?;
        for value in tuple {
            owned.push(self.work.copy_value(value)?);
        }
        // Sorted storage avoids opaque hash/set allocation. Charge each moved
        // key before Vec::insert's memmove; this intentionally bounds its O(k) work.
        for _ in low..self.keys.len() {
            self.work.tick()?;
        }
        self.keys
            .try_reserve(1)
            .map_err(|_| self.work.stop(Stop::Allocation))?;
        let total = self.totals[slot]
            .checked_add(i128::from(weight))
            .ok_or_else(|| self.work.stop(Stop::ArithmeticOverflow))?;
        self.keys.insert(
            low,
            Contribution {
                priority,
                weight,
                tuple: owned,
            },
        );
        self.totals[slot] = total;
        self.work.statistics.keys = self.keys.len();
        self.work.statistics.key_bytes = total_bytes;
        Ok(())
    }

    fn key_bytes(&mut self, tuple: &[&Value]) -> Result<usize, Error> {
        // Two i32 fields and a u64 tuple length. Each value has a one-byte tag,
        // then an i32 number or a u64 byte length followed by UTF-8 payload.
        // The two extrema have distinct tags and no payload.
        let mut bytes = 16;
        for value in tuple {
            self.work.tick()?;
            let payload = match value {
                Value::Infimum | Value::Supremum => 0,
                Value::Number(_) => 4,
                Value::Structured(value) => value.canonical_bytes() - 1,
                Value::String(text) | Value::Symbol(text) => self.work.add_size(8, text.len())?,
            };
            bytes = self.work.add_size(bytes, self.work.add_size(1, payload)?)?;
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::final_cost;
    use crate::ErrorKind;

    #[test]
    fn final_signed_cost_conversion_accepts_exact_edges_and_refuses_excess() {
        for edge in [i64::MIN, i64::MAX] {
            assert_eq!(final_cost(-7, i128::from(edge)), Ok(edge));
        }
        for value in [i128::from(i64::MIN) - 1, i128::from(i64::MAX) + 1] {
            assert_eq!(
                final_cost(-7, value),
                Err(ErrorKind::CostOverflow { priority: -7 })
            );
        }
    }
}
