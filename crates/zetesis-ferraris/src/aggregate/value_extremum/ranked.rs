//! Guard-independent ASP value order with both empty-selection endpoints.
use std::cmp::Ordering;
use zetesis_core::{Value, catalog::TermRef};

use super::ValueExtremumElement;
use crate::AggregateErrorKind;
use crate::aggregate::lower::Builder;

type Result<T> = std::result::Result<T, AggregateErrorKind>;

static INFIMUM: Value = Value::Infimum;
static SUPREMUM: Value = Value::Supremum;

/// Sort occurrence indexes plus two virtual endpoints, compact equal values in
/// the dictionary only, and assign every original occurrence its complete rank.
/// The caller supplies initialized index/rank cells and retains both buffers.
pub(super) fn prepare(
    builder: &mut Builder<'_, '_>,
    order: &mut [usize],
    ranks: &mut [u64],
    elements: &[ValueExtremumElement<TermRef<'_>>],
) -> Result<usize> {
    if let Some(distinct) = ordered_ranks(builder, ranks, elements)? {
        compact_ordered(builder, order, ranks, elements.len(), distinct)?;
        return Ok(distinct);
    }
    sort(builder, order, elements)?;
    let mut distinct = 0;
    for position in 0..order.len() {
        builder.tick()?;
        let index = order[position];
        if distinct == 0
            || !compare(
                builder,
                value(order[distinct - 1], elements),
                value(index, elements),
            )?
            .is_eq()
        {
            builder.tick()?;
            order[distinct] = index;
            distinct += 1;
        }
        if index < elements.len() {
            let rank =
                u64::try_from(distinct - 1).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
            builder.tick()?;
            ranks[index] = rank;
        }
    }
    Ok(distinct)
}

fn value<'a>(index: usize, elements: &[ValueExtremumElement<TermRef<'a>>]) -> TermRef<'a> {
    match index.cmp(&elements.len()) {
        Ordering::Less => elements[index].value,
        Ordering::Equal => (&INFIMUM).into(),
        Ordering::Greater => (&SUPREMUM).into(),
    }
}

fn compare(
    builder: &mut Builder<'_, '_>,
    left: TermRef<'_>,
    right: TermRef<'_>,
) -> Result<Ordering> {
    builder.tick()?;
    left.compare_terms_with(right, || builder.tick())
}

/// Check ASP term order while assigning complete ranks. Numeric contribution
/// order already agrees; mixed storage-identity order can differ. The public
/// family API verifies every comparison rather than trusting the input order.
/// An inversion leaves the dictionary untouched and its incomplete ranks are
/// overwritten by the ordinary sorting path before anything is published.
fn ordered_ranks(
    builder: &mut Builder<'_, '_>,
    ranks: &mut [u64],
    elements: &[ValueExtremumElement<TermRef<'_>>],
) -> Result<Option<usize>> {
    let mut previous = (&INFIMUM).into();
    let mut distinct = 1usize;
    for (rank, element) in ranks.iter_mut().zip(elements) {
        match compare(builder, previous, element.value)? {
            Ordering::Greater => return Ok(None),
            Ordering::Less => distinct += 1,
            Ordering::Equal => {}
        }
        builder.tick()?;
        *rank = u64::try_from(distinct - 1).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
        previous = element.value;
    }
    if !compare(builder, previous, (&SUPREMUM).into())?.is_eq() {
        distinct += 1;
    }
    Ok(Some(distinct))
}

/// Reuse the checked ranks to compact the dictionary without comparing typed
/// values again. Rank zero and the last rank include the virtual endpoints;
/// repeated first values still retain every original condition occurrence.
fn compact_ordered(
    builder: &mut Builder<'_, '_>,
    order: &mut [usize],
    ranks: &[u64],
    elements: usize,
    distinct: usize,
) -> Result<()> {
    builder.tick()?;
    order[0] = elements;
    let mut retained = 1usize;
    for (index, &rank) in ranks.iter().enumerate() {
        builder.tick()?;
        if rank == u64::try_from(retained).map_err(|_| AggregateErrorKind::ArithmeticOverflow)? {
            builder.tick()?;
            order[retained] = index;
            retained += 1;
        }
    }
    if retained < distinct {
        builder.tick()?;
        order[retained] = elements + 1;
    }
    Ok(())
}

/// Both insertion points in one search of the unique complete dictionary.
/// Equality has exactly one position; an absent bound has identical lower and
/// upper positions. Guard bounds keep their own typed authority and need not
/// occur in the row.
pub(super) fn cuts(
    builder: &mut Builder<'_, '_>,
    bound: TermRef<'_>,
    order: &[usize],
    elements: &[ValueExtremumElement<TermRef<'_>>],
) -> Result<(u64, u64)> {
    let mut low = 0;
    let mut high = order.len();
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(builder, value(order[middle], elements), bound)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => {
                let lower =
                    u64::try_from(middle).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
                return Ok((lower, lower + 1));
            }
        }
    }
    let boundary = u64::try_from(low).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
    Ok((boundary, boundary))
}

/// In-place heap sorting charges every fallible comparison and swap before its
/// mutation; an infallible comparator never conceals control/work failures.
fn sort(
    builder: &mut Builder<'_, '_>,
    order: &mut [usize],
    elements: &[ValueExtremumElement<TermRef<'_>>],
) -> Result<()> {
    for root in (0..order.len() / 2).rev() {
        builder.tick()?;
        sift(builder, order, root, elements)?;
    }
    for end in (1..order.len()).rev() {
        builder.tick()?;
        order.swap(0, end);
        sift(builder, &mut order[..end], 0, elements)?;
    }
    Ok(())
}

fn sift(
    builder: &mut Builder<'_, '_>,
    order: &mut [usize],
    mut root: usize,
    elements: &[ValueExtremumElement<TermRef<'_>>],
) -> Result<()> {
    while root < order.len() / 2 {
        builder.tick()?;
        let mut child = root * 2 + 1;
        if child + 1 < order.len()
            && compare(
                builder,
                value(order[child], elements),
                value(order[child + 1], elements),
            )?
            .is_lt()
        {
            child += 1;
        }
        if !compare(
            builder,
            value(order[root], elements),
            value(order[child], elements),
        )?
        .is_lt()
        {
            break;
        }
        builder.tick()?;
        order.swap(root, child);
        root = child;
    }
    Ok(())
}
