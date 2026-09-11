//! Diagnostic-only aggregation cannot discard joined semantic results.

/// Cumulative entered completion attempts, including failed and retried slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompletionAccounting {
    /// Requested upper bound on simultaneous exact query workspaces.
    pub requested_workers: usize,
    /// Largest admitted query concurrency so far; zero before residual entry.
    pub effective_workers: usize,
    /// Peak admitted logical scratch bytes across attempts, never RSS.
    pub peak_scratch_bytes: u64,
    /// Candidate slots actually entered, including certificates and retries.
    pub entered: u64,
    /// Entered slots requesting exact residual completion.
    pub residuals: u64,
    /// Entered slots whose local check completed, even if the batch did not commit.
    pub completed: u64,
    /// Entered slots whose local check failed.
    pub failed: u64,
    /// Residual slots that completed locally, including uncommitted successes.
    pub residual_completed: u64,
    /// Residual slots whose local exact check failed.
    pub residual_failed: u64,
    /// Coordinator batch wall intervals, present only with phase timing enabled.
    pub wall: Option<zetesis_sat::PhaseMeasurement>,
    /// Summed worker original-validation intervals; workers may overlap.
    pub worker_original: Option<zetesis_sat::PhaseMeasurement>,
    /// Summed worker reduct intervals; workers may overlap.
    pub worker_reduct: Option<zetesis_sat::PhaseMeasurement>,
    /// A diagnostic counter saturated; never changes committed semantic results.
    pub overflowed: bool,
}

impl CompletionAccounting {
    pub(crate) fn record(&mut self, progress: zetesis_sat::CompletionStatistics) {
        if let Some(elapsed) = progress.elapsed {
            self.wall.get_or_insert_default().record(elapsed);
        }
        merge(
            &mut self.worker_original,
            progress.worker_original_validation,
        );
        merge(&mut self.worker_reduct, progress.worker_reduct);
        self.requested_workers = progress.workers;
        self.effective_workers = self.effective_workers.max(progress.effective_workers);
        self.peak_scratch_bytes = self.peak_scratch_bytes.max(progress.peak_scratch_bytes);
        for (target, value) in [
            (&mut self.entered, progress.candidates),
            (&mut self.residuals, progress.residuals),
            (&mut self.completed, progress.completed),
            (&mut self.failed, progress.failed),
            (&mut self.residual_completed, progress.residual_completed),
            (&mut self.residual_failed, progress.residual_failed),
        ] {
            if let Ok(value) = u64::try_from(value)
                && let Some(total) = target.checked_add(value)
            {
                *target = total;
            } else {
                *target = u64::MAX;
                self.overflowed = true;
            }
        }
    }
}

fn merge(
    target: &mut Option<zetesis_sat::PhaseMeasurement>,
    source: Option<zetesis_sat::PhaseMeasurement>,
) {
    if let Some(source) = source {
        let target = target.get_or_insert_default();
        match (
            target.calls.checked_add(source.calls),
            target.elapsed.checked_add(source.elapsed),
        ) {
            (Some(calls), Some(elapsed)) if !target.overflowed && !source.overflowed => {
                target.calls = calls;
                target.elapsed = elapsed;
            }
            _ => target.overflowed = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::CompletionAccounting;
    use zetesis_sat::{CompletionStatistics, PhaseMeasurement};

    #[test]
    fn diagnostic_saturation_does_not_discard_completed_accounting_or_wrap() {
        let mut accounting = CompletionAccounting {
            entered: u64::MAX,
            ..Default::default()
        };
        let progress = CompletionStatistics {
            workers: 4,
            effective_workers: 2,
            peak_scratch_bytes: 900,
            candidates: 3,
            residuals: 2,
            completed: 2,
            failed: 1,
            residual_completed: 1,
            residual_failed: 1,
            elapsed: Some(Duration::from_nanos(10)),
            worker_original_validation: Some(PhaseMeasurement {
                calls: 2,
                elapsed: Duration::from_nanos(6),
                overflowed: false,
            }),
            worker_reduct: Some(PhaseMeasurement {
                calls: 2,
                elapsed: Duration::from_nanos(11),
                overflowed: false,
            }),
        };
        accounting.record(progress);
        assert_eq!(accounting.entered, u64::MAX);
        assert!(accounting.overflowed);
        assert_eq!((accounting.completed, accounting.failed), (2, 1));
        assert_eq!(
            (accounting.residual_completed, accounting.residual_failed),
            (1, 1)
        );
        assert_eq!(accounting.wall.unwrap().elapsed, Duration::from_nanos(10));
        assert_eq!(
            accounting.worker_reduct.unwrap().elapsed,
            Duration::from_nanos(11)
        );
        accounting.worker_reduct.as_mut().unwrap().calls = u64::MAX;
        accounting.record(progress);
        assert!(accounting.worker_reduct.unwrap().overflowed);
        assert_eq!(accounting.wall.unwrap().calls, 2);
        assert_eq!(accounting.worker_original.unwrap().calls, 4);
        accounting.record(CompletionStatistics {
            worker_original_validation: Some(PhaseMeasurement {
                overflowed: true,
                ..Default::default()
            }),
            ..Default::default()
        });
        assert!(accounting.worker_original.unwrap().overflowed);
        assert_eq!(accounting.peak_scratch_bytes, 900);
    }
}
