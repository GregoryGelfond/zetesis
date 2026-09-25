//! Canonical admission and occurrence-only complete-key uniqueness checking.

use super::{
    AdmissionLimits, Bound, BoundData, Error, ErrorKind, Function, Group, GroupData, GroupRef,
    Guard, GuardData, Resource, Statistics, Tuple, TupleData, Work, add, bytes,
};
use crate::Theory;
use std::cmp::Ordering;
use zetesis_core::catalog::{
    CatalogRead, Error as CatalogError, Limits, TermKey, TermRef, VocabularyBuilder,
    VocabularyFailure,
};
use zetesis_core::{
    TemplateCatalogFailure, TemplateComponents, TemplateTerm, Value as Term, ValueNodeRef,
};
use zetesis_cpu::Cancellation;

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
    let result = (|| {
        work.poll()?;
        let maximum = usize::try_from(limits.max_storage_bytes).unwrap_or(usize::MAX);
        let owner = VocabularyBuilder::new(maximum).map_err(catalog)?;
        let mut admission = Admission::new(
            theory,
            function,
            owner,
            tuples.len(),
            guards.len(),
            limits,
            &mut work,
        )?;
        admission.logical(add(
            bytes::<Tuple>(tuples.capacity())?,
            bytes::<Guard>(guards.capacity())?,
        )?)?;
        for tuple in tuples {
            admission.begin_tuple(tuple.key.len(), tuple.key.capacity(), tuple.condition)?;
            for value in &tuple.key {
                admission.owned_value(value)?;
            }
            admission.end_tuple(tuple.condition)?;
        }
        for guard in guards {
            admission.work.charge(1)?;
            let bound = match &guard.bound {
                Bound::Integer(value) => {
                    admission.guard_costs(1)?;
                    BoundData::Integer(*value)
                }
                Bound::Term(value) => {
                    let cost = term_work(value.into(), admission.work)?;
                    let at = admission.owned_value(value)?;
                    admission.guard_costs(cost)?;
                    BoundData::Term(at)
                }
            };
            admission.data.guards.push(GuardData {
                comparison: guard.comparison,
                bound,
            });
        }
        let (mut data, mut owner) = admission.finish()?;
        owner.ceiling(maximum).map_err(catalog)?;
        let vocabulary = owner
            .finish_with(data_bytes(&data), || work.charge(1))
            .map_err(vocabulary)?;
        work.statistics.storage_bytes = narrow(data_bytes(&data) + vocabulary.storage_bytes())?;
        work.statistics.storage_peak_bytes = work
            .statistics
            .storage_peak_bytes
            .max(narrow(vocabulary.publication_peak_bytes())?);
        data.statistics = work.statistics;
        Ok(Group { vocabulary, data })
    })();
    result.map_err(|kind| work.failure(kind))
}

pub(super) fn borrowed<'a>(
    theory: &Theory,
    function: Function,
    read: CatalogRead<'a>,
    tuples: Vec<Tuple<Vec<TermRef<'a>>>>,
    guards: Vec<Guard<TermRef<'a>>>,
    limits: AdmissionLimits,
    cancellation: &Cancellation,
) -> Result<GroupData, Error> {
    let mut work = Work {
        maximum: limits.max_work,
        cancellation,
        statistics: Statistics::default(),
    };
    let result = (|| {
        work.poll()?;
        let mut admission = Admission::new(
            theory,
            function,
            read,
            tuples.len(),
            guards.len(),
            limits,
            &mut work,
        )?;
        // The logical carrier uses Value cells even though execution retains only IDs.
        admission.logical(add(
            bytes::<Tuple>(tuples.capacity())?,
            bytes::<Guard>(guards.capacity())?,
        )?)?;
        for tuple in tuples {
            admission.begin_tuple(tuple.key.len(), tuple.key.capacity(), tuple.condition)?;
            for &value in &tuple.key {
                admission.borrowed_value(value)?;
            }
            admission.end_tuple(tuple.condition)?;
        }
        for guard in guards {
            admission.work.charge(1)?;
            let bound = match guard.bound {
                Bound::Integer(value) => {
                    admission.guard_costs(1)?;
                    BoundData::Integer(value)
                }
                Bound::Term(value) => {
                    let cost = term_work(value, admission.work)?;
                    let at = admission.borrowed_value(value)?;
                    admission.guard_costs(cost)?;
                    BoundData::Term(at)
                }
            };
            admission.data.guards.push(GuardData {
                comparison: guard.comparison,
                bound,
            });
        }
        let (mut data, _) = admission.finish()?;
        data.statistics = work.statistics;
        Ok(data)
    })();
    result.map_err(|kind| work.failure(kind))
}

/// The admission algorithm either imports into its owned vocabulary or borrows
/// an already admitted prefix. The source type determines that capability;
/// neither route allocates an indirection or retains a second payload owner.
trait Authority {
    fn read(&self) -> CatalogRead<'_>;
    fn bytes(&self) -> u128;
    fn import(
        &mut self,
        value: TermRef<'_>,
        external: u128,
        limits: AdmissionLimits,
        work: &mut Work<'_>,
    ) -> Result<Option<TermKey>, ErrorKind>;
}
impl Authority for VocabularyBuilder {
    fn read(&self) -> CatalogRead<'_> {
        Self::read(self)
    }
    fn bytes(&self) -> u128 {
        self.storage_bytes()
    }
    fn import(
        &mut self,
        value: TermRef<'_>,
        external: u128,
        limits: AdmissionLimits,
        work: &mut Work<'_>,
    ) -> Result<Option<TermKey>, ErrorKind> {
        self.ceiling(available(limits, external)?)
            .map_err(catalog)?;
        self.restart_storage_peak();
        let result = self.import_term_with(
            value,
            Limits {
                max_nodes: usize::MAX,
                max_depth: usize::MAX,
                max_bytes: usize::MAX,
            },
            || work.charge(1),
        );
        let current = external + self.storage_bytes();
        work.statistics.storage_bytes = narrow(current)?;
        work.statistics.storage_peak_bytes = work
            .statistics
            .storage_peak_bytes
            .max(narrow(external + self.storage_peak_bytes())?);
        result.map(Some).map_err(vocabulary)
    }
}
impl Authority for CatalogRead<'_> {
    fn read(&self) -> CatalogRead<'_> {
        *self
    }
    fn bytes(&self) -> u128 {
        0
    }
    fn import(
        &mut self,
        _value: TermRef<'_>,
        _external: u128,
        _limits: AdmissionLimits,
        _work: &mut Work<'_>,
    ) -> Result<Option<TermKey>, ErrorKind> {
        Ok(None)
    }
}
struct Admission<'w, 'c, A> {
    data: GroupData,
    authority: A,
    limits: AdmissionLimits,
    work: &'w mut Work<'c>,
    key_costs: Vec<u64>,
    logical_scratch: u64,
    tuple_values: usize,
    value_nodes: usize,
    next_term: usize,
    key_start: usize,
    key_cost: u64,
    first_cost: u64,
    in_key: bool,
}
impl<'w, 'c, A: Authority> Admission<'w, 'c, A> {
    fn new(
        theory: &Theory,
        function: Function,
        authority: A,
        tuples: usize,
        guards: usize,
        limits: AdmissionLimits,
        work: &'w mut Work<'c>,
    ) -> Result<Self, ErrorKind> {
        ceiling(tuples, limits.max_tuples, Resource::Tuples)?;
        ceiling(guards, limits.max_guards, Resource::Guards)?;
        let allowance = available(
            limits,
            authority.bytes() + size_of::<GroupData>() as u128
                - size_of::<TemplateComponents>() as u128,
        )?;
        let components =
            TemplateComponents::new(authority.read(), allowance).map_err(component_never)?;
        let data = GroupData {
            theory: theory.clone(),
            function,
            components,
            tuples: Vec::new(),
            guards: Vec::new(),
            first_costs: Vec::new(),
            guard_costs: Vec::new(),
            statistics: Statistics::default(),
        };
        let mut result = Self {
            data,
            authority,
            limits,
            work,
            key_costs: Vec::new(),
            logical_scratch: add(
                bytes::<u64>(tuples)?,
                bytes::<usize>(tuples)?
                    .checked_mul(2)
                    .ok_or(ErrorKind::Overflow)?,
            )?,
            tuple_values: 0,
            value_nodes: 0,
            next_term: 0,
            key_start: 0,
            key_cost: 1,
            first_cost: 0,
            in_key: false,
        };
        result.observe(0)?;
        result.reserve_buffers(tuples, guards)?;
        result.logical(add(bytes::<u64>(tuples)?, bytes::<u64>(guards)?)?)?;
        Ok(result)
    }
    fn current(&self) -> u128 {
        self.authority.bytes()
            + data_bytes(&self.data)
            + self.key_costs.capacity() as u128 * size_of::<u64>() as u128
    }
    fn observe(&mut self, overlap: u128) -> Result<(), ErrorKind> {
        let current = self.current();
        let peak = current.checked_add(overlap).ok_or(ErrorKind::Overflow)?;
        self.work.statistics.storage_bytes = narrow(current)?;
        self.work.statistics.storage_peak_bytes =
            self.work.statistics.storage_peak_bytes.max(narrow(peak)?);
        storage_limit(peak, self.limits)
    }
    fn reserve_buffers(&mut self, tuples: usize, guards: usize) -> Result<(), ErrorKind> {
        self.data.tuples = reserve(tuples, self.current(), self.limits, self.work)?;
        self.observe(0)?;
        self.data.guards = reserve(guards, self.current(), self.limits, self.work)?;
        self.observe(0)?;
        self.data.first_costs = reserve(tuples, self.current(), self.limits, self.work)?;
        self.observe(0)?;
        self.data.guard_costs = reserve(guards, self.current(), self.limits, self.work)?;
        self.observe(0)?;
        self.key_costs = reserve(tuples, self.current(), self.limits, self.work)?;
        self.observe(0)
    }
    fn logical(&mut self, added: u64) -> Result<(), ErrorKind> {
        self.work.statistics.resident_bytes = add(self.work.statistics.resident_bytes, added)?;
        self.work.statistics.peak_bytes =
            add(self.work.statistics.resident_bytes, self.logical_scratch)?;
        if self.work.statistics.peak_bytes > self.limits.max_bytes {
            return Err(ErrorKind::Limit(Resource::Bytes));
        }
        Ok(())
    }
    fn begin_tuple(
        &mut self,
        len: usize,
        capacity: usize,
        condition: usize,
    ) -> Result<(), ErrorKind> {
        self.work.charge(1)?;
        if condition >= self.data.theory.nodes().len() {
            return Err(ErrorKind::Condition {
                tuple: self.data.tuples.len(),
                node: condition,
            });
        }
        self.tuple_values = self
            .tuple_values
            .checked_add(len)
            .ok_or(ErrorKind::Overflow)?;
        ceiling(
            self.tuple_values,
            self.limits.max_tuple_values,
            Resource::TupleValues,
        )?;
        self.logical(bytes::<Term>(capacity)?)?;
        self.in_key = true;
        self.key_start = self.next_term;
        self.key_cost = 1;
        self.first_cost = 0;
        Ok(())
    }
    fn inspect(&mut self, value: TermRef<'_>, payload: u64) -> Result<u64, ErrorKind> {
        self.work.charge(1)?;
        self.value_nodes = self
            .value_nodes
            .checked_add(value.expanded_nodes())
            .ok_or(ErrorKind::Overflow)?;
        ceiling(
            self.value_nodes,
            self.limits.max_value_nodes,
            Resource::ValueNodes,
        )?;
        let cost = term_work(value, self.work)?;
        self.work.charge(cost)?;
        self.logical(payload)?;
        Ok(cost)
    }
    fn owned_value(&mut self, value: &Term) -> Result<usize, ErrorKind> {
        self.work.charge(1)?;
        let nodes = TermRef::from(value).expanded_nodes();
        self.work
            .charge(u64::try_from(nodes).map_err(|_| ErrorKind::Overflow)?)?;
        let cost = self.inspect(value.into(), narrow(value.payload_bytes() as u128)?)?;
        self.append(value.into(), cost)
    }
    fn borrowed_value(&mut self, value: TermRef<'_>) -> Result<usize, ErrorKind> {
        self.work.charge(1)?;
        self.authority
            .read()
            .term_key(value)
            .map_err(ErrorKind::Canonical)?;
        let payload = logical_payload(value, self.work)?;
        let cost = self.inspect(value, payload)?;
        self.append(value, cost)
    }
    fn append(&mut self, value: TermRef<'_>, cost: u64) -> Result<usize, ErrorKind> {
        let external = self.current() - self.authority.bytes();
        let imported = self
            .authority
            .import(value, external, self.limits, self.work)?;
        let value = if let Some(key) = &imported {
            self.authority
                .read()
                .term(key)
                .map_err(ErrorKind::Canonical)?
        } else {
            value
        };
        let other = self.authority.bytes() + data_bytes(&self.data)
            - self.data.components.storage_bytes()
            + self.key_costs.capacity() as u128 * size_of::<u64>() as u128;
        let maximum = available(self.limits, other)?;
        self.data.components.restart_storage_peak();
        let result = self.data.components.append_terms_with(
            self.authority.read(),
            [TemplateTerm::Constant(value)],
            maximum,
            || self.work.charge(1),
        );
        self.work.statistics.storage_peak_bytes = self
            .work
            .statistics
            .storage_peak_bytes
            .max(narrow(other + self.data.components.storage_peak_bytes())?);
        let range = result.map_err(component)?;
        self.observe(0)?;
        self.next_term = range.end;
        if self.in_key {
            if range.start == self.key_start {
                self.first_cost = cost;
            }
            self.key_cost = add(self.key_cost, cost)?;
        }
        Ok(range.start)
    }
    fn end_tuple(&mut self, condition: usize) -> Result<(), ErrorKind> {
        self.work.charge(1)?;
        self.data.tuples.push(TupleData {
            key: self.key_start..self.next_term,
            condition,
        });
        self.data.first_costs.push(self.first_cost);
        self.key_costs.push(self.key_cost);
        self.in_key = false;
        Ok(())
    }
    fn guard_costs(&mut self, cost: u64) -> Result<(), ErrorKind> {
        self.work.charge(cost)?;
        self.data.guard_costs.push(cost);
        Ok(())
    }
    fn finish(mut self) -> Result<(GroupData, A), ErrorKind> {
        let view = self
            .data
            .bind_with(self.authority.read(), || self.work.charge(1))
            .map_err(component)?;
        unique(
            view,
            &self.key_costs,
            self.current(),
            self.limits,
            self.work,
        )?;
        self.work.poll()?;
        self.key_costs.clear();
        drop(self.key_costs);
        self.work.statistics.storage_bytes =
            narrow(self.authority.bytes() + data_bytes(&self.data))?;
        Ok((self.data, self.authority))
    }
}

fn data_bytes(data: &GroupData) -> u128 {
    size_of::<GroupData>() as u128 - size_of::<TemplateComponents>() as u128
        + data.components.storage_bytes()
        + data.tuples.capacity() as u128 * size_of::<TupleData>() as u128
        + data.guards.capacity() as u128 * size_of::<GuardData>() as u128
        + (data.first_costs.capacity() as u128 + data.guard_costs.capacity() as u128)
            * size_of::<u64>() as u128
}
fn narrow(bytes: u128) -> Result<u64, ErrorKind> {
    u64::try_from(bytes).map_err(|_| ErrorKind::Overflow)
}
fn storage_limit(required: u128, limits: AdmissionLimits) -> Result<(), ErrorKind> {
    if required > u128::from(limits.max_storage_bytes) {
        Err(ErrorKind::Limit(Resource::StorageBytes))
    } else {
        Ok(())
    }
}
fn available(limits: AdmissionLimits, outside: u128) -> Result<usize, ErrorKind> {
    storage_limit(outside, limits)?;
    Ok(usize::try_from(u128::from(limits.max_storage_bytes) - outside).unwrap_or(usize::MAX))
}
fn reserve<T>(
    count: usize,
    current: u128,
    limits: AdmissionLimits,
    work: &mut Work<'_>,
) -> Result<Vec<T>, ErrorKind> {
    work.charge(1)?;
    storage_limit(current + count as u128 * size_of::<T>() as u128, limits)?;
    let result = super::storage(count)?;
    let peak = current + result.capacity() as u128 * size_of::<T>() as u128;
    work.statistics.storage_peak_bytes = work.statistics.storage_peak_bytes.max(narrow(peak)?);
    storage_limit(peak, limits)?;
    Ok(result)
}
fn catalog(error: CatalogError) -> ErrorKind {
    match error {
        CatalogError::Storage { .. } => ErrorKind::Limit(Resource::StorageBytes),
        CatalogError::Allocation => ErrorKind::Stopped(zetesis_cpu::Stop::Allocation),
        other => ErrorKind::Catalog(other),
    }
}
fn vocabulary(error: VocabularyFailure<ErrorKind>) -> ErrorKind {
    match error {
        VocabularyFailure::Storage(error) => catalog(error),
        VocabularyFailure::Stopped(error) => error,
    }
}
fn component(error: TemplateCatalogFailure<ErrorKind>) -> ErrorKind {
    match error {
        TemplateCatalogFailure::Storage(error) => catalog(error),
        TemplateCatalogFailure::Read(error) => ErrorKind::Canonical(error),
        TemplateCatalogFailure::Stopped(error) => error,
        TemplateCatalogFailure::Incomplete => ErrorKind::Overflow,
    }
}
fn component_never(error: TemplateCatalogFailure) -> ErrorKind {
    match error {
        TemplateCatalogFailure::Storage(error) => catalog(error),
        TemplateCatalogFailure::Read(error) => ErrorKind::Canonical(error),
        TemplateCatalogFailure::Stopped(never) => match never {},
        TemplateCatalogFailure::Incomplete => ErrorKind::Overflow,
    }
}
fn logical_payload(value: TermRef<'_>, work: &mut Work<'_>) -> Result<u64, ErrorKind> {
    work.charge(1)?;
    match value.descriptor() {
        ValueNodeRef::String(text) | ValueNodeRef::Symbol(text) => narrow(text.len() as u128),
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => {
            let nodes = value.expanded_nodes();
            let mut result = bytes::<zetesis_core::ValueNode>(nodes)?;
            result = add(result, narrow(value.rendered_bytes() as u128)?)?;
            for index in 0..nodes {
                let term = value
                    .subterm_with(index, || work.charge(1))?
                    .expect("admitted expanded rank");
                work.charge(1)?;
                let text = match term.descriptor() {
                    ValueNodeRef::String(text)
                    | ValueNodeRef::Symbol(text)
                    | ValueNodeRef::Function { name: text, .. } => text.len(),
                    _ => 0,
                };
                result = add(result, narrow(text as u128)?)?;
            }
            Ok(result)
        }
        _ => Ok(0),
    }
}
pub(super) fn term_work(value: TermRef<'_>, work: &mut Work<'_>) -> Result<u64, ErrorKind> {
    work.charge(1)?;
    let count = match value.descriptor() {
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => {
            value.canonical_bytes_with(|| work.charge(1))?
        }
        ValueNodeRef::String(text) | ValueNodeRef::Symbol(text) => {
            text.len().checked_add(1).ok_or(ErrorKind::Overflow)?
        }
        _ => 1,
    };
    u64::try_from(count).map_err(|_| ErrorKind::Overflow)
}
fn unique(
    group: GroupRef<'_>,
    costs: &[u64],
    current: u128,
    limits: AdmissionLimits,
    work: &mut Work<'_>,
) -> Result<(), ErrorKind> {
    let count = group.tuples().len();
    let mut order = reserve(count, current, limits, work)?;
    let mut buffer = reserve(
        count,
        current + order.capacity() as u128 * size_of::<usize>() as u128,
        limits,
        work,
    )?;
    for index in 0..count {
        work.charge(2)?;
        order.push(index);
        buffer.push(index);
    }
    let mut width = 1_usize;
    while width < count {
        let mut start = 0;
        while start < count {
            let middle = start.saturating_add(width).min(count);
            let end = middle.saturating_add(width).min(count);
            merge(
                group,
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
        if compare(group, costs, pair[0], pair[1], work)?.is_eq() {
            return Err(ErrorKind::Duplicate {
                first: pair[0],
                second: pair[1],
            });
        }
    }
    Ok(())
}
fn merge(
    group: GroupRef<'_>,
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
                && !compare(group, costs, order[left.start], order[right.start], work)?.is_gt());
        let next = if take_left { &mut left } else { &mut right };
        *slot = order[next.start];
        next.start += 1;
    }
    Ok(())
}
fn compare(
    group: GroupRef<'_>,
    costs: &[u64],
    left: usize,
    right: usize,
    work: &mut Work<'_>,
) -> Result<Ordering, ErrorKind> {
    work.charge(add(costs[left], costs[right])?)?;
    let left = group.tuples().at(left).expect("sort coordinate").key;
    let right = group.tuples().at(right).expect("sort coordinate").key;
    for index in 0..left.len().min(right.len()) {
        work.charge(1)?;
        let order = left
            .at(index)
            .expect("key extent")
            .compare_terms_with(right.at(index).expect("key extent"), || work.charge(1))?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(left.len().cmp(&right.len()))
}
fn ceiling(actual: usize, maximum: usize, resource: Resource) -> Result<(), ErrorKind> {
    if actual > maximum {
        Err(ErrorKind::Limit(resource))
    } else {
        Ok(())
    }
}
