//! Process-only interruption registration for workflows owning child solvers.
//!
//! The libraries receive only the cancellation flag. A signal handler performs
//! no allocation, reporting, waits or group signalling; normal capture cleanup
//! retains those responsibilities and its existing ownership checks.

use std::io;
use std::sync::{Arc, atomic::AtomicBool};

use signal_hook::{
    SigId,
    consts::signal::{SIGINT, SIGTERM},
};

pub(super) struct Signals {
    registrations: Vec<SigId>,
}

impl Signals {
    pub(super) fn install(
        invocation: &crate::Invocation,
        cancelled: &Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let mut registered = Self {
            registrations: Vec::new(),
        };
        if matches!(
            invocation,
            crate::Invocation::Test(_)
                | crate::Invocation::Bench(crate::benchmark::BenchCommand::Corpus(_))
        ) {
            for signal in [SIGINT, SIGTERM] {
                registered
                    .registrations
                    .push(signal_hook::flag::register(signal, Arc::clone(cancelled))?);
            }
        }
        Ok(registered)
    }
}

impl Drop for Signals {
    fn drop(&mut self) {
        // This guard lives to process-entry return. Unregistering removes our
        // actions, not the OS handler; it is a shutdown action, not a general
        // library mechanism for temporarily replacing signal dispositions.
        for registration in self.registrations.drain(..) {
            signal_hook::low_level::unregister(registration);
        }
    }
}
