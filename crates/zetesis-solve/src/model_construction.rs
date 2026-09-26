//! Cumulative checked construction over one formula session's fixed catalog.

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
    /// The next operation would exceed this session's cumulative allowance.
    Work {
        /// Proposed cumulative work, including the refused next operation.
        observed: u128,
        /// Inclusive session work ceiling.
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
        let order = ModelOrder::prepare_with(catalog, config.max_model_bytes, || {
            self.permit(config, cancellation)
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
        let publication = order
            .select_with(positions, config.max_model_bytes, || {
                self.permit(config, cancellation)
            })
            .map_err(|error| self.failure(error))?;
        let (model, peak) = publication.into_parts();
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(peak);
        self.statistics.constructed = constructed;
        Ok(model)
    }

    fn permit(
        &mut self,
        config: &SolveConfig,
        cancellation: &Cancellation,
    ) -> Result<(), ModelConstructionStop> {
        cancellation
            .poll()
            .map_err(ModelConstructionStop::Control)?;
        let observed = u128::from(self.statistics.work) + 1;
        if observed > u128::from(config.max_model_work) {
            return Err(ModelConstructionStop::Work {
                observed,
                limit: config.max_model_work,
            });
        }
        // The inclusive u64 allowance has admitted this addition.
        self.statistics.work += 1;
        Ok(())
    }

    fn failure(&mut self, error: ModelPublicationFailure<ModelConstructionStop>) -> Failure {
        let (failure, peak) = error.into_parts();
        self.statistics.peak_bytes = self.statistics.peak_bytes.max(peak);
        match failure {
            ModelFailure::Stopped(stop) => Failure::Interrupted(stop),
            ModelFailure::Model(ModelError::Bytes { required, limit }) => {
                Failure::Interrupted(ModelConstructionStop::Bytes { required, limit })
            }
            ModelFailure::Model(error) => Failure::Run(SolveError::Model(error)),
        }
    }
}
