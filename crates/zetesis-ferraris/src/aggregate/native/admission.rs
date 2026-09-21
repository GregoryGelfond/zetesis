//! Bounded transferred-storage inspection and complete-key uniqueness checking.

use std::cmp::Ordering;

use zetesis_core::Value as Term;
use zetesis_cpu::Cancellation;

use super::{
    AdmissionLimits, Bound, Error, ErrorKind, Function, Group, Guard, Resource, Statistics, Tuple,
    Work, add, bytes, storage,
};
use crate::Theory;

pub(super) fn build(
    theory: &Theory,
    function: Function,
    tuples: Vec<Tuple>,
    guards: Vec<Guard>,
    limits: AdmissionLimits,
    cancellation: &Cancellation,
) -> Result<Group, Error> {
    let mut work = Work {
        maximum: limits.max_work,
        cancellation,
        statistics: Statistics::default(),
    };
    let result = prepare(theory, &tuples, &guards, limits, &mut work);
    match result {
        Ok((first_costs, guard_costs)) => Ok(Group {
            theory: theory.clone(),
            function,
            tuples,
            guards,
            first_costs,
            guard_costs,
            statistics: work.statistics,
        }),
        Err(kind) => Err(work.failure(kind)),
    }
}

fn prepare(
    theory: &Theory,
    tuples: &Vec<Tuple>,
    guards: &Vec<Guard>,
    limits: AdmissionLimits,
    work: &mut Work<'_>,
) -> Result<(Vec<u64>, Vec<u64>), ErrorKind> {
    work.poll()?;
    ceiling(tuples.len(), limits.max_tuples, Resource::Tuples)?;
    ceiling(guards.len(), limits.max_guards, Resource::Guards)?;
    let resident = add(
        bytes::<Tuple>(tuples.capacity())?,
        bytes::<Guard>(guards.capacity())?,
    )?;
    let resident = add(
        resident,
        add(bytes::<u64>(tuples.len())?, bytes::<u64>(guards.len())?)?,
    )?;
    let scratch = add(
        bytes::<u64>(tuples.len())?,
        bytes::<usize>(tuples.len())?
            .checked_mul(2)
            .ok_or(ErrorKind::Overflow)?,
    )?;
    let mut inspection = Inspection {
        limits,
        resident,
        scratch,
        tuple_values: 0,
        value_nodes: 0,
    };
    inspection.check_bytes(work)?;
    for (index, tuple) in tuples.iter().enumerate() {
        work.charge(1)?;
        if tuple.condition >= theory.nodes().len() {
            return Err(ErrorKind::Condition {
                tuple: index,
                node: tuple.condition,
            });
        }
        inspection.tuple_values = inspection
            .tuple_values
            .checked_add(tuple.key.len())
            .ok_or(ErrorKind::Overflow)?;
        ceiling(
            inspection.tuple_values,
            limits.max_tuple_values,
            Resource::TupleValues,
        )?;
        inspection.resident = add(inspection.resident, bytes::<Term>(tuple.key.capacity())?)?;
        inspection.check_bytes(work)?;
        for value in &tuple.key {
            inspection.value(value, work)?;
        }
    }
    for guard in guards {
        work.charge(1)?;
        if let Bound::Term(value) = &guard.bound {
            inspection.value(value, work)?;
        }
    }
    let (first_costs, key_costs) = key_costs(tuples, work)?;
    let mut guard_costs = storage(guards.len())?;
    for guard in guards {
        let cost = match &guard.bound {
            Bound::Integer(_) => 1,
            Bound::Term(value) => term_work(value)?,
        };
        work.charge(cost)?;
        guard_costs.push(cost);
    }
    unique(tuples, &key_costs, work)?;
    work.poll()?;
    Ok((first_costs, guard_costs))
}

struct Inspection {
    limits: AdmissionLimits,
    resident: u64,
    scratch: u64,
    tuple_values: usize,
    value_nodes: usize,
}

impl Inspection {
    fn check_bytes(&self, work: &mut Work<'_>) -> Result<(), ErrorKind> {
        work.statistics.resident_bytes = self.resident;
        work.statistics.peak_bytes = add(self.resident, self.scratch)?;
        if work.statistics.peak_bytes > self.limits.max_bytes {
            return Err(ErrorKind::Limit(Resource::Bytes));
        }
        Ok(())
    }

    fn value(&mut self, value: &Term, work: &mut Work<'_>) -> Result<(), ErrorKind> {
        let nodes = match value {
            Term::Structured(value) => value.nodes().len(),
            _ => 1,
        };
        self.value_nodes = self
            .value_nodes
            .checked_add(nodes)
            .ok_or(ErrorKind::Overflow)?;
        ceiling(
            self.value_nodes,
            self.limits.max_value_nodes,
            Resource::ValueNodes,
        )?;
        work.charge(term_work(value)?)?;
        self.resident = add(
            self.resident,
            u64::try_from(value.payload_bytes()).map_err(|_| ErrorKind::Overflow)?,
        )?;
        self.check_bytes(work)
    }
}

fn key_costs(tuples: &[Tuple], work: &mut Work<'_>) -> Result<(Vec<u64>, Vec<u64>), ErrorKind> {
    let mut first = storage(tuples.len())?;
    let mut keys = storage(tuples.len())?;
    for tuple in tuples {
        work.charge(1)?;
        let mut key = 1;
        let mut first_cost = 0;
        for (index, value) in tuple.key.iter().enumerate() {
            let cost = term_work(value)?;
            work.charge(cost)?;
            key = add(key, cost)?;
            if index == 0 {
                first_cost = cost;
            }
        }
        first.push(first_cost);
        keys.push(key);
    }
    Ok((first, keys))
}

/// Conservative term-carrier comparison charge, matching the ordered-extremum
/// lowering convention. Actual lexicographic comparison may stop earlier.
pub(super) fn term_work(value: &Term) -> Result<u64, ErrorKind> {
    let count = match value {
        Term::Structured(value) => value.canonical_bytes(),
        Term::String(text) | Term::Symbol(text) => {
            text.len().checked_add(1).ok_or(ErrorKind::Overflow)?
        }
        _ => 1,
    };
    u64::try_from(count).map_err(|_| ErrorKind::Overflow)
}

fn unique(tuples: &[Tuple], costs: &[u64], work: &mut Work<'_>) -> Result<(), ErrorKind> {
    let mut order = storage(tuples.len())?;
    let mut buffer = storage(tuples.len())?;
    for index in 0..tuples.len() {
        work.charge(2)?;
        order.push(index);
        buffer.push(index);
    }
    let mut width = 1_usize;
    while width < tuples.len() {
        let mut start = 0_usize;
        while start < tuples.len() {
            let middle = start.saturating_add(width).min(tuples.len());
            let end = middle.saturating_add(width).min(tuples.len());
            merge(
                tuples,
                costs,
                &order,
                &mut buffer[start..end],
                start..middle,
                middle..end,
                work,
            )?;
            start = end;
        }
        std::mem::swap(&mut order, &mut buffer);
        width = width.saturating_mul(2);
    }
    for pair in order.windows(2) {
        work.charge(1)?;
        if compare(tuples, costs, pair[0], pair[1], work)?.is_eq() {
            return Err(ErrorKind::Duplicate {
                first: pair[0],
                second: pair[1],
            });
        }
    }
    Ok(())
}

fn merge(
    tuples: &[Tuple],
    costs: &[u64],
    order: &[usize],
    output: &mut [usize],
    mut left: std::ops::Range<usize>,
    mut right: std::ops::Range<usize>,
    work: &mut Work<'_>,
) -> Result<(), ErrorKind> {
    for slot in output {
        work.charge(1)?;
        let take_left = right.is_empty()
            || (!left.is_empty()
                && !compare(tuples, costs, order[left.start], order[right.start], work)?.is_gt());
        let next = if take_left { &mut left } else { &mut right };
        *slot = order[next.start];
        next.start += 1;
    }
    Ok(())
}

fn compare(
    tuples: &[Tuple],
    costs: &[u64],
    left: usize,
    right: usize,
    work: &mut Work<'_>,
) -> Result<Ordering, ErrorKind> {
    work.charge(add(costs[left], costs[right])?)?;
    let left = &tuples[left].key;
    let right = &tuples[right].key;
    Ok(left
        .iter()
        .zip(right)
        .map(|(left, right)| left.compare_terms(right))
        .find(|order| !order.is_eq())
        .unwrap_or_else(|| left.len().cmp(&right.len())))
}

fn ceiling(actual: usize, maximum: usize, resource: Resource) -> Result<(), ErrorKind> {
    if actual > maximum {
        Err(ErrorKind::Limit(resource))
    } else {
        Ok(())
    }
}
