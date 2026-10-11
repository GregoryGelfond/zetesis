//! Checked physical OR expansion of an ordinal extremum cut.
//!
//! The admitted numeric row stays authoritative. Each physical output is a fold
//! of qualifying predecessor outputs, or an endpoint constant. This operation
//! allocates no scratch and constructs no additional semantic theory. Different
//! cuts of a shared row are lowered independently: worst-case O(E * G) transport
//! work and gate attempts for E row occurrences and G cuts. Physical row sharing
//! is deliberately not claimed by this first lowering boundary.

pub use crate::threshold_lowering::{Error, Sink};
use crate::{ExtremumCut, ExtremumCutRule};

/// Exact unsimplified OR-gate attempts for one validated cut.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// One OR per qualifying occurrence, including the initial identity gate.
    pub gates: usize,
    constant: Option<bool>,
}

impl Shape {
    /// Validate every rank and count qualifying witnesses without allocation.
    ///
    /// `visit` charges and polls once per input occurrence, even for endpoint
    /// constants. Child IDs remain the admitted predecessor map's responsibility.
    /// Each native cut retains its own physical final node in an unsimplified sink.
    ///
    /// # Errors
    /// Propagates interruption or refuses malformed columns, carrier, ranks or cut.
    pub fn new<E: From<Error>>(
        cut: ExtremumCut<'_>,
        mut visit: impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        if cut.row.conditions.len() != cut.row.ranks.len() || cut.row.rank_count < 2 {
            return Err(Error::Invalid.into());
        }
        let (ExtremumCutRule::Prefix(boundary) | ExtremumCutRule::Suffix(boundary)) = *cut.rule;
        if boundary > cut.row.rank_count {
            return Err(Error::Invalid.into());
        }
        let mut gates = 0usize;
        for &rank in cut.row.ranks {
            visit()?;
            if rank >= cut.row.rank_count {
                return Err(Error::Invalid.into());
            }
            if cut.qualifies(rank) {
                gates = gates.checked_add(1).ok_or(Error::Capacity)?;
            }
        }
        let constant = cut.endpoint_truth().or((gates == 0).then_some(false));
        Ok(Self {
            gates: if constant.is_some() { 0 } else { gates },
            constant,
        })
    }

    /// Condition-independent truth after validating the complete row.
    #[must_use]
    pub const fn constant(self) -> Option<bool> {
        self.constant
    }
}

/// Lower a cut through the existing physical Boolean sink protocol.
///
/// Charges one entry tick and a complete checked shape scan. A nonconstant cut
/// then charges one scan tick per occurrence and emits one OR attempt per
/// qualifying occurrence, retaining order and repetitions. It begins with false;
/// a constant cut requests only its constant. Backend gate operations charge
/// their own work. Thus every unsimplified output has a newly appended last node.
///
/// The caller keeps semantic-to-physical addresses and authenticates original
/// truth. A frozen SAT query must retain the cut's original-truth gate; a physical
/// reduct evaluator must gate each emitted node by its own original truth.
/// No row/world cache, allocation or second semantic owner is introduced.
///
/// # Errors
/// Preserves shape, map, resource, gate and cancellation refusals. The caller owns
/// any partly emitted transport and publishes it only after complete success.
pub fn lower<S: Sink>(cut: ExtremumCut<'_>, sink: &mut S) -> Result<S::Value, S::Error> {
    sink.tick()?;
    let shape = Shape::new(cut, || sink.tick())?;
    if let Some(value) = shape.constant() {
        return sink.constant(value);
    }
    let mut value = sink.constant(false)?;
    for (&condition, &rank) in cut.row.conditions.iter().zip(cut.row.ranks) {
        sink.tick()?;
        if cut.qualifies(rank) {
            let condition = sink.condition(condition)?;
            value = sink.gate(true, value, condition)?;
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
