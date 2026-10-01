//! Compact publication configuration derived from typed execution observations.

use std::{io::Write, num::NonZeroUsize};

use crate::{AnswerRenderer, ExecutionObservation, ExecutionObserver, RunError};

#[cfg(test)]
mod tests;

/// Materialization selected for the admitted program, independently of backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundingDisplay {
    /// The complete ground program is materialized before membership checking.
    Eager,
    /// Source joins supply consequences while candidates are checked.
    Lazy,
    /// An eager core is checked together with streamed source constraints.
    Hybrid,
    /// An eager base is checked and terminal definitions reconstructed on the host.
    TerminalDefinitions,
}

/// Selected execution hardware, distinct from a requested backend preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendView<'a> {
    /// Host execution was selected.
    Cpu,
    /// A device executor was prepared; this does not establish completed GPU work.
    Gpu {
        /// Adapter name reported by the prepared executor.
        adapter: &'a str,
        /// Compute API reported by the prepared executor.
        api: &'a str,
        /// General device propagation includes exact residual completion on the CPU.
        cpu_completion: bool,
    },
}

/// Borrowed configuration at a selected execution boundary.
///
/// Later backend selection, such as automatic CPU-to-device promotion, produces
/// another view. Values describe execution setup, not measured utilization or
/// established answer-set membership. Construction neither allocates nor parses
/// diagnostic text; a renderer controls its own storage and output costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurationView<'a> {
    /// Selected host/device execution, including the observed adapter when present.
    pub backend: BackendView<'a>,
    /// Effective grounding or reconstruction mode of the original program.
    pub grounding: GroundingDisplay,
    /// Configured host worker capacity, not the number of simultaneously active threads.
    pub workers: NonZeroUsize,
}

/// Retain only fixed-size execution facts between observation callbacks.
pub(crate) struct Configuration {
    grounding: GroundingDisplay,
    workers: NonZeroUsize,
}

impl Configuration {
    pub(crate) const fn new(workers: NonZeroUsize) -> Self {
        Self {
            grounding: GroundingDisplay::Eager,
            workers,
        }
    }

    fn observe<'a>(&mut self, event: &ExecutionObservation<'a>) -> Option<ConfigurationView<'a>> {
        use ExecutionObservation as Event;
        let backend = match event {
            Event::TerminalDefinitions { .. } => {
                self.grounding = GroundingDisplay::TerminalDefinitions;
                return None;
            }
            Event::HybridGrounding { .. } => {
                self.grounding = GroundingDisplay::Hybrid;
                return None;
            }
            Event::StaticGrounding { .. } => {
                self.grounding = GroundingDisplay::Eager;
                return None;
            }
            Event::LazyGrounding { .. } => {
                self.grounding = GroundingDisplay::Lazy;
                return None;
            }
            Event::CpuClosure { grounder, .. } => {
                // The executor reports an effective closure mode, not its request.
                self.grounding = if *grounder == crate::Grounder::Eager {
                    GroundingDisplay::Eager
                } else {
                    GroundingDisplay::Lazy
                };
                BackendView::Cpu
            }
            Event::CpuFormula { .. } => BackendView::Cpu,
            #[cfg(feature = "gpu")]
            Event::LazyDeviceGrounding { .. } => {
                self.grounding = GroundingDisplay::Lazy;
                return None;
            }
            #[cfg(feature = "gpu")]
            Event::DeviceClosure {
                adapter,
                static_counts,
            } => {
                self.grounding = if static_counts.is_some() {
                    GroundingDisplay::Eager
                } else {
                    GroundingDisplay::Lazy
                };
                BackendView::Gpu {
                    adapter: adapter.name,
                    api: adapter.backend.label(),
                    cpu_completion: false,
                }
            }
            #[cfg(feature = "gpu")]
            Event::DeviceFormula { adapter, .. } => BackendView::Gpu {
                adapter: adapter.name,
                api: adapter.backend.label(),
                cpu_completion: true,
            },
            #[cfg(feature = "gpu")]
            Event::DeviceTight { adapter, .. } => BackendView::Gpu {
                adapter: adapter.name,
                api: adapter.backend.label(),
                cpu_completion: false,
            },
            _ => return None,
        };
        Some(ConfigurationView {
            backend,
            grounding: self.grounding,
            workers: self.workers,
        })
    }
}

/// Compose existing diagnostics and the renderer without retaining solver events.
pub(crate) struct Observer<'a, R, W> {
    renderer: &'a mut R,
    diagnostics: &'a mut crate::presentation::Diagnostics<W>,
    configuration: &'a mut Configuration,
    phases: &'a crate::phase_timing::Recorder,
}

impl<'a, R, W> Observer<'a, R, W> {
    pub(crate) fn new(
        renderer: &'a mut R,
        diagnostics: &'a mut crate::presentation::Diagnostics<W>,
        configuration: &'a mut Configuration,
        phases: &'a crate::phase_timing::Recorder,
    ) -> Self {
        Self {
            renderer,
            diagnostics,
            configuration,
            phases,
        }
    }
}

impl<R: AnswerRenderer, W: Write> ExecutionObserver for Observer<'_, R, W> {
    type Error = RunError;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        let _output = self.phases.enter(crate::SolvePhase::ObservationOutput);
        let configuration = self.configuration.observe(&event);
        self.diagnostics.observe(event)?;
        if let Some(configuration) = configuration {
            self.renderer.configuration(configuration)?;
        }
        Ok(())
    }
}
