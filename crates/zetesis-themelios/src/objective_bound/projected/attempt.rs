//! Optional construction separates its outcome from performed-work evidence.

use std::fmt;

use super::{Kind, ObjectiveBoundError, ObjectiveBoundStatistics, ObjectivePlan};

/// Refusal of optional projected objective preparation, never a logical result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectiveProjectionErrorKind {
    /// The existing objective compiler's exact refusal.
    Existing(Kind),
    /// Projection-owned simultaneous capacity exceeded its inclusive allowance.
    Bytes {
        /// Complete requested or observed capacity at this boundary.
        required: u128,
        /// Unchanged projection allowance.
        limit: u128,
    },
}
impl fmt::Display for ObjectiveProjectionErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Existing(kind) => write!(f, "objective compiler {kind:?}"),
            Self::Bytes { required, limit } => write!(
                f,
                "objective projection requires {required} bytes with allowance {limit}"
            ),
        }
    }
}
impl std::error::Error for ObjectiveProjectionErrorKind {}

/// Evidence retained whether optional projected preparation succeeds or refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectiveProjectionReceipt {
    template: Option<usize>,
    statistics: ObjectiveBoundStatistics,
    projection_peak_bytes: u128,
}
impl ObjectiveProjectionReceipt {
    /// Original objective template active at refusal; `None` on success.
    #[must_use]
    pub const fn template_index(self) -> Option<usize> {
        self.template
    }
    /// Actual performed work and construction counts, including refused attempts.
    #[must_use]
    pub const fn statistics(self) -> ObjectiveBoundStatistics {
        self.statistics
    }
    /// Peak actual projection-owned capacity; refused requests are not a peak.
    #[must_use]
    pub const fn projection_peak_bytes(self) -> u128 {
        self.projection_peak_bytes
    }
}

/// An optional objective plan and its independently retained construction receipt.
///
/// A refusal is not a logical result. Callers retain exact detailed scoring under
/// their remaining allowance. Inspect [`Self::error`] for the full diagnostic,
/// or consume the small typed outcome and receipt with [`Self::into_parts`].
#[derive(Debug)]
#[must_use]
pub struct ObjectiveProjectionAttempt {
    result: Result<ObjectivePlan, ObjectiveProjectionErrorKind>,
    receipt: ObjectiveProjectionReceipt,
}
impl ObjectiveProjectionAttempt {
    pub(super) fn new(
        result: Result<ObjectivePlan, ObjectiveProjectionErrorKind>,
        template: Option<usize>,
        statistics: ObjectiveBoundStatistics,
        projection_peak_bytes: u128,
    ) -> Self {
        Self {
            result,
            receipt: ObjectiveProjectionReceipt {
                template,
                statistics,
                projection_peak_bytes,
            },
        }
    }
    pub(super) fn refused(error: ObjectiveBoundError, projection_peak_bytes: u128) -> Self {
        Self::new(
            Err(ObjectiveProjectionErrorKind::Existing(error.kind())),
            error.template_index(),
            error.statistics(),
            projection_peak_bytes,
        )
    }
    /// Performed work and observed storage, available before consuming the plan.
    #[must_use]
    pub const fn receipt(&self) -> ObjectiveProjectionReceipt {
        self.receipt
    }
    /// Full typed failure evidence, or `None` when a completed plan is available.
    #[must_use]
    pub fn error(&self) -> Option<ObjectiveProjectionError> {
        match &self.result {
            Ok(_) => None,
            Err(kind) => Some(ObjectiveProjectionError {
                kind: *kind,
                receipt: self.receipt,
            }),
        }
    }
    /// Consume the completed plan or small refusal reason alongside its receipt.
    pub fn into_parts(
        self,
    ) -> (
        Result<ObjectivePlan, ObjectiveProjectionErrorKind>,
        ObjectiveProjectionReceipt,
    ) {
        (self.result, self.receipt)
    }
}

/// Retained work and actual capacity evidence from unsuccessful preparation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveProjectionError {
    kind: ObjectiveProjectionErrorKind,
    receipt: ObjectiveProjectionReceipt,
}
impl ObjectiveProjectionError {
    /// Structured refusal; callers may retain ordinary exact scoring.
    #[must_use]
    pub const fn kind(&self) -> &ObjectiveProjectionErrorKind {
        &self.kind
    }
    /// Original objective template, when one was being visited.
    #[must_use]
    pub const fn template_index(&self) -> Option<usize> {
        self.receipt.template_index()
    }
    /// Performed work, including any successful preparation before refusal.
    #[must_use]
    pub const fn statistics(&self) -> ObjectiveBoundStatistics {
        self.receipt.statistics()
    }
    /// Peak actual projection-owned capacity observed before refusal.
    /// A requested allocation refused before execution is not an actual peak.
    #[must_use]
    pub const fn projection_peak_bytes(&self) -> u128 {
        self.receipt.projection_peak_bytes()
    }
}
impl fmt::Debug for ObjectiveProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectiveProjectionError")
            .field("kind", &self.kind)
            .field("template", &self.receipt.template_index())
            .field("statistics", &self.receipt.statistics())
            .field(
                "projection_peak_bytes",
                &self.receipt.projection_peak_bytes(),
            )
            .finish()
    }
}
impl fmt::Display for ObjectiveProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (template {:?})", self.kind, self.template_index())
    }
}
impl std::error::Error for ObjectiveProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}
