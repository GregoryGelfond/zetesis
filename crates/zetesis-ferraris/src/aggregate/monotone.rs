//! Nonnegative comparisons share a row; Boolean boundary cases stay explicit.

use super::lower::{Builder, reserve};
use super::{AggregateComparison, AggregateElement, AggregateErrorKind, AggregateGuard};
use crate::NodeView;

type Result<T> = std::result::Result<T, AggregateErrorKind>;

#[derive(Clone, Copy)]
enum Reading {
    Truth,
    Falsum,
    All,
    Any,
    Threshold,
}

struct Shape {
    total: i128,
    minimum: i128,
}

impl Shape {
    fn reading(&self, bound: i128) -> Reading {
        if bound <= 0 {
            Reading::Truth
        } else if bound > self.total {
            Reading::Falsum
        } else if bound > self.total - self.minimum {
            Reading::All
        } else if bound <= self.minimum {
            Reading::Any
        } else {
            Reading::Threshold
        }
    }
}

fn bounds(guard: AggregateGuard) -> impl Iterator<Item = i128> {
    let bound = i128::from(guard.bound);
    let lower = if matches!(
        guard.comparison,
        AggregateComparison::Gt | AggregateComparison::Le
    ) {
        bound + 1
    } else {
        bound
    };
    std::iter::once(lower).chain(
        matches!(
            guard.comparison,
            AggregateComparison::Eq | AggregateComparison::Ne
        )
        .then_some(bound + 1),
    )
}

/// All children precede this family and all weights have been validated.
/// A row position is a distinct tuple contribution, even when conditions repeat.
/// Boundary-case replacements hold in the original theory and its frozen reduct.
pub(super) fn compile(
    builder: &mut Builder<'_, '_>,
    elements: &[AggregateElement],
    guards: &[AggregateGuard],
    total: i128,
    falsum: usize,
    truth: usize,
) -> Result<Vec<usize>> {
    let mut minimum = i128::MAX;
    let mut nonzero = 0usize;
    for element in elements {
        builder.tick()?;
        if element.weight > 0 {
            minimum = minimum.min(i128::from(element.weight));
            nonzero += 1;
        }
    }
    let shape = Shape { total, minimum };
    let mut count = 0usize;
    let mut has_row = false;
    for &guard in guards {
        for bound in bounds(guard) {
            builder.tick()?;
            match shape.reading(bound) {
                Reading::Threshold => {
                    count = count.checked_add(1).ok_or(AggregateErrorKind::StateLimit)?;
                    has_row = true;
                }
                Reading::All | Reading::Any => has_row = true,
                Reading::Truth | Reading::Falsum => {}
            }
        }
    }
    let cells = if has_row { nonzero } else { 0 };
    let states = if count == 0 {
        0
    } else {
        cells
            .checked_mul(2)
            .and_then(|n| n.checked_add(count))
            .ok_or(AggregateErrorKind::StateLimit)?
    };
    builder.states(states)?;
    let mut conditions = builder.reserve_operands(cells)?;
    let mut weights = reserve(if count == 0 { 0 } else { cells })?;
    if has_row {
        for element in elements {
            builder.tick()?;
            if element.weight > 0 {
                conditions.push(element.condition);
                if count != 0 {
                    weights.push(
                        u64::try_from(element.weight)
                            .map_err(|_| AggregateErrorKind::ArithmeticOverflow)?,
                    );
                }
            }
        }
    }
    let mut requests = reserve(count)?;
    for &guard in guards {
        for bound in bounds(guard) {
            builder.tick()?;
            if matches!(shape.reading(bound), Reading::Threshold) {
                requests.push(bound);
            }
        }
    }
    let native = builder.threshold_family(&conditions, &weights, &requests)?;
    let mut family = Family {
        shape,
        conditions: &conditions,
        native,
        all: None,
        any: None,
        falsum,
        truth,
    };
    let mut roots = reserve(guards.len())?;
    for &guard in guards {
        builder.tick()?;
        roots.push(family.compose(builder, guard)?);
    }
    Ok(roots)
}

struct Family<'a> {
    shape: Shape,
    conditions: &'a [usize],
    native: std::ops::Range<usize>,
    all: Option<usize>,
    any: Option<usize>,
    falsum: usize,
    truth: usize,
}

impl Family<'_> {
    fn threshold(&mut self, builder: &mut Builder<'_, '_>, bound: i128) -> Result<usize> {
        builder.tick()?;
        match self.shape.reading(bound) {
            Reading::Truth => Ok(self.truth),
            Reading::Falsum => Ok(self.falsum),
            Reading::All | Reading::Any => {
                let all = matches!(self.shape.reading(bound), Reading::All);
                let slot = if all { &mut self.all } else { &mut self.any };
                if let Some(root) = *slot {
                    return Ok(root);
                }
                let root = builder.group(
                    self.conditions,
                    if all { self.truth } else { self.falsum },
                    all,
                )?;
                *slot = Some(root);
                Ok(root)
            }
            Reading::Threshold => self
                .native
                .next()
                .ok_or(AggregateErrorKind::ArithmeticOverflow),
        }
    }

    fn compose(&mut self, builder: &mut Builder<'_, '_>, guard: AggregateGuard) -> Result<usize> {
        let mut requested = bounds(guard);
        let a = self.threshold(builder, requested.next().expect("lower bound"))?;
        match guard.comparison {
            AggregateComparison::Ge | AggregateComparison::Gt => Ok(a),
            AggregateComparison::Lt | AggregateComparison::Le => {
                builder.push(NodeView::Implies(a, self.falsum))
            }
            AggregateComparison::Eq | AggregateComparison::Ne => {
                let b = self.threshold(builder, requested.next().expect("successor bound"))?;
                if guard.comparison == AggregateComparison::Eq {
                    let negative = builder.push(NodeView::Implies(b, self.falsum))?;
                    builder.push(NodeView::And(&[a, negative]))
                } else {
                    // This is an implication, not default-negated equality.
                    builder.push(NodeView::Implies(a, b))
                }
            }
        }
    }
}
