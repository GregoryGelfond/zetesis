//! Exit precedence is restricted to resource refusals, preserving source identity.

mod resources;

use super::Accounting;
use crate::{
    ConstraintCheckCause, ConstraintCheckFailure, ConstraintCheckStatistics, FormulaFailure,
    ProgramSite, ReconstructionError,
};
use zetesis_core::{AtomIndexError, ModelFailure};
use zetesis_cpu::Stop;

/// Unlike the CPU's unlocated Refusal, replacement can inspect the old error.
pub(crate) trait StopPrecedence: Sized {
    fn is_resource_refusal(&self) -> bool;
    fn outranked_by(self, stop: Stop, fallback: ProgramSite) -> Self;
    fn from_stop(stop: Stop, site: ProgramSite, receipt: &Accounting) -> Self;
}

impl StopPrecedence for FormulaFailure {
    fn is_resource_refusal(&self) -> bool {
        resources::formula(self)
    }

    fn outranked_by(self, reason: Stop, fallback: ProgramSite) -> Self {
        match self {
            Self::Program { program, error } => Self::Program {
                program,
                error: Box::new(error.outranked_by(reason, fallback)),
            },
            // Include's public site is intentionally absent; its resource cause
            // can still have a precise expansion site at this internal boundary.
            Self::Include(error) => {
                let location = match error.as_ref() {
                    crate::BundleAdmissionError::Expansion(error) => error.site(),
                    crate::BundleAdmissionError::RootAlias { .. }
                    | crate::BundleAdmissionError::IncludeAlias { .. }
                    | crate::BundleAdmissionError::IncludeRedirection { .. } => None,
                }
                .unwrap_or(fallback);
                Self::Interrupted { reason, location }
            }
            error => Self::Interrupted {
                reason,
                location: error.site().unwrap_or(fallback),
            },
        }
    }

    fn from_stop(reason: Stop, location: ProgramSite, _: &Accounting) -> Self {
        Self::Interrupted { reason, location }
    }
}

impl StopPrecedence for ReconstructionError {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Source(error) | Self::Model(ModelFailure::Stopped(error)) => {
                error.is_resource_refusal()
            }
            Self::Model(ModelFailure::Model(error)) => error.is_resource_refusal(),
            Self::ForeignInput | Self::Failed | Self::Unification(_) | Self::Head(_) => false,
        }
    }

    fn outranked_by(self, stop: Stop, fallback: ProgramSite) -> Self {
        match self {
            Self::Source(error) => Self::Source(Box::new(error.outranked_by(stop, fallback))),
            Self::Model(ModelFailure::Stopped(error)) => Self::Model(ModelFailure::Stopped(
                Box::new(error.outranked_by(stop, fallback)),
            )),
            Self::Model(ModelFailure::Model(_)) => Self::Model(ModelFailure::Stopped(Box::new(
                FormulaFailure::Interrupted {
                    reason: stop,
                    location: fallback,
                },
            ))),
            error @ (Self::ForeignInput | Self::Failed | Self::Unification(_) | Self::Head(_)) => {
                error
            }
        }
    }

    fn from_stop(reason: Stop, location: ProgramSite, _: &Accounting) -> Self {
        Self::Source(Box::new(FormulaFailure::Interrupted { reason, location }))
    }
}

impl StopPrecedence for ConstraintCheckCause {
    fn is_resource_refusal(&self) -> bool {
        match self {
            Self::Source(error) | Self::Index(AtomIndexError::Stopped(error)) => {
                error.is_resource_refusal()
            }
            Self::Index(AtomIndexError::Allocation) => true,
            Self::WrongProgram
            | Self::NoActiveRegion
            | Self::WrongRegionSize { .. }
            | Self::Index(AtomIndexError::Duplicate { .. })
            | Self::Stopped(_) => false,
        }
    }

    fn outranked_by(self, stop: Stop, fallback: ProgramSite) -> Self {
        match self {
            Self::Source(error) => Self::Source(Box::new(error.outranked_by(stop, fallback))),
            Self::Index(AtomIndexError::Stopped(error)) => Self::Index(AtomIndexError::Stopped(
                Box::new(error.outranked_by(stop, fallback)),
            )),
            Self::Index(AtomIndexError::Allocation) => Self::Stopped(stop),
            error @ (Self::WrongProgram
            | Self::NoActiveRegion
            | Self::WrongRegionSize { .. }
            | Self::Index(AtomIndexError::Duplicate { .. })
            | Self::Stopped(_)) => error,
        }
    }

    fn from_stop(stop: Stop, _: ProgramSite, _: &Accounting) -> Self {
        Self::Stopped(stop)
    }
}

impl StopPrecedence for ConstraintCheckFailure {
    fn is_resource_refusal(&self) -> bool {
        self.cause.is_resource_refusal()
    }

    fn outranked_by(self, stop: Stop, fallback: ProgramSite) -> Self {
        Self {
            cause: self.cause.outranked_by(stop, fallback),
            statistics: self.statistics,
        }
    }

    fn from_stop(stop: Stop, _: ProgramSite, receipt: &Accounting) -> Self {
        Self {
            cause: ConstraintCheckCause::Stopped(stop),
            statistics: ConstraintCheckStatistics {
                work: receipt.work,
                substitutions: receipt.substitutions,
                scalar_bytes: 0,
            },
        }
    }
}
