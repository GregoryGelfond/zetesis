//! Per-operation construction allowances over one formula session's fixed catalog.
//! Preparation and each selection start fresh; their accepted work stays cumulative.

use std::fmt;

use zetesis_core::{
    AtomCatalog, Model, ModelError, ModelFailure, ModelOrder, ModelPublicationFailure,
};
use zetesis_cpu::{Cancellation, Stop};

use crate::{SolveConfig, SolveError};

/// Why checked model construction stopped without publishing its interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelConstructionStop {
    /// Caller cancellation or deadline, checked before the next operation.
    Control(Stop),
    /// The next operation would exceed this preparation or selection's allowance.
    Work {
        /// Proposed work in this preparation or selection, including the refused operation.
        observed: u128,
        /// Inclusive work ceiling for this preparation or selection.
        limit: u64,
    },
    /// Preparing the order or a model would exceed live metadata capacity.
    Bytes {
        /// Proposed or observed named capacity, independently of actual peak.
        required: u128,
        /// Inclusive named metadata allowance.
        limit: usize,
    },
}

impl fmt::Display for ModelConstructionStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(stop) => stop.fmt(formatter),
            Self::Work { observed, limit } => write!(
                formatter,
                "model construction work {observed} exceeds allowance {limit}"
            ),
            Self::Bytes { required, limit } => write!(
                formatter,
                "model construction requires {required} bytes, allowance is {limit}"
            ),
        }
    }
}

/// One formula session's order preparation and selected-model construction.
/// Hybrid and terminal inputs report their base models here; original-answer
/// qualification retains its separate receipt. This record is available without
/// timings. No field establishes membership, output delivery or exhaustion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelConstructionStatistics {
    /// Accepted cumulative preparation and construction operations.
    pub work: u64,
    /// Retained prepared-order metadata; zero if preparation did not complete.
    pub prepared_bytes: u128,
    /// Largest actual named metadata peak, including refused attempts.
    /// Proposed reservations and already published models are excluded.
    pub peak_bytes: u128,
    /// Complete models constructed after membership, before scoring or output.
    pub constructed: u64,
}

#[derive(Default)]
pub(crate) struct Account {
    statistics: ModelConstructionStatistics,
}

pub(crate) enum Failure {
    Interrupted(ModelConstructionStop),
    Run(SolveError),
}

impl Account {
    pub(crate) const fn statistics(&self) -> ModelConstructionStatistics {
        self.statistics
    }

    pub(crate) fn prepare<'a>(
        &mut self,
        catalog: &'a AtomCatalog,
        config: &SolveConfig,
        cancellation: &Cancellation,
    ) -> Result<ModelOrder<'a>, Failure> {
        let mut work = 0;
        let order = ModelOrder::prepare_with(catalog, config.max_model_bytes, || {
            self.permit(&mut work, config, cancellation)
        })
        .map_err(|error| self.failure(error))?;
        self.statistics.prepared_bytes = order.retained_bytes();
        self.statistics.peak_bytes = self
            .statistics
            .peak_bytes
            .max(order.preparation_peak_bytes());
        Ok(order)
    }

    pub(crate) fn select(
        &mut self,
        order: &ModelOrder<'_>,
        positions: impl IntoIterator<Item = usize>,
        config: &SolveConfig,
        cancellation: &Cancellation,
    ) -> Result<Model, Failure> {
        // Admit the completed counter before the operation can publish a model.
        let constructed = self
            .statistics
            .constructed
            .checked_add(1)
            .ok_or(Failure::Run(SolveError::ModelStatisticsOverflow))?;
        let mut work = 0;
        let publication = order
            .select_with(positions, config.max_model_bytes, || {
                self.permit(&mut work, config, cancellation)
            })
            .map_err(|error| self.failure(error))?;
        let (model, peak) = publication.into_parts();
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(peak);
        self.statistics.constructed = constructed;
        Ok(model)
    }

    fn permit(
        &mut self,
        work: &mut u64,
        config: &SolveConfig,
        cancellation: &Cancellation,
    ) -> Result<(), Failure> {
        cancellation
            .poll()
            .map_err(|stop| Failure::Interrupted(ModelConstructionStop::Control(stop)))?;
        let observed = u128::from(*work) + 1;
        if observed > u128::from(config.max_model_work) {
            return Err(Failure::Interrupted(ModelConstructionStop::Work {
                observed,
                limit: config.max_model_work,
            }));
        }
        let total = self
            .statistics
            .work
            .checked_add(1)
            .ok_or(Failure::Run(SolveError::ModelStatisticsOverflow))?;
        // Only accepted operations enter either receipt. The inclusive u64
        // allowance admits the local addition; the total has its own check.
        *work += 1;
        self.statistics.work = total;
        Ok(())
    }

    fn failure(&mut self, error: ModelPublicationFailure<Failure>) -> Failure {
        let (failure, peak) = error.into_parts();
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(peak);
        match failure {
            ModelFailure::Stopped(failure) => failure,
            ModelFailure::Model(ModelError::Bytes { required, limit }) => {
                Failure::Interrupted(ModelConstructionStop::Bytes { required, limit })
            }
            ModelFailure::Model(error) => Failure::Run(SolveError::Model(error)),
        }
    }
}

#[cfg(test)]
mod tests;
