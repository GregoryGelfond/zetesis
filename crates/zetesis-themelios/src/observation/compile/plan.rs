//! Schedule finite binding dependencies before evaluating model-relative conditions.
//!
//! Scalar and aggregate assignments add data slots, never logical atoms. An
//! aggregate's element inputs must already be available; its remaining guards
//! return to ordinary comparison scheduling after the actual measure is bound.

use super::{
    BTreeSet, Binder, Compiler, Condition, DefaultNegation, Error, Feature, Pattern, Query,
    Relation, Template,
};

impl Compiler<'_> {
    fn ready(&self, term: &Template) -> bool {
        self.ready_with(term, &BTreeSet::new())
    }
    pub(super) fn ready_with(&self, term: &Template, available: &BTreeSet<usize>) -> bool {
        match term {
            Template::Variable(slot) => self.safe.contains(slot) || available.contains(slot),
            Template::Value(_) => true,
            Template::Unary(_, argument) | Template::Absolute(argument) => {
                self.ready_with(argument, available)
            }
            Template::Binary(_, left, right) | Template::Interval(left, right) => {
                self.ready_with(left, available) && self.ready_with(right, available)
            }
            Template::Function(_, _, arguments)
            | Template::Tuple(arguments)
            | Template::Pool(arguments) => arguments
                .iter()
                .all(|argument| self.ready_with(argument, available)),
        }
    }
    fn assignment(&self, condition: &Condition) -> Option<(usize, bool)> {
        let Condition::Compare(DefaultNegation::None, left, steps) = condition else {
            return None;
        };
        let [(Relation::Eq, right)] = steps.as_slice() else {
            return None;
        };
        if let Template::Variable(slot) = left
            && !self.safe.contains(slot)
            && self.ready(right)
        {
            return Some((*slot, true));
        }
        if let Template::Variable(slot) = right
            && !self.safe.contains(slot)
            && self.ready(left)
        {
            return Some((*slot, false));
        }
        None
    }
    fn assignments(&mut self, conditions: &mut Vec<Condition>, binders: &mut Vec<Binder>) {
        while let Some((index, slot, forward)) =
            conditions
                .iter()
                .enumerate()
                .find_map(|(index, condition)| {
                    self.assignment(condition)
                        .map(|(slot, forward)| (index, slot, forward))
                })
        {
            let Condition::Compare(_, left, mut steps) = conditions.remove(index) else {
                unreachable!()
            };
            let expression = if forward { steps.remove(0).1 } else { left };
            binders.push(Binder::Assign(slot, expression));
            self.safe.insert(slot);
        }
    }
    fn aggregate_assignment(&self, condition: &Condition) -> Option<(usize, usize)> {
        let Condition::Aggregate(DefaultNegation::None, aggregate, guards) = condition else {
            return None;
        };
        if aggregate
            .elements
            .iter()
            .flat_map(|element| &element.query.inputs)
            .any(|slot| !self.safe.contains(slot))
        {
            return None;
        }
        guards.iter().enumerate().find_map(|(index, guard)| {
            let Template::Variable(slot) = guard.bound else {
                return None;
            };
            (guard.relation == Relation::Eq && !self.safe.contains(&slot)).then_some((index, slot))
        })
    }
    fn bind_aggregate(
        &mut self,
        conditions: &mut Vec<Condition>,
        binders: &mut Vec<Binder>,
    ) -> bool {
        let Some((index, guard_index, slot)) =
            conditions
                .iter()
                .enumerate()
                .find_map(|(index, condition)| {
                    self.aggregate_assignment(condition)
                        .map(|(guard, slot)| (index, guard, slot))
                })
        else {
            return false;
        };
        let Condition::Aggregate(_, aggregate, mut guards) = conditions.remove(index) else {
            unreachable!()
        };
        guards.remove(guard_index);
        binders.push(Binder::Aggregate(slot, aggregate));
        self.safe.insert(slot);
        for guard in guards {
            conditions.push(Condition::Compare(
                DefaultNegation::None,
                Template::Variable(slot),
                vec![(guard.relation, guard.bound)],
            ));
        }
        true
    }
    pub(super) fn finish(
        &mut self,
        mut positive: Vec<Vec<Pattern>>,
        mut conditions: Vec<Condition>,
    ) -> Result<Query, Error> {
        let mut binders = Vec::new();
        let mut generated = std::mem::take(&mut self.generated);
        loop {
            if let Some(index) = positive
                .iter()
                .position(|patterns| self.patterns_ready(patterns))
            {
                let pattern = positive.remove(index);
                self.safe.extend(Self::common_captures(&pattern));
                binders.push(Binder::Atom(pattern));
                continue;
            }
            if let Some(index) = generated
                .iter()
                .position(|(_, expression)| self.ready(expression))
            {
                let (slot, expression) = generated.remove(index);
                self.safe.insert(slot);
                binders.push(Binder::Assign(slot, expression));
                continue;
            }
            let before = binders.len();
            self.assignments(&mut conditions, &mut binders);
            if before == binders.len() && !self.bind_aggregate(&mut conditions, &mut binders) {
                break;
            }
        }
        if !positive.is_empty()
            || !generated.is_empty()
            || self.used.iter().any(|slot| !self.safe.contains(slot))
        {
            return Err(self.unsupported(Feature::UnsafeVariable));
        }
        Ok(Query {
            binders,
            conditions,
            variables: self.slots,
            inputs: self
                .used
                .iter()
                .copied()
                .filter(|slot| *slot < self.scope_outer)
                .collect(),
        })
    }
}
