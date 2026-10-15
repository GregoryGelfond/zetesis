//! Proposal growth follows completed work and the two existing capacity owners.

use std::num::NonZeroUsize;

use zetesis_sat::StableModels;

use crate::{GpuBatching, SolveConfig};

#[derive(Default)]
pub(super) enum Batching {
    #[default]
    Fixed,
    Adaptive {
        target: NonZeroUsize,
        prefix_limit: NonZeroUsize,
    },
}

impl Batching {
    pub(super) fn new(options: &SolveConfig) -> Self {
        match options.gpu_batching {
            GpuBatching::Fixed => Self::Fixed,
            GpuBatching::Adaptive => Self::Adaptive {
                target: options.batch_size,
                prefix_limit: NonZeroUsize::MAX,
            },
        }
    }

    pub(super) fn target(&self, models: &StableModels, options: &SolveConfig) -> NonZeroUsize {
        match self {
            Self::Fixed => options.batch_size,
            Self::Adaptive { target, .. } => {
                // Retried proposals keep their original count. The normal batch
                // admission still checks changed limits and retained capacity.
                if models.batch_statistics().pending != 0 {
                    return *target;
                }
                let memory =
                    NonZeroUsize::new(models.joint_batch_capacity(options.max_batch_bytes))
                        .unwrap_or(NonZeroUsize::MIN);
                // If even one cannot fit, ordinary admission reports its typed
                // refusal; a sizing observation cannot claim exhaustion.
                (*target).min(memory)
            }
        }
    }

    /// Called only after complete membership/commit, never after a partial
    /// device prefix or a failed completion. Final short batches do not grow.
    pub(super) fn committed(
        &mut self,
        requested: NonZeroUsize,
        checked: usize,
        observed_prefix: Option<NonZeroUsize>,
    ) {
        let Self::Adaptive {
            target,
            prefix_limit,
        } = self
        else {
            return;
        };
        if let Some(observed) = observed_prefix {
            *prefix_limit = (*prefix_limit).min(observed);
        }
        *target = if checked == requested.get() {
            requested
                .checked_mul(NonZeroUsize::new(2).unwrap())
                .unwrap_or(NonZeroUsize::MAX)
        } else {
            *target
        }
        .min(*prefix_limit);
    }
}

#[cfg(test)]
mod tests;
