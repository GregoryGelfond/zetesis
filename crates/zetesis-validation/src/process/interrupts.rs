//! Interrupt registration for a process that supervises bounded children.
//!
//! The campaigns receive only a cancellation flag; this guard makes SIGINT and
//! SIGTERM set it. A handler performs no allocation, reporting, waits or group
//! signalling; the supervised capture's normal cleanup keeps those
//! responsibilities and its ownership checks.

use std::io;
use std::sync::{Arc, atomic::AtomicBool};

use signal_hook::{
    SigId,
    consts::signal::{SIGINT, SIGTERM},
};

/// SIGINT and SIGTERM registered to set a cancellation flag, until dropped.
pub struct Interrupts {
    registrations: Vec<SigId>,
}

impl Interrupts {
    /// Register SIGINT and SIGTERM to set `cancelled`.
    ///
    /// # Errors
    /// Returns the operating system's refusal to register either signal; any
    /// registration already made is removed when the partial guard drops.
    pub fn install(cancelled: &Arc<AtomicBool>) -> io::Result<Self> {
        let mut installed = Self {
            registrations: Vec::new(),
        };
        for signal in [SIGINT, SIGTERM] {
            installed
                .registrations
                .push(signal_hook::flag::register(signal, Arc::clone(cancelled))?);
        }
        Ok(installed)
    }
}

impl Drop for Interrupts {
    fn drop(&mut self) {
        // Unregistering removes these actions, not the operating system's
        // handler; the guard lives to process-entry return, so this is a
        // shutdown action, not a way to replace signal dispositions temporarily.
        for registration in self.registrations.drain(..) {
            signal_hook::low_level::unregister(registration);
        }
    }
}
