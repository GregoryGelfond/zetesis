//! Construction by checked sorting and binary lookup, without value cloning.

use std::{cmp::Ordering, mem::size_of};

use crate::{Predicate, Value};

use super::{
    Cell, Failure, Layout, LayoutOwner, Limits, Relation, Resource, Source, Storage, Work, ceiling,
};

pub(super) fn build<'source>(
    predicate: &'source Predicate,
    source: Source<'source>,
    limits: Limits,
) -> Result<Relation<'source>, Failure> {
    ceiling(
        Resource::Rows,
        source.len() as u128,
        limits.max_rows as u128,
    )?;
    ceiling(
        Resource::Columns,
        predicate.arity() as u128,
        limits.max_columns as u128,
    )?;
    let cells = source
        .len()
        .checked_mul(predicate.arity())
        .ok_or(Failure::Overflow)?;
    let mapping = match &source {
        Source::Atoms(_) => 0,
        Source::Catalog { indices, .. } => indices
            .len()
            .checked_mul(size_of::<usize>())
            .ok_or(Failure::Overflow)?,
    };
    let mut work = Work::new(limits, size_of::<Relation<'_>>() as u128)?;
    let payload = validate(predicate, &source, &mut work)?;
    let mut values = work.reserve(cells)?;
    for row in 0..source.len() {
        let atom = source.atom(row).ok_or(Failure::CatalogIndex)?;
        for column in 0..atom.values().len() {
            work.tick(1)?;
            values.push(Cell { row, column });
        }
    }
    sort(&mut values, &source, &mut work)?;
    let dictionary = dictionary(&values, &source, &mut work)?;
    work.release(values);
    let mut ordered = work.reserve(dictionary.len())?;
    for id in 0..dictionary.len() {
        work.tick(1)?;
        ordered.push(u32::try_from(id).map_err(|_| Failure::Overflow)?);
    }
    let mut columns = work.reserve(predicate.arity())?;
    for _ in 0..predicate.arity() {
        let mut column = work.reserve(source.len())?;
        work.tick(source.len() as u128)?;
        column.resize(source.len(), 0);
        columns.push(column);
    }
    let mut layout = Layout {
        dictionary,
        ordered,
        columns,
    };
    for row in 0..source.len() {
        let atom = source.atom(row).ok_or(Failure::CatalogIndex)?;
        for (column, value) in atom.values().iter().enumerate() {
            let id =
                lookup(&layout, &source, value, &mut work)?.map_err(|_| Failure::Dictionary)?;
            work.tick(1)?;
            layout.columns[column][row] = id;
        }
    }
    Ok(Relation {
        predicate,
        source,
        layout: LayoutOwner::Owned(layout),
        storage: Storage {
            retained_bytes: work.live,
            peak_construction_bytes: work.peak,
            referenced_payload_bytes: payload,
            borrowed_mapping_bytes: mapping,
            construction_work: work.used,
        },
    })
}

fn validate(predicate: &Predicate, source: &Source<'_>, work: &mut Work) -> Result<u128, Failure> {
    let mut payload = 0_u128;
    for row in 0..source.len() {
        let atom = source.atom(row).ok_or(Failure::CatalogIndex)?;
        work.tick(1 + predicate.name().len() as u128 + atom.predicate().name().len() as u128)?;
        if atom.predicate() != predicate {
            return Err(Failure::Predicate);
        }
        for value in atom.values() {
            work.tick(1 + value.payload_bytes() as u128)?;
            payload = payload
                .checked_add(value.payload_bytes() as u128)
                .ok_or(Failure::Overflow)?;
        }
    }
    Ok(payload)
}

/// Width doubles after each complete pass. Before a pass, adjacent runs of at
/// most width are ordered; merging preserves every reference exactly once.
/// A failed comparison discards all construction buffers and publishes no view.
fn sort(values: &mut Vec<Cell>, source: &Source<'_>, work: &mut Work) -> Result<(), Failure> {
    if values.len() < 2 {
        return Ok(());
    }
    let mut scratch = work.reserve(values.len())?;
    work.tick(values.len() as u128)?;
    scratch.extend_from_slice(values);
    let mut width = 1;
    while width < values.len() {
        let mut start = 0;
        while start < values.len() {
            let middle = start.saturating_add(width).min(values.len());
            let end = middle.saturating_add(width).min(values.len());
            merge(values, &mut scratch, start, middle, end, source, work)?;
            start = end;
        }
        std::mem::swap(values, &mut scratch);
        width = width.saturating_mul(2);
    }
    work.release(scratch);
    Ok(())
}

fn merge(
    cells: &[Cell],
    target: &mut [Cell],
    start: usize,
    middle: usize,
    end: usize,
    source: &Source<'_>,
    work: &mut Work,
) -> Result<(), Failure> {
    let mut left = start;
    let mut right = middle;
    for destination in &mut target[start..end] {
        work.tick(1)?;
        let take_left = left < middle
            && (right == end
                || work.compare(cells[left].value(source)?, cells[right].value(source)?)?
                    != Ordering::Greater);
        if take_left {
            *destination = cells[left];
            left += 1;
        } else {
            *destination = cells[right];
            right += 1;
        }
    }
    Ok(())
}

/// The sorted reference stream supplies one dictionary entry per typed value.
/// A second pass fills a compact reserved vector, so duplicate argument cells
/// do not remain as retained dictionary capacity.
fn dictionary(values: &[Cell], source: &Source<'_>, work: &mut Work) -> Result<Vec<Cell>, Failure> {
    let mut distinct = 0_usize;
    let mut previous = None;
    for &value in values {
        if differs(previous, value, source, work)? {
            distinct += 1;
        }
        previous = Some(value);
    }
    ceiling(
        Resource::Values,
        distinct as u128,
        (work.limits.max_values as u128).min(u128::from(u32::MAX) + 1),
    )?;
    let mut dictionary = work.reserve(distinct)?;
    previous = None;
    for &value in values {
        if differs(previous, value, source, work)? {
            work.tick(1)?;
            dictionary.push(value);
        }
        previous = Some(value);
    }
    Ok(dictionary)
}

fn differs(
    previous: Option<Cell>,
    value: Cell,
    source: &Source<'_>,
    work: &mut Work,
) -> Result<bool, Failure> {
    if let Some(previous) = previous {
        Ok(work.compare(previous.value(source)?, value.value(source)?)? != Ordering::Equal)
    } else {
        work.tick(1)?;
        Ok(true)
    }
}

/// Found equality ID, or the insertion position in the ordered-ID vector.
pub(super) fn lookup(
    layout: &Layout,
    source: &Source<'_>,
    value: &Value,
    work: &mut Work,
) -> Result<Result<u32, usize>, Failure> {
    let mut start = 0;
    let mut end = layout.ordered.len();
    while start < end {
        let middle = start + (end - start) / 2;
        let id = layout.ordered[middle];
        match work.compare(layout.dictionary[id as usize].value(source)?, value)? {
            Ordering::Less => start = middle + 1,
            Ordering::Equal => return Ok(Ok(id)),
            Ordering::Greater => end = middle,
        }
    }
    Ok(Err(start))
}
