//! Checked neutral-plan admission and once-only fact seeds.
use super::{
    Analysis, Argument, ArgumentId, Binding, Budget, Constant, ConstantId, Error, Expression,
    ExpressionId, Failure, Guard, Interval, Ordering, PredicateId, Rule, RuleBounds, RuleId,
    Status, Values, lattice::Abstract,
};

pub(super) struct Producer<C> {
    pub rule: Rule,
    pub context: Option<C>,
}
/// A synchronous lowering into one complete producer system. Coordinates belong
/// to this input; use the returned coordinates instead of constructing numbers.
pub struct Input<'a, R: Values, C: Copy> {
    pub(super) reader: &'a R,
    pub(super) budget: &'a Budget,
    pub(super) context: Option<C>,
    pub(super) constants: Vec<Constant<R::Key>>,
    key_order: Vec<usize>,
    predicates: Vec<(usize, usize)>,
    pub(super) expressions: Vec<Expression>,
    pub(super) seeds: Vec<Abstract<'a>>,
    pub(super) producers: Vec<Vec<C>>,
    pub(super) rules: Vec<Producer<C>>,
    pub(super) thresholds: Vec<i32>,
}
impl<'a, R: Values, C: Copy> Input<'a, R, C> {
    pub(super) fn new(reader: &'a R, budget: &'a Budget) -> Self {
        Self {
            reader,
            budget,
            context: None,
            constants: Vec::new(),
            key_order: Vec::new(),
            predicates: Vec::new(),
            expressions: Vec::new(),
            seeds: Vec::new(),
            producers: Vec::new(),
            rules: Vec::new(),
            thresholds: Vec::new(),
        }
    }
    /// Select the original context for subsequent operations and producers.
    pub fn context(&mut self, context: C) {
        self.context = Some(context);
    }
    /// Admit actual lowering work before performing it.
    /// # Errors
    /// Reports the inclusive work limit.
    pub fn work(&mut self, amount: u64) -> Result<(), Error<R::Error>> {
        Ok(self.budget.work(amount)?)
    }
    /// Admit actual inspected name/string bytes.
    /// # Errors
    /// Reports the inclusive inspected-byte limit.
    pub fn bytes(&mut self, amount: u64) -> Result<(), Error<R::Error>> {
        Ok(self.budget.bytes(amount)?)
    }
    pub(super) fn links(&self, count: usize) -> Result<(), Error<R::Error>> {
        let mut statistics = self.budget.statistics();
        crate::limits::check(
            crate::Resource::Links,
            statistics.links as u128 + count as u128,
            self.budget.limits.max_links as u128,
        )?;
        statistics.links += count;
        self.budget.statistics.set(statistics);
        Ok(())
    }
    /// Register a distinct signed predicate; identity/deduplication belongs to
    /// the lowering, whose owner already knows the predicate signature.
    /// # Errors
    /// Reports predicate, position, work or allocation limits.
    pub fn predicate(&mut self, arity: usize) -> Result<PredicateId, Error<R::Error>> {
        self.work(1)?;
        let mut statistics = self.budget.statistics();
        crate::limits::check(
            crate::Resource::Predicates,
            statistics.predicates as u128 + 1,
            self.budget.limits.max_predicates as u128,
        )?;
        crate::limits::check(
            crate::Resource::Positions,
            statistics.positions as u128 + arity as u128,
            self.budget.limits.max_positions as u128,
        )?;
        self.work(u64::try_from(arity).map_err(|_| Error::InvalidInput)?)?;
        statistics = self.budget.statistics();
        self.predicates.try_reserve(1)?;
        self.seeds.try_reserve(arity)?;
        self.producers.try_reserve(arity)?;
        let id = PredicateId(self.predicates.len());
        self.predicates.push((self.seeds.len(), arity));
        for _ in 0..arity {
            self.seeds.push(Abstract::bottom(self.budget));
            self.producers.push(Vec::new());
        }
        statistics.predicates += 1;
        statistics.positions += arity;
        self.budget.statistics.set(statistics);
        Ok(id)
    }
    /// Resolve a valid argument coordinate in a registered predicate.
    /// # Errors
    /// Refuses an out-of-range coordinate. Coordinates must come from this input.
    pub fn argument(
        &self,
        predicate: PredicateId,
        position: usize,
    ) -> Result<ArgumentId, Error<R::Error>> {
        let &(start, arity) = self
            .predicates
            .get(predicate.0)
            .ok_or(Error::InvalidInput)?;
        if position >= arity {
            return Err(Error::InvalidInput);
        }
        Ok(ArgumentId(start + position))
    }
    fn threshold(&mut self, value: i32) -> Result<(), Error<R::Error>> {
        let (mut low, mut high) = (0, self.thresholds.len());
        while low < high {
            self.work(1)?;
            let middle = low + (high - low) / 2;
            match self.thresholds[middle].cmp(&value) {
                Ordering::Less => low = middle + 1,
                Ordering::Equal => return Ok(()),
                Ordering::Greater => high = middle,
            }
        }
        self.work(
            u64::try_from(self.thresholds.len() - low + 1).map_err(|_| Error::InvalidInput)?,
        )?;
        self.links(1)?;
        self.thresholds.try_reserve(1)?;
        self.thresholds.insert(low, value);
        Ok(())
    }
    fn thresholds(&mut self, value: i32) -> Result<(), Error<R::Error>> {
        for value in [value.checked_sub(1), Some(value), value.checked_add(1)]
            .into_iter()
            .flatten()
        {
            self.threshold(value)?;
        }
        Ok(())
    }
    /// Classify a submitted key and retain it once by semantic identity. Large
    /// symbols retain their numeric classification but no finite-set payload.
    /// # Errors
    /// Preserves reader, work, structural-plan and allocation failures.
    /// # Panics
    /// Panics if the internal ordered-key registry contains an unretained key;
    /// valid input operations maintain this invariant.
    pub fn constant(&mut self, key: R::Key) -> Result<ConstantId, Error<R::Error>> {
        self.work(1)?;
        let class = self
            .reader
            .classify(&key, &self.budget.limits, self.budget)?;
        if let Some(number) = class.number {
            self.thresholds(number)?;
        }
        let mut low = 0;
        let mut high = self.key_order.len();
        if class.retain {
            while low < high {
                self.work(1)?;
                let middle = low + (high - low) / 2;
                let old = self.constants[self.key_order[middle]]
                    .key
                    .as_ref()
                    .expect("ordered keys are retained");
                match self.reader.compare(old, &key, self.budget)? {
                    Ordering::Less => low = middle + 1,
                    Ordering::Equal => return Ok(ConstantId(self.key_order[middle])),
                    Ordering::Greater => high = middle,
                }
            }
        }
        self.links(1)?;
        self.constants.try_reserve(1)?;
        let id = ConstantId(self.constants.len());
        if class.retain {
            self.work(u64::try_from(self.key_order.len() - low).map_err(|_| Error::InvalidInput)?)?;
            self.key_order.try_reserve(1)?;
            self.key_order.insert(low, id.0);
        }
        self.constants.push(Constant {
            key: class.retain.then_some(key),
            number: class.number,
        });
        Ok(id)
    }
    /// Submit one acyclic expression. Equal explicit nodes share identity so
    /// a guard over an unchanged expression also refines that head expression.
    /// Unknown descriptors are distinct precision summaries, never identities.
    /// # Errors
    /// Refuses invalid dependencies, work/plan limits or allocation failure.
    pub fn expression(&mut self, value: Expression) -> Result<ExpressionId, Error<R::Error>> {
        self.work(1)?;
        let valid = match value {
            Expression::Constant(key) => key.0 < self.constants.len(),
            Expression::Slot(_) | Expression::Unknown { .. } => true,
            Expression::Unary(_, value) | Expression::Absolute(value) => {
                value.0 < self.expressions.len()
            }
            Expression::Binary(_, left, right) => {
                left.0 < self.expressions.len() && right.0 < self.expressions.len()
            }
        };
        if !valid {
            return Err(Error::InvalidInput);
        }
        if !matches!(value, Expression::Unknown { .. }) {
            for (at, old) in self.expressions.iter().enumerate() {
                self.budget.work(1)?;
                if *old == value {
                    return Ok(ExpressionId(at));
                }
            }
        }
        self.links(1)?;
        self.expressions.try_reserve(1)?;
        let id = ExpressionId(self.expressions.len());
        self.expressions.push(value);
        Ok(id)
    }
    fn provenance(&mut self, argument: ArgumentId) -> Result<(), Error<R::Error>> {
        if argument.0 >= self.seeds.len() {
            return Err(Error::InvalidInput);
        }
        if let Some(context) = self.context {
            self.links(1)?;
            self.producers[argument.0].try_reserve(1)?;
            self.producers[argument.0].push(context);
        }
        Ok(())
    }
    /// Seed a fact-column value through its original admitted key.
    /// # Errors
    /// Refuses invalid coordinates, logical limits or allocation failure.
    pub fn seed_value(
        &mut self,
        argument: ArgumentId,
        value: ConstantId,
    ) -> Result<(), Error<R::Error>> {
        self.work(1)?;
        if value.0 >= self.constants.len() {
            return Err(Error::InvalidInput);
        }
        self.provenance(argument)?;
        let value = Abstract::constant(value.0, &self.constants[value.0], self.budget)?;
        let joined =
            self.seeds[argument.0].join(&value, self.budget.limits.max_values_per_argument)?;
        if self.seeds[argument.0].values.is_some() && joined.values.is_none() {
            let mut statistics = self.budget.statistics();
            statistics.widened += 1;
            self.budget.statistics.set(statistics);
        }
        self.seeds[argument.0] = joined;
        Ok(())
    }
    /// Seed a compact interval without enumerating its rows. Its finite values
    /// are Unknown; use `seed_value` instead for a complete small column.
    /// # Errors
    /// Refuses descending intervals, invalid arguments or logical limits.
    pub fn seed_interval(
        &mut self,
        argument: ArgumentId,
        low: i32,
        high: i32,
    ) -> Result<(), Error<R::Error>> {
        self.work(1)?;
        if low > high {
            return Err(Error::InvalidInput);
        }
        self.provenance(argument)?;
        self.thresholds(low)?;
        self.thresholds(high)?;
        let value = Abstract::numeric(Interval::Bounded { low, high }, self.budget);
        let joined =
            self.seeds[argument.0].join(&value, self.budget.limits.max_values_per_argument)?;
        if self.seeds[argument.0].values.is_some() && joined.values.is_none() {
            let mut statistics = self.budget.statistics();
            statistics.widened += 1;
            self.budget.statistics.set(statistics);
        }
        self.seeds[argument.0] = joined;
        Ok(())
    }
    fn check_expression(
        &self,
        expression: ExpressionId,
        slots: usize,
    ) -> Result<(), Error<R::Error>> {
        let mut pending = Vec::new();
        pending.try_reserve(1)?;
        pending.push(expression);
        while let Some(id) = pending.pop() {
            self.budget.work(1)?;
            match *self.expressions.get(id.0).ok_or(Error::InvalidInput)? {
                Expression::Slot(slot) if slot >= slots => return Err(Error::InvalidInput),
                Expression::Unary(_, value) | Expression::Absolute(value) => {
                    pending.try_reserve(1)?;
                    pending.push(value);
                }
                Expression::Binary(_, left, right) => {
                    pending.try_reserve(2)?;
                    pending.extend([left, right]);
                }
                Expression::Slot(_) | Expression::Constant(_) | Expression::Unknown { .. } => {}
            }
        }
        Ok(())
    }
    /// Submit a whole producer scope. No producer may be omitted merely because
    /// its transfer is imprecise; use an Unknown expression for that output.
    /// # Errors
    /// Refuses invalid scopes/dependencies, logical limits or allocation failure.
    pub fn rule(&mut self, rule: Rule) -> Result<RuleId, Error<R::Error>> {
        self.work(1)?;
        self.links(
            rule.slots
                .checked_add(rule.bindings.len())
                .and_then(|n| n.checked_add(rule.guards.len()))
                .and_then(|n| n.checked_add(rule.heads.len()))
                .and_then(|n| n.checked_add(rule.integer_queries.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or(Error::InvalidInput)?,
        )?;
        for binding in &rule.bindings {
            self.budget.work(1)?;
            let slot = match *binding {
                Binding::Positive { slot, argument } => {
                    if argument.0 >= self.seeds.len() {
                        return Err(Error::InvalidInput);
                    }
                    slot
                }
                Binding::Assign { slot, value } => {
                    self.check_expression(value, rule.slots)?;
                    slot
                }
                Binding::Range { slot, low, high } => {
                    self.check_expression(low, rule.slots)?;
                    self.check_expression(high, rule.slots)?;
                    slot
                }
                Binding::Aggregate { slot, .. } => slot,
            };
            if slot >= rule.slots {
                return Err(Error::InvalidInput);
            }
        }
        for guard in &rule.guards {
            self.budget.work(1)?;
            match *guard {
                Guard::Compare { left, right, .. } => {
                    self.check_expression(left, rule.slots)?;
                    self.check_expression(right, rule.slots)?;
                }
                Guard::Membership { slot, low, high } => {
                    if slot >= rule.slots {
                        return Err(Error::InvalidInput);
                    }
                    self.check_expression(low, rule.slots)?;
                    self.check_expression(high, rule.slots)?;
                }
            }
        }
        for head in &rule.heads {
            self.budget.work(1)?;
            self.check_expression(head.value, rule.slots)?;
            self.provenance(head.argument)?;
        }
        for &query in &rule.integer_queries {
            self.budget.work(1)?;
            self.check_expression(query, rule.slots)?;
        }
        self.rules.try_reserve(1)?;
        let id = RuleId(self.rules.len());
        self.rules.push(Producer {
            rule,
            context: self.context,
        });
        Ok(id)
    }
    pub(super) fn finish(
        mut self,
        values: Vec<Abstract<'a>>,
        rules: Vec<RuleBounds>,
    ) -> Result<Analysis<R::Key, C>, Failure<R::Error, C>> {
        let mut arguments = Vec::new();
        if let Err(error) = arguments.try_reserve(values.len()) {
            return Err(Failure {
                error: Error::Allocation(error),
                statistics: self.budget.statistics(),
                context: self.context,
            });
        }
        self.seeds.clear();
        let statistics = self.budget.statistics();
        for (value, producers) in values.into_iter().zip(self.producers) {
            let (bound, values, widening) = value.into_parts();
            arguments.push(Argument {
                bound,
                values,
                producers,
                widening,
            });
        }
        Ok(Analysis {
            constants: self.constants,
            arguments,
            rules,
            context: None,
            statistics,
            status: Status::FixedPoint,
        })
    }
    pub(super) fn complete_thresholds(&mut self) -> Result<(), Error<R::Error>> {
        self.threshold(i32::MIN)?;
        self.threshold(i32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Reader, assert_work_stop, leave_work, test_budget};
    use super::*;
    #[test]
    fn predicate_columns_are_admitted_as_work() {
        let budget = test_budget();
        let mut input = Input::<_, ()>::new(&Reader, &budget);
        leave_work(&budget, 1);
        assert_work_stop(&input.predicate(7), &budget);
        assert_eq!(budget.statistics().positions, 0);
        assert_eq!(budget.statistics().predicates, 0);
    }
    #[test]
    fn an_existing_threshold_still_charges_its_search() {
        let budget = test_budget();
        let mut input = Input::<_, ()>::new(&Reader, &budget);
        for value in 0..7 {
            input.threshold(value).unwrap();
        }
        let links = budget.statistics().links;
        leave_work(&budget, 2);
        assert_work_stop(&input.threshold(0), &budget);
        assert_eq!(
            budget.statistics().links,
            links,
            "refused lookup retained no new record"
        );
    }
}
