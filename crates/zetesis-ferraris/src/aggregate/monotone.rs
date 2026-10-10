//! Nonnegative comparisons retain one row across numeric and boundary guards.

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

impl Reading {
    fn uses_row(self, numeric: bool) -> bool {
        match self {
            Self::Threshold => true,
            Self::All | Self::Any => numeric,
            Self::Truth | Self::Falsum => false,
        }
    }
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

    fn normalized_bound(&self, bound: i128, divisor: u64) -> Result<i128> {
        // Boundary guards previously shared one Boolean node. Keep their
        // equivalence visible in both directions after moving them onto a row.
        let bound = match self.reading(bound) {
            Reading::All => self.total,
            Reading::Any => self.minimum,
            _ => bound,
        };
        if divisor == 1 {
            return Ok(bound);
        }
        // Row requests are positive. This ceiling division avoids adding the
        // divisor to a potentially large bound before division.
        bound
            .checked_sub(1)
            .and_then(|value| value.checked_div(i128::from(divisor)))
            .and_then(|value| value.checked_add(1))
            .ok_or(AggregateErrorKind::ArithmeticOverflow)
    }
}

fn gcd(builder: &mut Builder<'_, '_>, mut left: u64, mut right: u64) -> Result<u64> {
    if left == 1 || right == 1 {
        return Ok(1);
    }
    if left == right {
        return Ok(left);
    }
    while right != 0 && left != 1 {
        builder.tick()?;
        let remainder = left
            .checked_rem(right)
            .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
        left = right;
        right = remainder;
    }
    Ok(left)
}

fn divide_weights(builder: &mut Builder<'_, '_>, weights: &mut [u64], divisor: u64) -> Result<()> {
    if divisor != 1 {
        for weight in weights {
            builder.tick()?;
            *weight = weight
                .checked_div(divisor)
                .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
        }
    }
    Ok(())
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
/// Pure Boolean families simplify in both the original theory and its reduct.
/// A numeric family keeps all nonconstant guards on its row: simplifying just
/// one to conjunction/disjunction would hide ordered-bound implications during
/// propagation, although it would preserve complete models.
/// Dividing positive weights by their common divisor preserves every original
/// and frozen condition valuation: `S = dQ` satisfies `S >= k` iff `Q >= ceil(k/d)`.
/// Boundary classification uses original units; tuple occurrences are unchanged.
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
    let mut numeric = false;
    for &guard in guards {
        for bound in bounds(guard) {
            builder.tick()?;
            match shape.reading(bound) {
                Reading::Threshold => {
                    count = count.checked_add(1).ok_or(AggregateErrorKind::StateLimit)?;
                    numeric = true;
                }
                Reading::All | Reading::Any => {
                    count = count.checked_add(1).ok_or(AggregateErrorKind::StateLimit)?;
                }
                Reading::Truth | Reading::Falsum => {}
            }
        }
    }
    let has_row = count != 0;
    let cells = if has_row { nonzero } else { 0 };
    let count = if numeric { count } else { 0 };
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
    // The minimum is itself a positive coefficient. Seeding with it also skips
    // every Euclidean step for rows containing a unit weight.
    let mut divisor = if numeric {
        u64::try_from(shape.minimum).map_err(|_| AggregateErrorKind::ArithmeticOverflow)?
    } else {
        1
    };
    if has_row {
        for element in elements {
            builder.tick()?;
            if element.weight > 0 {
                conditions.push(element.condition);
                if count != 0 {
                    let weight = u64::try_from(element.weight)
                        .map_err(|_| AggregateErrorKind::ArithmeticOverflow)?;
                    divisor = gcd(builder, weight, divisor)?;
                    weights.push(weight);
                }
            }
        }
    }
    divide_weights(builder, &mut weights, divisor)?;
    let mut requests = reserve(count)?;
    for &guard in guards {
        for bound in bounds(guard) {
            builder.tick()?;
            if shape.reading(bound).uses_row(numeric) {
                requests.push(shape.normalized_bound(bound, divisor)?);
            }
        }
    }
    let native = builder.threshold_family(&conditions, &weights, &requests)?;
    let mut family = Family {
        shape,
        numeric,
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
    numeric: bool,
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
            Reading::All | Reading::Any if !self.numeric => {
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
            Reading::Threshold | Reading::All | Reading::Any => self
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
