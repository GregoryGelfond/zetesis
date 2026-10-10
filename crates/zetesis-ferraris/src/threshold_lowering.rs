//! Checked physical Boolean expansion of nonnegative thresholds.
//!
//! This transport operation does not construct a second semantic theory. The
//! caller retains the semantic node map and owns the two DP rows. For each
//! nonzero contribution and target `t`, the recurrence is
//! `next[t] = previous[t] OR (condition AND previous[t - weight])`, with
//! saturating subtraction. Repeated conditions retain independent coefficients.

use crate::Threshold;

/// A malformed numerical row or an unrepresentable physical expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Parallel columns differ, the total overflows, or scratch is too small.
    Invalid,
    /// Physical gates or scratch cannot be addressed by the host.
    Capacity,
}

/// Exact unsimplified DP gate count and caller-owned temporary value slots.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// Binary AND/OR gate attempts, before backend-specific simplification.
    pub gates: usize,
    /// Value slots required by the two recurrence rows.
    pub scratch: usize,
    constant: Option<bool>,
    bound: usize,
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
        for &weight in threshold.weights {
            visit()?;
            total = total
                .checked_add(i128::from(weight))
                .ok_or(Error::Invalid)?;
            nonzero += usize::from(weight != 0);
        }
        let constant = if threshold.bound <= 0 {
            Some(true)
        } else if threshold.bound > total {
            Some(false)
        } else {
            None
        };
        if constant.is_some() {
            return Ok(Self {
                gates: 0,
                scratch: 0,
                constant,
                bound: 0,
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
            constant,
            bound,
        })
    }

    /// Whether the validated threshold is independent of all conditions.
    #[must_use]
    pub const fn constant(self) -> Option<bool> {
        self.constant
    }
}

/// Backend operations used by the common recurrence.
///
/// Implementations preserve existing semantic atom identities. `gate` may
/// simplify classical expressions; it must never replace the semantic owner.
pub trait Sink {
    /// Copyable physical output handle or constant.
    type Value: Copy;
    /// Caller-owned typed resource, control and shape failures.
    type Error: From<Error>;
    /// Charge one recurrence/input visit and poll cancellation.
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
/// The two rows use O(bound) slots and the recurrence attempts exactly
/// `Shape::gates` gates. Each condition occurrence, initialization slot and DP
/// cell consumes a `tick`, as does the checked shape scan. All arithmetic is
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
    if let Some(value) = shape.constant {
        return sink.constant(value);
    }
    if scratch.len() < shape.scratch {
        return Err(Error::Invalid.into());
    }
    let truth = sink.constant(true)?;
    let falsum = sink.constant(false)?;
    let width = shape.bound + 1;
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
        let weight = usize::try_from(u128::from(weight).min(shape.bound as u128))
            .map_err(|_| Error::Capacity)?;
        current[0] = truth;
        for target in 1..width {
            sink.tick()?;
            let included = sink.gate(false, condition, previous[target.saturating_sub(weight)])?;
            current[target] = sink.gate(true, previous[target], included)?;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(previous[shape.bound])
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Boolean {
        mask: usize,
        gates: usize,
        visits: usize,
        limit: usize,
    }

    impl Sink for Boolean {
        type Value = bool;
        type Error = Error;
        fn tick(&mut self) -> Result<(), Error> {
            if self.visits == self.limit {
                return Err(Error::Capacity);
            }
            self.visits += 1;
            Ok(())
        }
        fn constant(&mut self, value: bool) -> Result<bool, Error> {
            Ok(value)
        }
        fn condition(&mut self, index: usize) -> Result<bool, Error> {
            Ok(self.mask & (1 << index) != 0)
        }
        fn gate(&mut self, disjunction: bool, left: bool, right: bool) -> Result<bool, Error> {
            self.gates += 1;
            Ok(if disjunction {
                left || right
            } else {
                left && right
            })
        }
    }

    #[test]
    fn physical_recurrence_matches_independent_weighted_sums() {
        for weights in [[0, 0, 0, 0], [1, 2, 0, 3], [u64::MAX, 1, 2, 0]] {
            for bound in -1..=8 {
                let threshold = Threshold {
                    conditions: &[0, 0, 1, 2],
                    weights: &weights,
                    bound,
                };
                let shape = Shape::new(threshold, || Ok::<_, Error>(())).unwrap();
                for mask in 0..8 {
                    let mut sink = Boolean {
                        mask,
                        gates: 0,
                        visits: 0,
                        limit: usize::MAX,
                    };
                    let actual =
                        lower(threshold, &mut vec![false; shape.scratch], &mut sink).unwrap();
                    let total: i128 = threshold
                        .conditions
                        .iter()
                        .zip(weights)
                        .filter(|(condition, _)| mask & (1 << **condition) != 0)
                        .map(|(_, weight)| i128::from(weight))
                        .sum();
                    assert_eq!(actual, total >= bound);
                    assert_eq!(sink.gates, shape.gates);
                }
            }
        }
    }

    #[test]
    fn empty_thresholds_expand_only_their_constant() {
        for bound in [i128::MIN, -1, 0, 1, i128::MAX] {
            let threshold = Threshold {
                conditions: &[],
                weights: &[],
                bound,
            };
            let shape = Shape::new(threshold, || Ok::<_, Error>(())).unwrap();
            assert_eq!((shape.gates, shape.scratch), (0, 0));
            let mut sink = Boolean {
                mask: 0,
                gates: 0,
                visits: 0,
                limit: usize::MAX,
            };
            assert_eq!(lower(threshold, &mut [], &mut sink).unwrap(), 0 >= bound);
        }
    }

    #[test]
    fn physical_shape_validates_columns_before_constant_simplification() {
        let threshold = Threshold {
            conditions: &[0],
            weights: &[],
            bound: 0,
        };
        assert!(matches!(
            Shape::new(threshold, || Ok::<_, Error>(())),
            Err(Error::Invalid)
        ));
    }

    #[test]
    fn physical_shape_refuses_unaddressable_expansion() {
        let threshold = Threshold {
            conditions: &[0],
            weights: &[u64::MAX],
            bound: i128::from(u64::MAX),
        };
        assert!(matches!(
            Shape::new(threshold, || Ok::<_, Error>(())),
            Err(Error::Capacity)
        ));
    }

    #[test]
    fn physical_lowering_preserves_control_failure() {
        let threshold = Threshold {
            conditions: &[0, 1],
            weights: &[1, 1],
            bound: 1,
        };
        let mut sink = Boolean {
            mask: 3,
            gates: 0,
            visits: 0,
            limit: 3,
        };
        assert_eq!(
            lower(threshold, &mut [false; 4], &mut sink),
            Err(Error::Capacity)
        );
    }
}
