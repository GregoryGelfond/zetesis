//! Checked physical Boolean expansion of nonnegative thresholds.
//!
//! This transport operation does not construct a second semantic theory. The
//! caller retains the semantic node map and owns the two DP rows. For each
//! nonzero contribution and target `t`, the recurrence is
//! `next[t] = previous[t] OR (condition AND previous[t - weight])`, with
//! saturating subtraction. Bounds requiring every positive contribution or any
//! positive contribution instead use a linear connective fold with no DP rows.
//! Repeated conditions retain independent coefficients.

use crate::Threshold;

/// A malformed numerical row or an unrepresentable physical expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Columns, ranks or bounds are invalid, the total overflows, or scratch is too small.
    Invalid,
    /// Physical gates or scratch cannot be addressed by the host.
    Capacity,
}

/// Exact unsimplified gate count and caller-owned temporary value slots.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// Binary AND/OR gate attempts, before backend-specific simplification.
    pub gates: usize,
    /// Value slots required by the two recurrence rows.
    pub scratch: usize,
    expansion: Expansion,
}

#[derive(Clone, Copy, Debug)]
enum Expansion {
    Constant(bool),
    All,
    Any,
    Dynamic { bound: usize },
}

impl Shape {
    /// Inspect every contribution before any constant simplification.
    ///
    /// `visit` charges and polls once per input occurrence. No allocation occurs.
    ///
    /// # Errors
    /// Returns the caller's interruption or a checked numerical/physical refusal.
    pub fn new<E: From<Error>>(
        threshold: Threshold<'_>,
        mut visit: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        if threshold.conditions.len() != threshold.weights.len() {
            return Err(Error::Invalid.into());
        }
        let mut total = 0_i128;
        let mut nonzero = 0_usize;
        let mut minimum = i128::MAX;
        for &weight in threshold.weights {
            visit()?;
            total = total
                .checked_add(i128::from(weight))
                .ok_or(Error::Invalid)?;
            if weight != 0 {
                nonzero += 1;
                minimum = minimum.min(i128::from(weight));
            }
        }
        let constant = if threshold.bound <= 0 {
            Some(true)
        } else if threshold.bound > total {
            Some(false)
        } else {
            None
        };
        if let Some(value) = constant {
            return Ok(Self {
                gates: 0,
                scratch: 0,
                expansion: Expansion::Constant(value),
            });
        }
        // A nonconstant bound implies a positive total and at least one positive
        // contribution. Dropping any such position loses at least minimum;
        // selecting any one gains at least minimum. Equal conditions and zero
        // positions do not change either implication.
        let boundary = if threshold.bound > total - minimum {
            Some(Expansion::All)
        } else if threshold.bound <= minimum {
            Some(Expansion::Any)
        } else {
            None
        };
        if let Some(expansion) = boundary {
            return Ok(Self {
                // Include the identity gate so even a single condition returns
                // a newly appended last node in sinks that retain every gate.
                gates: nonzero,
                scratch: 0,
                expansion,
            });
        }
        let bound = usize::try_from(threshold.bound).map_err(|_| Error::Capacity)?;
        let scratch = bound
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .ok_or(Error::Capacity)?;
        let gates = bound
            .checked_mul(nonzero)
            .and_then(|n| n.checked_mul(2))
            .ok_or(Error::Capacity)?;
        Ok(Self {
            gates,
            scratch,
            expansion: Expansion::Dynamic { bound },
        })
    }

    /// Whether the validated threshold is independent of all conditions.
    #[must_use]
    pub const fn constant(self) -> Option<bool> {
        match self.expansion {
            Expansion::Constant(value) => Some(value),
            Expansion::All | Expansion::Any | Expansion::Dynamic { .. } => None,
        }
    }
}

/// Backend Boolean operations shared by threshold and ordinal-cut lowering.
///
/// Implementations preserve existing semantic atom identities. `gate` may
/// simplify classical expressions; it must never replace the semantic owner.
pub trait Sink {
    /// Copyable physical output handle or constant.
    type Value: Copy;
    /// Caller-owned typed resource, control and shape failures.
    type Error: From<Error>;
    /// Charge one recurrence or input visit and poll cancellation.
    ///
    /// # Errors
    /// Returns a work or control refusal.
    fn tick(&mut self) -> Result<(), Self::Error>;
    /// Materialize a Boolean constant.
    ///
    /// # Errors
    /// Returns the backend's physical resource refusal.
    fn constant(&mut self, value: bool) -> Result<Self::Value, Self::Error>;
    /// Read the already translated semantic condition.
    ///
    /// # Errors
    /// Returns a malformed map or physical resource refusal.
    fn condition(&mut self, index: usize) -> Result<Self::Value, Self::Error>;
    /// Compose an AND (`false`) or OR (`true`) gate.
    ///
    /// # Errors
    /// Returns the backend's physical resource refusal.
    fn gate(
        &mut self,
        disjunction: bool,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Self::Error>;
}

/// Expand one threshold into physical Boolean operations using checked scratch.
///
/// Conditions must belong to an admitted predecessor map, including zero-weight
/// positions: this numeric operation does not authenticate formula ownership.
/// Boundary bounds use zero scratch and one AND/OR gate per positive occurrence;
/// interior bounds use two O(bound) rows. Both attempt exactly `Shape::gates`
/// gates. Each condition occurrence, initialization slot and DP cell consumes a
/// `tick`, as does the checked shape scan. Every nonconstant expansion requests
/// true and false constants before emitting gates, retaining the physical layout
/// and last-node root contract. The caller retains the original-node truth gate
/// when lowering a frozen query. All arithmetic is
/// widened before any host-address conversion. No retained semantic nodes or
/// allocations are created by this operation.
///
/// # Errors
/// Returns input/physical shape, backend resource or interruption failures.
pub fn lower<S: Sink>(
    threshold: Threshold<'_>,
    scratch: &mut [S::Value],
    sink: &mut S,
) -> Result<S::Value, S::Error> {
    sink.tick()?;
    let shape = Shape::new(threshold, || sink.tick())?;
    if let Some(value) = shape.constant() {
        return sink.constant(value);
    }
    if scratch.len() < shape.scratch {
        return Err(Error::Invalid.into());
    }
    let truth = sink.constant(true)?;
    let falsum = sink.constant(false)?;
    let bound = match shape.expansion {
        Expansion::All => return boundary(threshold, false, truth, sink),
        Expansion::Any => return boundary(threshold, true, falsum, sink),
        Expansion::Dynamic { bound } => bound,
        Expansion::Constant(_) => unreachable!("constant expansion returned above"),
    };
    let width = bound + 1;
    let (mut previous, mut current) = scratch[..shape.scratch].split_at_mut(width);
    for target in 0..width {
        sink.tick()?;
        previous[target] = if target == 0 { truth } else { falsum };
        current[target] = falsum;
    }
    for (&condition, &weight) in threshold.conditions.iter().zip(threshold.weights) {
        sink.tick()?;
        if weight == 0 {
            continue;
        }
        let condition = sink.condition(condition)?;
        // Only min(weight, bound) participates in a host index; a u64 weight
        // never narrows through i32 or truncates on a narrower host.
        let weight =
            usize::try_from(u128::from(weight).min(bound as u128)).map_err(|_| Error::Capacity)?;
        current[0] = truth;
        for target in 1..width {
            sink.tick()?;
            let included = sink.gate(false, condition, previous[target.saturating_sub(weight)])?;
            current[target] = sink.gate(true, previous[target], included)?;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(previous[bound])
}

/// Every row position still receives its scan tick, including zero weights.
/// Conditions keep their original order and repetitions; neither a decided
/// result nor an identity gate bypasses later reads or their typed failures.
fn boundary<S: Sink>(
    threshold: Threshold<'_>,
    disjunction: bool,
    mut value: S::Value,
    sink: &mut S,
) -> Result<S::Value, S::Error> {
    for (&condition, &weight) in threshold.conditions.iter().zip(threshold.weights) {
        sink.tick()?;
        if weight != 0 {
            let condition = sink.condition(condition)?;
            value = sink.gate(disjunction, value, condition)?;
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
