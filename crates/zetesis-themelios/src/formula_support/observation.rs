//! Attribute accepted formula charges without changing their admission path.

use super::{Counters, Event};

impl Counters {
    pub(super) fn observe_work<T>(
        &mut self,
        event: fn(u64) -> Event,
        action: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if !self.observed.enabled() {
            return action(self);
        }
        let start = self.accounting.work;
        let measured = Accepted {
            counters: self,
            start,
            event,
        };
        action(&mut *measured.counters)
    }
}

/// Accounting is monotone while borrowed by the action. The guard records its
/// accepted prefix on ordinary return, typed refusal and unwind. It neither
/// owns accounting nor retains an observer borrow while the action runs.
struct Accepted<'a> {
    counters: &'a mut Counters,
    start: u64,
    event: fn(u64) -> Event,
}

impl Drop for Accepted<'_> {
    fn drop(&mut self) {
        self.counters
            .record((self.event)(self.counters.accounting.work - self.start));
    }
}

#[cfg(test)]
mod tests;
