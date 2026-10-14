//! Synchronous rounds, outward threshold widening and one sound narrowing pass.
use super::{
    Budget, Error, Input, Interval, RuleBounds, Values, integer, lattice::Abstract, transfer,
};

fn copied<'a, E>(values: &[Abstract<'a>]) -> Result<Vec<Abstract<'a>>, Error<E>> {
    let mut copy = Vec::new();
    copy.try_reserve(values.len())?;
    for value in values {
        copy.push(value.copied()?);
    }
    Ok(copy)
}
fn round<R: Values, C: Copy>(input: &Input<'_, R, C>) -> Result<(), Error<R::Error>> {
    let mut statistics = input.budget.statistics();
    crate::limits::check(
        crate::Resource::Rounds,
        u128::from(statistics.rounds) + 1,
        u128::from(input.budget.limits.max_rounds),
    )?;
    statistics.rounds += 1;
    input.budget.statistics.set(statistics);
    Ok(())
}
fn image<'a, R: Values, C: Copy>(
    input: &mut Input<'a, R, C>,
    previous: &[Abstract<'a>],
) -> Result<Vec<Abstract<'a>>, Error<R::Error>> {
    round(input)?;
    let mut result = copied(&input.seeds)?;
    for producer in &input.rules {
        input.context = producer.context;
        input.budget.work(1)?;
        let local = transfer::rule(input, &producer.rule, previous)?;
        for head in &producer.rule.heads {
            input.budget.work(1)?;
            let value = transfer::head(input, &local, head.value)?;
            result[head.argument.0] = result[head.argument.0]
                .join(&value, input.budget.limits.max_values_per_argument)?;
        }
    }
    Ok(result)
}
#[derive(Clone, Copy, Default)]
struct Growth {
    low: u8,
    high: u8,
}
fn widen<E>(
    previous: Interval,
    next: Interval,
    growth: &mut Growth,
    thresholds: &[i32],
    budget: &Budget,
) -> Result<Interval, Error<E>> {
    let (Some((old_low, old_high)), Some((mut low, mut high))) =
        (previous.endpoints(), next.endpoints())
    else {
        return Ok(next);
    };
    if low < old_low {
        if growth.low >= 2 {
            for &threshold in thresholds.iter().rev() {
                budget.work(1)?;
                if threshold <= low {
                    low = threshold;
                    break;
                }
            }
        } else {
            growth.low += 1;
        }
    }
    if high > old_high {
        if growth.high >= 2 {
            for &threshold in thresholds {
                budget.work(1)?;
                if threshold >= high {
                    high = threshold;
                    break;
                }
            }
        } else {
            growth.high += 1;
        }
    }
    Ok(if next == Interval::Unbounded {
        next
    } else {
        Interval::Bounded { low, high }
    })
}
pub(super) struct Completed<'a> {
    pub arguments: Vec<Abstract<'a>>,
    pub rules: Vec<RuleBounds>,
}
pub(super) fn run<'a, R: Values, C: Copy>(
    input: &mut Input<'a, R, C>,
) -> Result<Completed<'a>, Error<R::Error>> {
    input.complete_thresholds()?;
    let mut current = copied(&input.seeds)?;
    let mut growth = Vec::new();
    input
        .budget
        .work(u64::try_from(current.len()).map_err(|_| Error::InvalidInput)?)?;
    growth.try_reserve(current.len())?;
    growth.resize(current.len(), Growth::default());
    if !input.rules.is_empty() {
        loop {
            let candidates = image(input, &current)?;
            let mut next = Vec::new();
            next.try_reserve(current.len())?;
            let mut changed = false;
            for (index, (old, candidate)) in current.iter().zip(&candidates).enumerate() {
                input.budget.work(1)?;
                // Retain the accumulated upper value during ascent. This also
                // prevents any component from narrowing before convergence.
                let mut value = old.join(candidate, input.budget.limits.max_values_per_argument)?;
                value.bound.interval = widen(
                    old.bound.interval,
                    value.bound.interval,
                    &mut growth[index],
                    &input.thresholds,
                    input.budget,
                )?;
                changed |= !old.same(&value)?;
                if old.values.is_some() && value.values.is_none() {
                    let mut statistics = input.budget.statistics();
                    statistics.widened += 1;
                    input.budget.statistics.set(statistics);
                }
                next.push(value);
            }
            drop(candidates);
            current = next;
            if !changed {
                break;
            }
        }
        // The stable ascent establishes F(W) <= W. Applying every producer to
        // that same W, without a join with W or in-place updates, stays above
        // the least fixed point. Only this complete pass may replace W.
        current = image(input, &current)?;
    }
    let mut rules = Vec::new();
    rules.try_reserve(input.rules.len())?;
    for producer in &input.rules {
        input.context = producer.context;
        input.budget.work(1)?;
        let local = transfer::rule(input, &producer.rule, &current)?;
        let mut domains = Vec::new();
        domains.try_reserve(local.domains.len())?;
        let mut produced = Vec::new();
        produced.try_reserve(local.produced.len())?;
        for value in &local.domains {
            input.budget.work(1)?;
            domains.push(value.bound);
        }
        for value in &local.produced {
            input.budget.work(1)?;
            produced.push(value.bound);
        }
        let integers = integer::queries(input, &producer.rule.integer_queries, &domains)?;
        rules.push(RuleBounds {
            domains,
            produced,
            integers,
        });
    }
    input
        .budget
        .work(u64::try_from(current.len()).map_err(|_| Error::InvalidInput)?)?;
    Ok(Completed {
        arguments: current,
        rules,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_work_stop, leave_work, test_budget};
    use super::*;
    #[test]
    fn both_widening_searches_charge_every_examined_threshold() {
        for lower in [false, true] {
            let budget = test_budget();
            let mut growth = Growth { low: 2, high: 2 };
            let previous = Interval::Bounded { low: -1, high: 1 };
            let next = if lower {
                Interval::Bounded { low: -2, high: 1 }
            } else {
                Interval::Bounded { low: -1, high: 2 }
            };
            let thresholds = [i32::MIN, -10, -5, 0, 5, 10, i32::MAX];
            leave_work(&budget, 2);
            assert_work_stop(
                &widen::<()>(previous, next, &mut growth, &thresholds, &budget),
                &budget,
            );
        }
    }
}
