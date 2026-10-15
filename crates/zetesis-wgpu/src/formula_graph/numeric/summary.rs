//! Cold potential measures classify guards without rescanning shared rows.

use crate::GpuError;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{ExtremumCutRule, NodeView};

pub(super) enum Summary {
    Weights(i128),
    Ordinal {
        minimum: u64,
        maximum: u64,
        sentinel: u64,
    },
}
impl Summary {
    pub(super) fn new(node: NodeView<'_>, cancellation: &Cancellation) -> Result<Self, GpuError> {
        match node {
            NodeView::Threshold(threshold) => {
                let mut total = 0i128;
                for &weight in threshold.weights {
                    super::poll(cancellation)?;
                    // Theory admission establishes a nonnegative i128 total.
                    total += i128::from(weight);
                }
                Ok(Self::Weights(total))
            }
            NodeView::ExtremumCut(cut) => {
                let mut minimum = cut.row.rank_count - 1;
                let mut maximum = 0;
                for &rank in cut.row.ranks {
                    super::poll(cancellation)?;
                    minimum = minimum.min(rank);
                    maximum = maximum.max(rank);
                }
                Ok(Self::Ordinal {
                    minimum,
                    maximum,
                    sentinel: cut.row.rank_count - 1,
                })
            }
            _ => unreachable!("only numeric rows have summaries"),
        }
    }
    fn constant(&self, node: NodeView<'_>) -> Option<bool> {
        match (self, node) {
            (Self::Weights(total), NodeView::Threshold(threshold)) => {
                (threshold.bound > *total).then_some(false)
            }
            (
                Self::Ordinal {
                    minimum, maximum, ..
                },
                NodeView::ExtremumCut(cut),
            ) => {
                let impossible = match cut.rule {
                    ExtremumCutRule::Prefix(bound) => minimum >= bound,
                    ExtremumCutRule::Suffix(bound) => maximum < bound,
                };
                impossible.then_some(false)
            }
            _ => unreachable!("an admitted row retains its numeric domain"),
        }
    }

    /// Every measure and live bound fits this many u32 limbs. Threshold bounds
    /// outside (0, total] were folded to constants; ordinal endpoints were too.
    /// The ordinal empty-minimum sentinel must fit even when every rank is small.
    pub(super) fn width(&self) -> u32 {
        let largest = match self {
            Self::Weights(total) => u128::try_from(*total).expect("nonnegative admitted total"),
            Self::Ordinal { sentinel, .. } => u128::from(*sentinel),
        };
        if largest <= u128::from(u32::MAX) {
            1
        } else if largest <= u128::from(u64::MAX) {
            2
        } else {
            4
        }
    }

    /// Contributions are u64; a complete 32-bit row needs no high value column.
    /// Wider sums still read only the two limbs of each admitted contribution.
    pub(super) fn value_words(&self) -> u32 {
        self.width().min(2)
    }
}

pub(super) fn endpoint(node: NodeView<'_>) -> Option<bool> {
    match node {
        NodeView::Threshold(threshold) if threshold.bound <= 0 => Some(true),
        NodeView::ExtremumCut(cut) => cut.endpoint_truth(),
        _ => None,
    }
}

/// The single physical constant rule, shared by prepared summaries and the
/// allocation-free constant-only compatibility scan. Absence of a possible
/// witness proves false; a possible witness never establishes true.
pub(super) fn constant(
    node: NodeView<'_>,
    summary: Option<&Summary>,
    cancellation: &Cancellation,
) -> Result<Option<bool>, GpuError> {
    if let Some(value) = endpoint(node) {
        return Ok(Some(value));
    }
    let computed;
    let summary = if let Some(summary) = summary {
        summary
    } else {
        computed = Summary::new(node, cancellation)?;
        &computed
    };
    Ok(summary.constant(node))
}

#[cfg(test)]
mod tests;
