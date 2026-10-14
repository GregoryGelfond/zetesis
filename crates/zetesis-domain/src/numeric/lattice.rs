//! Reduced numeric components and bounded finite sets, with scoped entry leases.
use super::{Bound, Budget, Constant, Error, Integrality, Interval, Ordering, Stop, Widening};

impl Interval {
    pub(super) fn hull(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value,
            (Self::Unbounded, _) | (_, Self::Unbounded) => Self::Unbounded,
            (Self::Bounded { low: a, high: b }, Self::Bounded { low: c, high: d }) => {
                Self::Bounded {
                    low: a.min(c),
                    high: b.max(d),
                }
            }
        }
    }
    pub(super) fn meet(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            (Self::Unbounded, value) | (value, Self::Unbounded) => value,
            (Self::Bounded { low: a, high: b }, Self::Bounded { low: c, high: d }) => {
                Self::checked(i64::from(a.max(c)), i64::from(b.min(d)))
            }
        }
    }
    pub(super) fn checked(low: i64, high: i64) -> Self {
        let low = low.max(i64::from(i32::MIN));
        let high = high.min(i64::from(i32::MAX));
        if low > high {
            Self::Bottom
        } else {
            Self::Bounded {
                low: i32::try_from(low).expect("clipped lower endpoint"),
                high: i32::try_from(high).expect("clipped upper endpoint"),
            }
        }
    }
    pub(super) fn endpoints(self) -> Option<(i32, i32)> {
        match self {
            Self::Bottom => None,
            Self::Unbounded => Some((i32::MIN, i32::MAX)),
            Self::Bounded { low, high } => Some((low, high)),
        }
    }
    pub(super) fn contains(self, number: i32) -> bool {
        self.endpoints()
            .is_some_and(|(low, high)| low <= number && number <= high)
    }
}
impl Bound {
    pub(super) const TOP: Self = Self {
        integrality: Integrality::Mixed,
        interval: Interval::Unbounded,
    };
    pub(super) const BOTTOM: Self = Self {
        integrality: Integrality::Integer,
        interval: Interval::Bottom,
    };
    fn hull(self, other: Self) -> Self {
        Self {
            integrality: if self.integrality == Integrality::Integer
                && other.integrality == Integrality::Integer
            {
                Integrality::Integer
            } else {
                Integrality::Mixed
            },
            interval: self.interval.hull(other.interval),
        }
    }
    fn meet(self, other: Self) -> Self {
        Self {
            integrality: if self.integrality == Integrality::Integer
                || other.integrality == Integrality::Integer
            {
                Integrality::Integer
            } else {
                Integrality::Mixed
            },
            interval: self.interval.meet(other.interval),
        }
    }
}
impl Budget {
    fn retain(&self, count: usize) -> Result<(), Stop> {
        let mut statistics = self.statistics();
        crate::limits::check(
            crate::Resource::ValueEntries,
            statistics.value_entries as u128 + count as u128,
            self.limits.max_value_entries as u128,
        )?;
        statistics.value_entries += count;
        self.statistics.set(statistics);
        Ok(())
    }
    fn release(&self, count: usize) {
        let mut statistics = self.statistics();
        statistics.value_entries -= count;
        self.statistics.set(statistics);
    }
}
/// Every allocated finite reference belongs to one value and returns its live
/// reservation on drop, including early returns and failed intersections.
pub(super) struct Abstract<'a> {
    pub bound: Bound,
    pub values: Option<Vec<usize>>,
    pub widening: Option<Widening>,
    budget: &'a Budget,
}
impl Drop for Abstract<'_> {
    fn drop(&mut self) {
        if let Some(values) = &self.values {
            self.budget.release(values.len());
        }
    }
}
impl<'a> Abstract<'a> {
    pub(super) fn bottom(budget: &'a Budget) -> Self {
        Self {
            bound: Bound::BOTTOM,
            values: Some(Vec::new()),
            widening: None,
            budget,
        }
    }
    pub(super) fn top(reason: Option<Widening>, budget: &'a Budget) -> Self {
        Self {
            bound: Bound::TOP,
            values: None,
            widening: reason,
            budget,
        }
    }
    pub(super) fn numeric(interval: Interval, budget: &'a Budget) -> Self {
        if interval == Interval::Bottom {
            return Self::bottom(budget);
        }
        Self {
            bound: Bound {
                integrality: Integrality::Integer,
                interval,
            },
            values: None,
            widening: None,
            budget,
        }
    }
    pub(super) fn constant<K, E>(
        index: usize,
        value: &Constant<K>,
        budget: &'a Budget,
    ) -> Result<Self, Error<E>> {
        let bound = Bound {
            integrality: if value.number.is_some() {
                Integrality::Integer
            } else {
                Integrality::Mixed
            },
            interval: value
                .number
                .map_or(Interval::Bottom, |n| Interval::Bounded { low: n, high: n }),
        };
        let retain = value.key.is_some() && budget.limits.max_values_per_argument > 0;
        let widening = if value.key.is_none() {
            Some(Widening::SymbolSize)
        } else if retain {
            None
        } else {
            Some(Widening::ValueWidth)
        };
        let mut result = Self {
            bound,
            values: retain.then(Vec::new),
            widening,
            budget,
        };
        if result.values.is_some() {
            result.push(index)?;
        }
        Ok(result)
    }
    fn push<E>(&mut self, value: usize) -> Result<(), Error<E>> {
        let values = self
            .values
            .as_mut()
            .expect("only finite results collect entries");
        self.budget.retain(1)?;
        if let Err(error) = values.try_reserve(1) {
            self.budget.release(1);
            return Err(error.into());
        }
        values.push(value);
        Ok(())
    }
    pub(super) fn copied<E>(&self) -> Result<Self, Error<E>> {
        self.budget.work(1)?;
        let mut copy = Self {
            bound: self.bound,
            values: self.values.as_ref().map(|_| Vec::new()),
            widening: self.widening,
            budget: self.budget,
        };
        if let Some(values) = &self.values {
            self.budget
                .work(u64::try_from(values.len()).map_err(|_| Error::InvalidInput)?)?;
            for &value in values {
                copy.push(value)?;
            }
        }
        Ok(copy)
    }
    pub(super) fn into_parts(mut self) -> (Bound, Option<Vec<usize>>, Option<Widening>) {
        let values = self.values.take();
        if let Some(values) = &values {
            self.budget.release(values.len());
        }
        (self.bound, values, self.widening)
    }
    pub(super) fn same<E>(&self, other: &Self) -> Result<bool, Error<E>> {
        self.budget.work(1)?;
        if self.bound != other.bound {
            return Ok(false);
        }
        match (&self.values, &other.values) {
            (None, None) => Ok(true),
            (Some(left), Some(right)) if left.len() == right.len() => {
                for (left, right) in left.iter().zip(right) {
                    self.budget.work(1)?;
                    if left != right {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn permits<E>(&self, value: usize) -> Result<bool, Error<E>> {
        let Some(values) = &self.values else {
            return Ok(true);
        };
        let (mut low, mut high) = (0, values.len());
        while low < high {
            self.budget.work(1)?;
            let middle = low + (high - low) / 2;
            match values[middle].cmp(&value) {
                Ordering::Less => low = middle + 1,
                Ordering::Equal => return Ok(true),
                Ordering::Greater => high = middle,
            }
        }
        Ok(false)
    }
    pub(super) fn join<E>(&self, other: &Self, width: usize) -> Result<Self, Error<E>> {
        self.budget.work(1)?;
        let mut result = Self {
            bound: self.bound.hull(other.bound),
            values: None,
            widening: self.widening.or(other.widening),
            budget: self.budget,
        };
        if let (Some(left), Some(right)) = (&self.values, &other.values) {
            result.values = Some(Vec::new());
            let (mut a, mut b) = (0, 0);
            while a < left.len() || b < right.len() {
                self.budget.work(1)?;
                let value = if b == right.len() || (a < left.len() && left[a] < right[b]) {
                    let v = left[a];
                    a += 1;
                    v
                } else if a == left.len() || right[b] < left[a] {
                    let v = right[b];
                    b += 1;
                    v
                } else {
                    let v = left[a];
                    a += 1;
                    b += 1;
                    v
                };
                if result
                    .values
                    .as_ref()
                    .is_some_and(|values| values.len() >= width)
                {
                    if let Some(values) = result.values.take() {
                        self.budget.release(values.len());
                    }
                    result.widening = result.widening.or(Some(Widening::ValueWidth));
                    break;
                }
                result.push(value)?;
            }
        }
        Ok(result)
    }
    pub(super) fn meet<K, E>(
        &self,
        other: &Self,
        constants: &[Constant<K>],
    ) -> Result<Self, Error<E>> {
        self.budget.work(1)?;
        let mut result = Self {
            bound: self.bound.meet(other.bound),
            values: None,
            widening: self.widening.or(other.widening),
            budget: self.budget,
        };
        let source = match (&self.values, &other.values) {
            (Some(left), Some(right)) => Some(if left.len() <= right.len() {
                left
            } else {
                right
            }),
            (Some(values), None) | (None, Some(values)) => Some(values),
            (None, None) => None,
        };
        if let Some(source) = source {
            result.values = Some(Vec::new());
            for &value in source {
                self.budget.work(1)?;
                if !self.permits(value)? || !other.permits(value)? {
                    continue;
                }
                let allowed = constants[value]
                    .number
                    .map_or(result.bound.integrality == Integrality::Mixed, |n| {
                        result.bound.interval.contains(n)
                    });
                if allowed {
                    result.push(value)?;
                }
            }
            result.reduce(constants)?;
            result.widening = None;
        } else if result.bound == Bound::BOTTOM {
            result.values = Some(Vec::new());
        }
        Ok(result)
    }
    pub(super) fn refine<K, E>(
        &self,
        interval: Interval,
        constants: &[Constant<K>],
    ) -> Result<Self, Error<E>> {
        self.meet(&Self::numeric(interval, self.budget), constants)
    }
    fn reduce<K, E>(&mut self, constants: &[Constant<K>]) -> Result<(), Error<E>> {
        if let Some(values) = &self.values {
            let mut bound = Bound::BOTTOM;
            for &value in values {
                self.budget.work(1)?;
                let item = &constants[value];
                bound = bound.hull(Bound {
                    integrality: if item.number.is_some() {
                        Integrality::Integer
                    } else {
                        Integrality::Mixed
                    },
                    interval: item
                        .number
                        .map_or(Interval::Bottom, |n| Interval::Bounded { low: n, high: n }),
                });
            }
            self.bound = self.bound.meet(bound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_work_stop, leave_work, test_budget};
    use super::*;
    #[test]
    fn copying_an_unknown_bound_is_still_work() {
        let budget = test_budget();
        let value = Abstract::top(None, &budget);
        leave_work(&budget, 0);
        assert_work_stop(&value.copied::<()>(), &budget);
    }
    #[test]
    fn finite_scans_stop_at_each_inspection_limit() {
        for operation in 0..3 {
            let budget = test_budget();
            let constants: Vec<_> = (0..7)
                .map(|number| Constant {
                    key: Some(number),
                    number: Some(number),
                })
                .collect();
            let mut value = Abstract::bottom(&budget);
            for index in 0..7 {
                value.push::<()>(index).unwrap();
            }
            value.bound = Bound {
                integrality: Integrality::Integer,
                interval: Interval::Bounded { low: 0, high: 6 },
            };
            let other = value.copied::<()>().unwrap();
            leave_work(&budget, 2);
            let result = match operation {
                0 => value.same::<()>(&other).map(|_| ()),
                1 => value.permits::<()>(0).map(|_| ()),
                _ => value.reduce::<_, ()>(&constants),
            };
            assert_work_stop(&result, &budget);
        }
    }
}
