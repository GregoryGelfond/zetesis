//! Explicit implementation choice for one enabled Boolean gate's exact supports.

use crate::{GpuError, GpuErrorKind};
use std::borrow::Cow;

/// How an enabled frozen gate projects supported Boolean values.
///
/// Both implementations use the same three atomic observations and intersections,
/// including physical-slot aliases and observations taken at different instants.
/// They retain the original frozen query, sweep/work bounds and result protocol.
/// Selecting an alternative establishes no physical qualification or speedup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GateProjection {
    /// Enumerate all eight Boolean relation rows; the existing default shader.
    #[default]
    Enumerated,
    /// Intersect eight-bit relation masks before projecting position supports.
    /// This opt-in alternative still requires physical device qualification.
    Bitwise,
}

#[cfg(test)]
mod tests;

impl GateProjection {
    /// Both implementations, with the existing baseline first.
    pub const ALL: [Self; 2] = [Self::Enumerated, Self::Bitwise];

    /// Stable diagnostic identifier. Constant time and no allocation.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Enumerated => "enumerated",
            Self::Bitwise => "bitwise",
        }
    }

    // The alternative replaces only the local transfer in the immutable baseline.
    // A fixed-size, fallible reservation precedes construction. No user text or
    // repeated replacement occurs; both variants therefore share the scaffold.
    pub(super) fn shader(self) -> Result<Cow<'static, str>, GpuError> {
        const BASELINE: &str = include_str!("../formula.wgsl");
        match self {
            Self::Enumerated => Ok(Cow::Borrowed(BASELINE)),
            Self::Bitwise => {
                let invalid = || {
                    GpuError::new(
                        GpuErrorKind::Validation,
                        "formula shader gate boundary is missing",
                    )
                };
                let (prefix, gate_and_suffix) =
                    BASELINE.split_once("fn gate(").ok_or_else(invalid)?;
                let (_, suffix) = gate_and_suffix
                    .split_once("fn finish(")
                    .ok_or_else(invalid)?;
                let parts = [prefix, include_str!("bitwise.wgsl"), "fn finish(", suffix];
                let capacity = parts
                    .iter()
                    .try_fold(0usize, |total, part| total.checked_add(part.len()))
                    .ok_or_else(|| {
                        GpuError::new(
                            GpuErrorKind::Capacity,
                            "formula shader source length overflow",
                        )
                    })?;
                let mut source = String::new();
                source
                    .try_reserve_exact(capacity)
                    .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
                for part in parts {
                    source.push_str(part);
                }
                Ok(Cow::Owned(source))
            }
        }
    }
}
