use std::cmp::Ordering;

use zetesis_core::catalog::{AtomRef, TermRef};
use zetesis_core::{AtomRows, Model, PatternRef, TemplateTerm, ValueNodeRef};
use zetesis_cpu::Cancellation;

use crate::{
    Condition, ConditionNode, Contribution, Error, ErrorKind, Evaluation, Limits, ObjectiveProgram,
    ObjectiveTemplateRef, Score, Statistics, Stop,
};

pub(super) struct Work<'a> {
    pub(super) limits: Limits,
    pub(super) cancellation: &'a Cancellation,
    pub(super) statistics: Statistics,
    pub(super) template: Option<usize>,
}
impl Work<'_> {
    pub(super) fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            template: self.template,
            statistics: self.statistics,
        }
    }
    pub(super) fn stop(&self, reason: Stop) -> Error {
        self.error(ErrorKind::Stopped(reason))
    }
    pub(super) fn poll(&self) -> Result<(), Error> {
        self.cancellation.poll().map_err(|reason| {
            self.stop(match reason {
                zetesis_cpu::Stop::Cancelled => Stop::Cancelled,
                zetesis_cpu::Stop::Deadline => Stop::Deadline,
                other => Stop::Control(other),
            })
        })
    }
    pub(super) fn tick(&mut self) -> Result<(), Error> {
        self.poll()?;
        if self.statistics.work >= self.limits.max_work {
            return Err(self.stop(Stop::WorkLimit));
        }
        self.statistics.work += 1;
        Ok(())
    }
    pub(super) fn reserve<T>(&self, count: usize) -> Result<Vec<T>, Error> {
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
    // Contribution keys use typed storage order; matching needs only equality.
    fn compare_identity(
        &mut self,
        left: TermRef<'_>,
        right: TermRef<'_>,
    ) -> Result<Ordering, Error> {
        left.compare_ref_with(right, || self.tick())
    }
    fn resolve<'a>(
        &mut self,
        term: TemplateTerm<'a>,
        binding: &[Option<TermRef<'a>>],
    ) -> Result<TermRef<'a>, Error> {
        self.tick()?;
        match term {
            TemplateTerm::Constant(value) => Ok(value),
            TemplateTerm::Variable(variable) => binding
                .get(variable)
                .copied()
                .flatten()
                .ok_or_else(|| self.error(ErrorKind::UnboundVariable { variable })),
        }
    }
    fn contains(&mut self, model: &Model, query: AtomRef<'_>) -> Result<bool, Error> {
        Ok(model.lookup().get_with(query, || self.tick())?.is_some())
    }

    fn condition(&mut self, condition: &Condition, model: &Model) -> Result<bool, Error> {
        let mut values: Vec<bool> = self.reserve(condition.nodes().len())?;
        for index in 0..condition.nodes().len() {
            self.tick()?;
            let node = condition
                .nodes()
                .at(index)
                .expect("bounded condition operation");
            let value = match node {
                ConditionNode::Boolean(value) => value,
                ConditionNode::Atom(atom) => self.contains(model, atom)?,
                ConditionNode::Not(operand) => !values[operand],
                ConditionNode::And(left, right) => values[left] && values[right],
                ConditionNode::Or(left, right) => values[left] || values[right],
            };
            values.push(value);
        }
        Ok(values.last().copied().unwrap_or(true))
    }
}

struct Evaluator<'input, 'control> {
    work: Work<'control>,
    keys: Vec<Contribution<'input>>,
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
pub fn evaluate<'input>(
    program: &'input ObjectiveProgram,
    model: &'input Model,
    limits: Limits,
    cancellation: &Cancellation,
) -> Result<Evaluation<'input>, Error> {
    let mut work = Work {
        limits,
        cancellation,
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

pub(super) fn final_cost(priority: i32, total: i128) -> Result<i64, ErrorKind> {
    i64::try_from(total).map_err(|_| ErrorKind::CostOverflow { priority })
}

struct Frame<'a> {
    atoms: AtomRows<'a, 'a>,
    trail_start: usize,
}

impl<'input> Evaluator<'input, '_> {
    fn join(
        &mut self,
        template: ObjectiveTemplateRef<'input>,
        model: &'input Model,
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
            atoms: model.lookup().predicate_with(
                template
                    .positive()
                    .at(0)
                    .expect("nonempty positive body")
                    .predicate(),
                || self.work.tick(),
            )?,
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
            let Some(row) = frames[depth].atoms.next() else {
                frames.pop();
                continue;
            };
            if !self.matches(
                template.positive().at(depth).expect("join depth in body"),
                row.atom(),
                &mut binding,
                &mut trail,
            )? {
                continue;
            }
            if depth + 1 == template.positive().len() {
                self.active(template, &binding, slot)?;
            } else {
                frames.push(Frame {
                    atoms: model.lookup().predicate_with(
                        template
                            .positive()
                            .at(depth + 1)
                            .expect("next join depth in body")
                            .predicate(),
                        || self.work.tick(),
                    )?,
                    trail_start: trail.len(),
                });
            }
        }
        Ok(())
    }

    fn matches(
        &mut self,
        pattern: PatternRef<'input>,
        atom: AtomRef<'input>,
        binding: &mut [Option<TermRef<'input>>],
        trail: &mut Vec<usize>,
    ) -> Result<bool, Error> {
        self.work.tick()?;
        // The predicate window has already established the full signed signature;
        // matching below still checks constants and repeated/bound whole values.
        for column in 0..pattern.terms().len() {
            self.work.tick()?;
            let term = pattern.terms().at(column).expect("bounded pattern column");
            let value = atom.values().at(column).expect("matched predicate arity");
            match term {
                TemplateTerm::Constant(constant) => {
                    if !constant.equals_ref_with(value, || self.work.tick())? {
                        return Ok(false);
                    }
                }
                TemplateTerm::Variable(variable) => {
                    if let Some(previous) = binding[variable] {
                        if !previous.equals_ref_with(value, || self.work.tick())? {
                            return Ok(false);
                        }
                    } else {
                        binding[variable] = Some(value);
                        trail.push(variable);
                    }
                }
            }
        }
        Ok(true)
    }

    fn active(
        &mut self,
        template: ObjectiveTemplateRef<'input>,
        binding: &[Option<TermRef<'input>>],
        slot: usize,
    ) -> Result<(), Error> {
        self.work.tick()?;
        if self.work.statistics.bindings >= self.work.limits.max_bindings {
            return Err(self.work.stop(Stop::BindingLimit));
        }
        self.work.statistics.bindings += 1;
        for index in 0..template.filters().len() {
            self.work.tick()?;
            let filter = template.filters().at(index).expect("bounded filter index");
            let (left, right) = filter.terms();
            let left = self.work.resolve(left, binding)?;
            let right = self.work.resolve(right, binding)?;
            let equal = left.equals_ref_with(right, || self.work.tick())?;
            if equal != filter.is_equality() {
                return Ok(());
            }
        }
        self.work.statistics.active_bindings =
            self.work.increment(self.work.statistics.active_bindings)?;
        let weight = self.work.resolve(template.weight(), binding)?;
        self.work.tick()?;
        let ValueNodeRef::Number(weight) = weight.descriptor() else {
            return Ok(());
        };
        let weight = template
            .weight_polarity()
            .normalize(weight)
            .ok_or_else(|| self.work.error(ErrorKind::WeightNormalizationOverflow))?;
        let mut tuple = self.work.reserve(template.tuple().len())?;
        for index in 0..template.tuple().len() {
            self.work.tick()?;
            let term = template.tuple().at(index).expect("bounded tuple field");
            tuple.push(self.work.resolve(term, binding)?);
        }
        self.contribute(template.priority(), weight, tuple, slot)
    }

    fn compare_key(
        &mut self,
        index: usize,
        priority: i32,
        weight: i32,
        tuple: &[TermRef<'input>],
    ) -> Result<Ordering, Error> {
        self.work.tick()?;
        let key = &self.keys[index];
        let prefix = key.priority.cmp(&priority).then(key.weight.cmp(&weight));
        if prefix != Ordering::Equal {
            return Ok(prefix);
        }
        for (left, right) in key.tuple.iter().zip(tuple) {
            let order = self.work.compare_identity(*left, *right)?;
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
        tuple: Vec<TermRef<'input>>,
        slot: usize,
    ) -> Result<(), Error> {
        let mut low = 0;
        let mut high = self.keys.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match self.compare_key(middle, priority, weight, &tuple)? {
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
        let bytes = self.key_bytes(&tuple)?;
        let total_bytes = self.work.add_size(self.work.statistics.key_bytes, bytes)?;
        if total_bytes > self.work.limits.max_key_bytes {
            return Err(self.work.stop(Stop::KeyBytesLimit));
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
                tuple,
            },
        );
        self.totals[slot] = total;
        self.work.statistics.keys = self.keys.len();
        self.work.statistics.key_bytes = total_bytes;
        Ok(())
    }

    fn key_bytes(&mut self, tuple: &[TermRef<'input>]) -> Result<usize, Error> {
        // This logical encoding measure is unchanged by borrowing canonical
        // payload. Retained cells contain handles, never copied text or DAGs.
        let mut bytes = 16;
        for value in tuple {
            let length = value.canonical_bytes_with(|| self.work.tick())?;
            bytes = self.work.add_size(bytes, length)?;
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::final_cost;
    use crate::ErrorKind;

    #[test]
    fn accepted_contribution_retains_the_resolved_tuple() {
        let source = [
            zetesis_core::Value::Number(1),
            zetesis_core::Value::Number(2),
        ];
        let tuple: Vec<_> = source
            .iter()
            .map(zetesis_core::catalog::TermRef::from)
            .collect();
        let allocation = tuple.as_ptr();
        let cancellation = zetesis_cpu::Cancellation::default();
        let mut evaluator = super::Evaluator {
            work: super::Work {
                limits: crate::Limits::default(),
                cancellation: &cancellation,
                statistics: crate::Statistics::default(),
                template: None,
            },
            keys: Vec::new(),
            totals: vec![0],
        };
        evaluator.contribute(0, 7, tuple, 0).unwrap();
        // The resolved cells become retained evidence without a second buffer.
        assert_eq!(evaluator.keys[0].tuple.as_ptr(), allocation);
        assert_eq!(evaluator.keys[0].tuple.len(), source.len());
    }

    #[test]
    fn contribution_evidence_borrows_model_payload() {
        use crate::{AdmissionLimits, Limits, ObjectiveProgram, ObjectiveTemplate};
        use zetesis_core::{Atom, AtomPattern, Model, Predicate, Term, Value, ValueNodeRef};
        let predicate = Predicate::new("p", 1).unwrap();
        let model = Model::new([Atom::new(
            predicate.clone(),
            vec![Value::String("retained-payload".repeat(1024))],
        )
        .unwrap()])
        .unwrap();
        let program = ObjectiveProgram::new(
            vec![ObjectiveTemplate::new(
                Term::Constant(Value::Number(1)),
                0,
                vec![Term::Variable(0)],
                vec![AtomPattern::new(predicate, vec![Term::Variable(0)]).unwrap()],
                vec![],
            )],
            AdmissionLimits::default(),
        )
        .unwrap();
        let evaluated = super::evaluate(
            &program,
            &model,
            Limits::default(),
            &zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        let ValueNodeRef::String(source) = model
            .atoms()
            .iter()
            .next()
            .unwrap()
            .values()
            .at(0)
            .unwrap()
            .descriptor()
        else {
            panic!("string source");
        };
        let ValueNodeRef::String(retained) = evaluated.contributions()[0].tuple()[0].descriptor()
        else {
            panic!("string evidence");
        };
        assert!(std::ptr::eq(source, retained));
    }

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
