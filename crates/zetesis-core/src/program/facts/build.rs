//! Checked transition from compact inputs to one closed fact authority.

use super::input::{self, Columns, FactSegment};
use super::{Block, Data, Occurrence};
use crate::atom_interner::{AtomInterner, Failure as AtomFailure, Limits as AtomLimits};
use crate::catalog::storage::{PredicateId, TermId};
use crate::catalog::{Error, PredicateRef, TermRef};
use crate::program::admission::{Admission, Symbols};
use crate::relation::{Catalog, Failure as RelationFailure, Limits as RelationLimits, Resource};
use crate::template::catalog::TermData;
use crate::{
    AdmissionError, AdmissionLimits, AdmissionOrigin, AdmissionResource, PatternRef,
    TemplateCatalog, TemplateTerm, Value,
};
use std::convert::Infallible;

#[derive(Default)]
pub(in crate::program) struct Prepared {
    blocks: Vec<Input>,
}
struct Input {
    before: usize,
    first: usize,
    len: usize,
    predicate: PredicateId,
    columns: Vec<Column>,
    product: bool,
}
struct Column {
    values: Vec<TermId>,
    stride: usize,
}
impl Prepared {
    pub(in crate::program) fn bytes(&self) -> u128 {
        self.blocks.capacity() as u128 * size_of::<Input>() as u128
            + self
                .blocks
                .iter()
                .map(|block| {
                    block.columns.capacity() as u128 * size_of::<Column>() as u128
                        + block
                            .columns
                            .iter()
                            .map(|column| {
                                column.values.capacity() as u128 * size_of::<TermId>() as u128
                            })
                            .sum::<u128>()
                })
                .sum::<u128>()
    }
}

pub(in crate::program) fn append(
    admission: &mut Admission,
    prepared: &mut Prepared,
    input: input::Block,
    symbols: &mut Symbols,
) -> Result<(), AdmissionError> {
    admission.template = None;
    admission.fact = Some(input.first);
    if input.predicate.arity() > admission.limits.max_predicate_arity {
        return Err(AdmissionError::FactLimitExceeded {
            resource: AdmissionResource::PredicateArity,
            limit: admission.limits.max_predicate_arity,
            actual: input.predicate.arity(),
            fact: input.first,
        });
    }
    let predicate = admission
        .builder
        .predicate((&input.predicate).into())
        .map_err(|error| admission.error(error))?;
    admission.push(&mut symbols.predicates, predicate)?;
    let mut columns = Vec::new();
    admission.reserve(&mut columns, input.predicate.arity())?;
    let product = matches!(&input.columns, Columns::Product(_));
    match input.columns {
        Columns::Product(inputs) => product_columns(
            admission,
            inputs,
            input.first,
            input.len,
            symbols,
            &mut columns,
        )?,
        Columns::Run(inputs) => run_columns(admission, inputs, input.first, symbols, &mut columns)?,
    }
    admission.reserve(&mut prepared.blocks, 1)?;
    prepared.blocks.push(Input {
        before: input.before,
        first: input.first,
        len: input.len,
        predicate,
        columns,
        product,
    });
    Ok(())
}

/// Column storage still carries the source row of every scalar import. A
/// failed later row must not inherit its fused run's first evidence position.
fn run_columns(
    admission: &mut Admission,
    inputs: Vec<Vec<Value>>,
    first: usize,
    symbols: &mut Symbols,
    columns: &mut Vec<Column>,
) -> Result<(), AdmissionError> {
    for input in inputs {
        let mut values = Vec::new();
        for (row, value) in input.into_iter().enumerate() {
            admission.fact = Some(first + row);
            push_value(admission, &mut values, &value, symbols)?;
        }
        columns.push(Column { values, stride: 1 });
    }
    Ok(())
}

/// An alternative first occurs with every earlier and later column at its first
/// position: its local alternative index times this column's suffix stride.
fn product_columns(
    admission: &mut Admission,
    inputs: Vec<Vec<FactSegment>>,
    first: usize,
    len: usize,
    symbols: &mut Symbols,
    columns: &mut Vec<Column>,
) -> Result<(), AdmissionError> {
    let mut stride = len;
    for input in inputs {
        let width = input
            .iter()
            .try_fold(0_usize, |sum, segment| {
                sum.checked_add(segment.len()?).ok_or(Error::Overflow)
            })
            .map_err(|error| admission.error(error))?;
        stride = stride
            .checked_div(width)
            .ok_or_else(|| admission.error(Error::Shape))?;
        let mut values = Vec::new();
        for segment in input {
            match segment {
                FactSegment::Value(value) => {
                    alternative(admission, &mut values, &value, first, stride, symbols)?;
                }
                FactSegment::Interval {
                    first: lower,
                    last: upper,
                } => {
                    for number in i64::from(lower)..=i64::from(upper) {
                        let value =
                            Value::Number(i32::try_from(number).expect("bounded input interval"));
                        alternative(admission, &mut values, &value, first, stride, symbols)?;
                    }
                }
            }
        }
        columns.push(Column { values, stride });
    }
    Ok(())
}

fn alternative(
    admission: &mut Admission,
    values: &mut Vec<TermId>,
    value: &Value,
    first: usize,
    stride: usize,
    symbols: &mut Symbols,
) -> Result<(), AdmissionError> {
    let row = values
        .len()
        .checked_mul(stride)
        .and_then(|offset| first.checked_add(offset))
        .ok_or_else(|| admission.error(Error::Overflow))?;
    admission.fact = Some(row);
    push_value(admission, values, value, symbols)
}

fn push_value(
    admission: &mut Admission,
    values: &mut Vec<TermId>,
    value: &Value,
    symbols: &mut Symbols,
) -> Result<(), AdmissionError> {
    let TermData::Constant(id) = admission
        .builder
        .term(TemplateTerm::Constant(value.into()))
        .map_err(|error| admission.error(error))?
    else {
        unreachable!("closed fact value")
    };
    admission.push(values, id)?;
    admission.push(&mut symbols.domain, id)
}

/// All operations include the still-live template authority and imported-column
/// scratch. The latter is released by the caller only after this consuming close.
pub(in crate::program) fn close(
    catalog: &TemplateCatalog,
    prepared: Prepared,
    base: u128,
    limits: AdmissionLimits,
) -> Result<Option<Data>, AdmissionError> {
    if prepared.blocks.is_empty() {
        return Ok(None);
    }
    let mut builder = Builder {
        base,
        extra: 0,
        limits,
        vocabulary: catalog.shared_vocabulary_bytes(),
        fact: None,
    };
    let mut writer = AtomInterner::for_template_catalog(catalog, builder.writer_limit()?)
        .map_err(|error| builder.catalog_error(error, 0))?;
    let mut rows = Rows::default();
    builder.reserve(
        &mut rows.slots,
        catalog.read().predicate_ids().len(),
        writer.storage_bytes(),
    )?;
    rows.slots
        .resize(catalog.read().predicate_ids().len(), None);
    for input in &prepared.blocks {
        rows.append(input, catalog, &mut writer, &mut builder)?;
    }
    builder.fact = None;
    let relation_order = rows.ordered(&writer, &mut builder)?;
    builder.release(rows.slots);
    let allowance = builder.atom_limits_with(rows.bytes)?;
    let store = writer
        .into_closed_with(allowance, || Ok::<_, Infallible>(()))
        .map_err(|error| builder.atom_error(error.into_parts().0, rows.bytes))?;
    builder.admit(store.publication_peak_bytes() + rows.bytes)?;
    drop(prepared);
    Ok(Some(Data {
        store,
        relations: rows.relations,
        relation_order,
        occurrences: rows.occurrences,
        blocks: rows.blocks,
    }))
}

#[derive(Default)]
struct Rows {
    relations: Vec<Catalog>,
    /// Catalog payload only: headers are already in the relations vector.
    bytes: u128,
    occurrences: Vec<Occurrence>,
    blocks: Vec<Block>,
    slots: Vec<Option<usize>>,
}

impl Rows {
    fn relation(
        &mut self,
        predicate: PredicateRef<'_>,
        id: PredicateId,
        writer: &mut AtomInterner,
        builder: &mut Builder,
    ) -> Result<usize, AdmissionError> {
        if let Some(index) = self.slots[id.position()] {
            return Ok(index);
        }
        builder.reserve(&mut self.relations, 1, writer.storage_bytes() + self.bytes)?;
        let allowance = builder.atom_limits_with(self.bytes)?;
        let declaration = writer
            .declare_predicate_with(predicate, allowance, || Ok::<_, Infallible>(()))
            .map_err(|error| builder.atom_error(error, self.bytes))?;
        let allowance = builder.relation_limits(
            writer.storage_bytes(),
            self.bytes,
            size_of::<Catalog>() as u128,
        )?;
        let relation = Catalog::new(writer.read(), declaration, allowance).map_err(|error| {
            builder.relation_error(
                &error.error,
                writer.storage_bytes() + self.bytes,
                size_of::<Catalog>() as u128,
            )
        })?;
        self.bytes += relation.retained_bytes() as u128 - size_of::<Catalog>() as u128;
        builder.admit(writer.storage_bytes() + self.bytes)?;
        let index = self.relations.len();
        self.relations.push(relation);
        self.slots[id.position()] = Some(index);
        Ok(index)
    }

    fn append(
        &mut self,
        input: &Input,
        catalog: &TemplateCatalog,
        writer: &mut AtomInterner,
        builder: &mut Builder,
    ) -> Result<(), AdmissionError> {
        builder.fact = Some(input.first);
        let predicate =
            PredicateRef::new(catalog.read(), input.predicate).expect("admitted fact predicate");
        let relation = self.relation(predicate, input.predicate, writer, builder)?;
        let mut terms = Vec::new();
        builder.reserve(
            &mut terms,
            input.columns.len(),
            writer.storage_bytes() + self.bytes,
        )?;
        let start = self.occurrences.len();
        builder.reserve(
            &mut self.occurrences,
            input.len,
            writer.storage_bytes() + self.bytes,
        )?;
        for row in 0..input.len {
            builder.fact = Some(input.first + row);
            terms.clear();
            for column in &input.columns {
                let index = if input.product {
                    (row / column.stride) % column.values.len()
                } else {
                    row
                };
                terms.push(TemplateTerm::Constant(
                    TermRef::new(catalog.read(), column.values[index])
                        .expect("admitted fact column"),
                ));
            }
            let pattern = PatternRef::from_parts(predicate, &terms).expect("checked fact arity");
            let key = pattern.key(&[] as &[Value]).expect("closed fact pattern");
            let allowance = builder.atom_limits_with(self.bytes)?;
            let position = writer
                .entry_key_with(key, allowance, || Ok::<_, Infallible>(()))
                .and_then(|entry| entry.insert_with(allowance, || Ok::<_, Infallible>(())))
                .map_err(|error| builder.atom_error(error, self.bytes))?;
            self.insert(relation, position, writer, builder)?;
        }
        builder.release(terms);
        builder.reserve(&mut self.blocks, 1, writer.storage_bytes() + self.bytes)?;
        self.blocks.push(Block {
            before: input.before,
            occurrences: start..self.occurrences.len(),
        });
        Ok(())
    }

    fn insert(
        &mut self,
        relation: usize,
        position: usize,
        writer: &AtomInterner,
        builder: &Builder,
    ) -> Result<(), AdmissionError> {
        let old = self.relations[relation].retained_bytes() as u128;
        let other = self.bytes - (old - size_of::<Catalog>() as u128);
        let allowance =
            builder.relation_limits(writer.storage_bytes(), other, size_of::<Catalog>() as u128)?;
        let atom = writer.get(position).expect("inserted fact atom");
        let insertion = self.relations[relation]
            .insert(atom, allowance)
            .map_err(|error| {
                builder.relation_error(
                    &error.error,
                    writer.storage_bytes() + other,
                    size_of::<Catalog>() as u128,
                )
            })?;
        self.bytes = self.bytes + insertion.storage.retained_bytes as u128 - old;
        builder.admit(writer.storage_bytes() + self.bytes)?;
        self.occurrences.push(Occurrence {
            relation,
            row: insertion.row,
        });
        Ok(())
    }

    fn ordered(
        &mut self,
        writer: &AtomInterner,
        builder: &mut Builder,
    ) -> Result<Vec<usize>, AdmissionError> {
        for relation in &mut self.relations {
            let old = relation.retained_bytes() as u128;
            let other = self.bytes - (old - size_of::<Catalog>() as u128);
            let allowance = builder.relation_limits(
                writer.storage_bytes(),
                other,
                size_of::<Catalog>() as u128,
            )?;
            let prepared = relation
                .prepare_ordered(writer.read(), allowance)
                .map_err(|error| {
                    builder.relation_error(
                        &error.error,
                        writer.storage_bytes() + other,
                        size_of::<Catalog>() as u128,
                    )
                })?;
            self.bytes = self.bytes + prepared.storage.retained_bytes as u128 - old;
            builder.admit(writer.storage_bytes() + self.bytes)?;
        }
        let mut order = Vec::new();
        builder.reserve(
            &mut order,
            self.relations.len(),
            writer.storage_bytes() + self.bytes,
        )?;
        order.extend(0..self.relations.len());
        order.sort_unstable_by(|&left, &right| {
            self.relations[left]
                .predicate(writer.read())
                .expect("closed fact predicate")
                .cmp(
                    &self.relations[right]
                        .predicate(writer.read())
                        .expect("closed fact predicate"),
                )
        });
        Ok(order)
    }
}

struct Builder {
    base: u128,
    extra: u128,
    limits: AdmissionLimits,
    vocabulary: u128,
    fact: Option<usize>,
}
impl Builder {
    fn error(&self, error: Error) -> AdmissionError {
        AdmissionError::Canonical {
            error,
            origin: self
                .fact
                .map_or(AdmissionOrigin::Program, AdmissionOrigin::Fact),
        }
    }
    fn admit(&self, writer_and_relations: u128) -> Result<(), AdmissionError> {
        let required = self.base + self.extra + writer_and_relations - self.vocabulary;
        if required > self.limits.max_bytes as u128 {
            Err(self.error(Error::Storage {
                required,
                limit: self.limits.max_bytes,
            }))
        } else {
            Ok(())
        }
    }
    fn writer_limit(&self) -> Result<usize, AdmissionError> {
        let other = self.base + self.extra - self.vocabulary;
        usize::try_from(
            (self.limits.max_bytes as u128)
                .checked_sub(other)
                .ok_or_else(|| {
                    self.error(Error::Storage {
                        required: other,
                        limit: self.limits.max_bytes,
                    })
                })?,
        )
        .map_err(|_| self.error(Error::Overflow))
    }
    fn atom_limits_with(&self, relations: u128) -> Result<AtomLimits, AdmissionError> {
        let limit = self
            .writer_limit()?
            .checked_sub(usize::try_from(relations).map_err(|_| self.error(Error::Overflow))?)
            .ok_or_else(|| {
                self.error(Error::Storage {
                    required: self.base + self.extra + relations,
                    limit: self.limits.max_bytes,
                })
            })?;
        Ok(AtomLimits::for_atoms(self.limits.max_templates, limit))
    }
    fn relation_limits(
        &self,
        writer: u128,
        other_relations: u128,
        included_header: u128,
    ) -> Result<RelationLimits, AdmissionError> {
        let other =
            self.base + self.extra + writer - self.vocabulary + other_relations - included_header;
        let max_bytes = usize::try_from(
            (self.limits.max_bytes as u128)
                .checked_sub(other)
                .ok_or_else(|| {
                    self.error(Error::Storage {
                        required: other,
                        limit: self.limits.max_bytes,
                    })
                })?,
        )
        .map_err(|_| self.error(Error::Overflow))?;
        Ok(RelationLimits {
            max_rows: self.limits.max_templates,
            max_columns: self.limits.max_predicate_arity,
            max_values: self.limits.max_domain_values,
            max_bytes,
            max_work: u64::MAX,
        })
    }
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        writer_and_relations: u128,
    ) -> Result<(), AdmissionError> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.error(Error::Overflow))?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = required
            .max(
                values
                    .capacity()
                    .checked_mul(2)
                    .ok_or_else(|| self.error(Error::Overflow))?,
            )
            .max(4);
        let old = values.capacity() as u128 * size_of::<T>() as u128;
        self.admit(writer_and_relations + target as u128 * size_of::<T>() as u128)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| self.error(Error::Allocation))?;
        let new = values.capacity() as u128 * size_of::<T>() as u128;
        self.admit(writer_and_relations + new)?;
        self.extra += new - old;
        self.admit(writer_and_relations)
    }
    fn release<T>(&mut self, values: Vec<T>) {
        self.extra -= values.capacity() as u128 * size_of::<T>() as u128;
        drop(values);
    }
    fn catalog_error(&self, error: Error, relations: u128) -> AdmissionError {
        match error {
            Error::Storage { required, .. } => self.error(Error::Storage {
                required: self.base + self.extra + relations + required - self.vocabulary,
                limit: self.limits.max_bytes,
            }),
            error => self.error(error),
        }
    }
    fn atom_error(&self, error: AtomFailure<Infallible>, relations: u128) -> AdmissionError {
        match error {
            AtomFailure::Catalog(Error::Storage { required, .. })
            | AtomFailure::Bytes { required, .. } => self.error(Error::Storage {
                required: self.base + self.extra + relations + required - self.vocabulary,
                limit: self.limits.max_bytes,
            }),
            AtomFailure::Catalog(error) => self.error(error),
            AtomFailure::Stopped(never) => match never {},
            AtomFailure::Allocation(_) => self.error(Error::Allocation),
            AtomFailure::Overflow => self.error(Error::Overflow),
            AtomFailure::Atoms { required, limit } => match self.fact {
                Some(fact) => AdmissionError::FactLimitExceeded {
                    resource: AdmissionResource::Templates,
                    limit,
                    actual: required,
                    fact,
                },
                None => AdmissionError::LimitExceeded {
                    resource: AdmissionResource::Templates,
                    limit,
                    actual: required,
                    template: None,
                },
            },
        }
    }
    fn relation_error(
        &self,
        error: &RelationFailure,
        writer_and_others: u128,
        included_header: u128,
    ) -> AdmissionError {
        self.error(match error {
            RelationFailure::Allocation => Error::Allocation,
            RelationFailure::Limit {
                resource: Resource::Bytes,
                observed,
                ..
            } => Error::Storage {
                required: self.base + self.extra + writer_and_others + observed
                    - self.vocabulary
                    - included_header,
                limit: self.limits.max_bytes,
            },
            RelationFailure::Overflow => Error::Overflow,
            _ => Error::Shape,
        })
    }
}
