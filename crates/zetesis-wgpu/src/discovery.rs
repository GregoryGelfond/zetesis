//! Adapter enumeration begun before the GPU primitive that will use it.
//!
//! A discovery performs, on one named thread, the instance creation and adapter
//! enumeration an independent constructor would perform, and creates no device.
//! A primitive constructed from it reruns its own selection over that
//! enumeration, with its own policy and capability check, so it chooses the
//! adapter and reports the error it would have without the discovery, at the
//! same step; it then requests its device from the chosen adapter, whose handle
//! nothing else has used.
//!
//! Ownership: the thread holds only its result, which it returns through its
//! `JoinHandle`. The join handle lives in the state the consumers' handles
//! share and the thread never references, so the last handle dropped is always
//! a consumer's. Taking the result replaces that state, so it is taken once.
//! Dropping the last handle while the thread runs joins it: no discovery work
//! outlives its owner, at the cost of waiting for the rest of the enumeration,
//! which wgpu does not let anyone cancel.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use crate::selection::{self, Candidate};
use crate::{GpuContext, GpuError, GpuOptions, GpuSelection};

/// Adapter enumeration begun before the GPU primitive that will use it; it
/// creates no device. A primitive takes it through
/// [`crate::GpuDeviceSource::Discovered`].
///
/// Cloning shares the discovery. It serves one primitive: the first primitive
/// constructed from it takes its enumeration, and any later one enumerates as
/// an independent constructor does. Dropping the last handle waits for the
/// discovery thread, so no discovery work outlives its owner; that wait is
/// bounded by wgpu's adapter enumeration, which cannot be cancelled.
#[derive(Clone)]
#[must_use]
pub struct GpuDiscovery {
    shared: Arc<Shared>,
}

struct Shared {
    selection: GpuSelection,
    state: Mutex<State>,
    /// Test seam: whether a consumer selected over the prefetched enumeration.
    #[cfg(test)]
    served: std::sync::atomic::AtomicBool,
}

enum State {
    Running(JoinHandle<Result<Vec<Candidate>, GpuError>>),
    /// Taken by a consumer, or never started: consumers enumerate as
    /// independent constructors do.
    Spent,
}

impl GpuDiscovery {
    /// Start, on one named thread, the instance creation and adapter
    /// enumeration an independent constructor would perform for `selection`.
    ///
    /// Never fails: a thread that cannot be started leaves nothing prefetched,
    /// and consumers then enumerate as independent constructors do.
    pub fn spawn(selection: GpuSelection) -> Self {
        let state = std::thread::Builder::new()
            .name("zetesis-gpu-discovery".into())
            .spawn(move || pollster::block_on(selection::enumerate_requested(selection)))
            .map_or(State::Spent, State::Running);
        Self::with_state(selection, state)
    }

    fn with_state(selection: GpuSelection, state: State) -> Self {
        Self {
            shared: Arc::new(Shared {
                selection,
                state: Mutex::new(state),
                #[cfg(test)]
                served: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// The API this discovery enumerates.
    #[must_use]
    pub fn selection(&self) -> GpuSelection {
        self.shared.selection
    }

    /// The enumeration, waiting for the thread if it is still running; `None`
    /// once taken, or when the thread never started or panicked.
    fn take(&self) -> Option<Result<Vec<Candidate>, GpuError>> {
        let state = std::mem::replace(
            &mut *self
                .shared
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
            State::Spent,
        );
        match state {
            State::Running(thread) => thread.join().ok(),
            State::Spent => None,
        }
    }

    /// The context a primitive with device `label`, capability check `validate`
    /// and timestamp request `timing` opens under `options`: its own selection
    /// over the enumeration, then its device requested from the chosen adapter.
    /// Without an enumeration to take this is an independent constructor's
    /// selection.
    ///
    /// # Errors
    /// The independent constructors' typed failures, at the same step: the
    /// enumeration's, the selection's, the device request's, or the granted
    /// limits' refusal by `validate`.
    pub(crate) async fn context(
        &self,
        options: GpuOptions,
        label: &'static str,
        validate: fn(&wgpu::Limits) -> Result<(), GpuError>,
        timing: bool,
    ) -> Result<GpuContext, GpuError> {
        let Some(enumeration) = self.take() else {
            return GpuContext::create_with_timing(
                options,
                self.selection(),
                label,
                validate,
                timing,
            )
            .await;
        };
        #[cfg(test)]
        self.shared
            .served
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let candidates = enumeration?;
        let index = selection::select_from(&candidates, options, self.selection(), validate)?;
        let candidate = &candidates[index];
        GpuContext::open(
            &candidate.adapter,
            candidate.info.clone(),
            label,
            validate,
            timing,
        )
        .await
    }

    /// Whether a consumer selected over this discovery's prefetched enumeration.
    #[cfg(test)]
    pub(crate) fn served(&self) -> bool {
        self.shared
            .served
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let state = std::mem::replace(
            self.state.get_mut().unwrap_or_else(PoisonError::into_inner),
            State::Spent,
        );
        if let State::Running(thread) = state {
            // The enumeration, with its adapter handles, is dropped with it.
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests;
