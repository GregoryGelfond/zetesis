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

/// Lower/upper insertion point in the complete dictionary. Guard bounds need
/// not occur in the row; all comparisons use their existing typed authority.
pub(super) fn insertion(
    builder: &mut Builder<'_, '_>,
    bound: TermRef<'_>,
    order: &[usize],
    elements: &[ValueExtremumElement<TermRef<'_>>],
    inclusive: bool,
) -> Result<u64> {
    let mut low = 0;
    let mut high = order.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let compared = compare(builder, value(order[middle], elements), bound)?;
        if compared.is_lt() || (inclusive && compared.is_eq()) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    u64::try_from(low).map_err(|_| AggregateErrorKind::ArithmeticOverflow)
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
