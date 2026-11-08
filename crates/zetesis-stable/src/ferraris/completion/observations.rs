//! Bounded, opt-in receipts for supplied-region completion. Only the worker
//! serving a slot can write its observations. Level bins are shared by
//! successive queries on that worker and reduced after all workers join.

use std::time::Duration;
use zetesis_algebra::{MeterBytes, region::Region};
use zetesis_ferraris::SlateId;

use super::{BatchVerdict, CompletionExecutor, Incomplete};
use crate::ferraris::regions::depth::DepthWorkspace;

/// Disposition of one originally residual slot in a supplied-region attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionalSlotState {
    /// Neither certification nor completion entered this slot.
    Unentered,
    /// An original-program certificate settled the slot before regional search.
    Certified,
    /// No proper witness exists in the supplied region.
    NoWitnessInSuppliedRegion,
    /// Completion validated a proper reduct witness.
    ValidatedCountermodel,
    /// Certification or regional completion returned this typed stop.
    Stopped(Incomplete),
}

/// Work actually performed by one entered supplied-region query, including
/// the prefix of a stopped query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionalQueryObservation {
    /// Regions successfully narrowed and recorded as visited.
    pub visited: usize,
    /// Split attempts counted by the walk.
    pub attempted_splits: usize,
    /// Split pairs actually published.
    pub published_splits: usize,
    /// Maximum visited depth (root zero), absent without visits or depth storage.
    pub deepest: Option<usize>,
    /// Work charged during the query, including proper-witness validation.
    pub work: u64,
    /// Measured reduct interval, absent when timing is disabled or overflowed.
    pub host_time: Option<Duration>,
}

/// Slot disposition and the receipt if the regional query was actually called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionalSlotObservation {
    /// Latest disposition in this attempt.
    pub state: RegionalSlotState,
    /// Returned work even if the query stopped before completion.
    pub query: Option<RegionalQueryObservation>,
}

/// One bounded attempt's receipts in original slate order.
#[derive(Debug)]
pub struct RegionalObservations {
    slate: SlateId,
    attempt: u64,
    max_bytes: u64,
    retained_bytes: u64,
    peak_bytes: u64,
    pub(super) slots: Option<Vec<Option<RegionalSlotObservation>>>,
    width: Option<u64>,
    timed: bool,
}

impl RegionalObservations {
    /// The identity of the oldest slate served.
    #[must_use]
    pub const fn slate(&self) -> SlateId {
        self.slate
    }
    /// The attempt identifier supplied by the driver.
    #[must_use]
    pub const fn attempt(&self) -> u64 {
        self.attempt
    }
    /// Original-slot-indexed records. An empty slot was originally terminal.
    /// An entirely terminal slate returns an empty slice without allocating.
    /// The outer option is absent when record storage could not be admitted.
    #[must_use]
    pub fn slots(&self) -> Option<&[Option<RegionalSlotObservation>]> {
        self.slots.as_deref()
    }
    /// Maximum over depths of the visit counts summed across all entered
    /// queries, including stopped prefixes. Zero means no visits. Absent
    /// when optional storage or an overflow prevented an exact result.
    #[must_use]
    pub const fn width(&self) -> Option<u64> {
        self.width
    }
    /// Sum of recorded query intervals. Parallel intervals can overlap.
    /// An enabled clock gives zero when no query was entered. Disabled
    /// timing, overflow or unavailable slot records yield no duration.
    #[must_use]
    pub fn host_time(&self) -> Option<Duration> {
        self.timed.then_some(())?;
        self.slots()?
            .iter()
            .flatten()
            .filter_map(|s| s.query)
            .try_fold(Duration::ZERO, |sum, q| sum.checked_add(q.host_time?))
    }
    /// Actual optional heap payload retained with the receipt, in bytes.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
    /// Peak admitted optional heap payload including worker depth storage.
    #[must_use]
    pub const fn peak_bytes(&self) -> u64 {
        self.peak_bytes
    }

    pub(super) fn workers(
        &mut self,
        rows: usize,
        chunk_size: usize,
        roots: &[(usize, Region)],
    ) -> Option<Vec<Option<DepthWorkspace>>> {
        self.slots.as_ref()?;
        if roots.is_empty() {
            return Some(Vec::new());
        }
        let chunks = rows.div_ceil(chunk_size);
        let mut used = u128::from(self.retained_bytes());
        let max = u128::from(self.max_bytes);
        let mut depths = reserve(chunks, max - used)?;
        used += bytes(&depths);
        self.peak_bytes = self.peak_bytes.max(u64::try_from(used).ok()?);
        // The authenticated residual slots are in ascending order. Their
        // indices stay fixed when a certificate removes other roots.
        let mut groups = roots
            .chunk_by(|a, b| a.0 / chunk_size == b.0 / chunk_size)
            .peekable();
        for chunk in 0..chunks {
            let maximum = groups
                .next_if(|group| group[0].0 / chunk_size == chunk)
                .and_then(|group| {
                    group
                        .iter()
                        .map(|(_, root)| {
                            (0..root.mask_words())
                                .map(|i| root.open_word(i, u64::MAX).count_ones() as usize)
                                .sum()
                        })
                        .max()
                });
            let worker = maximum.and_then(|open| {
                let mut local_peak = 0;
                let workspace = DepthWorkspace::with_peak(open, max - used, &mut local_peak);
                // Both quantities were admitted under the same u64 ceiling.
                self.peak_bytes = self.peak_bytes.max(u64::try_from(used + local_peak).ok()?);
                workspace
            });
            if let Some(worker) = &worker {
                used += worker.payload_bytes();
            }
            depths.push(worker);
        }
        Some(depths)
    }

    pub(super) fn finish(&mut self, depths: Option<&[Option<DepthWorkspace>]>) {
        let Some(slots) = self.slots() else { return };
        let mut visited = false;
        for query in slots.iter().flatten().filter_map(|slot| slot.query) {
            if query.visited != 0 {
                if query.deepest.is_none() {
                    self.width = None;
                    return;
                }
                visited = true;
            }
        }
        if !visited {
            self.width = Some(0);
            return;
        }
        self.width = depths.and_then(|workers| {
            let measured = workers.iter().flatten();
            let levels = measured
                .clone()
                .try_fold(0, |max, d| Some(max.max(d.levels()?.len())))?;
            (0..levels).try_fold(0, |peak, level| {
                let sum = measured.clone().try_fold(0u64, |sum, d| {
                    sum.checked_add(d.levels()?.get(level).copied().unwrap_or(0))
                })?;
                Some(peak.max(sum))
            })
        });
    }
}

fn bytes<T>(slots: &Vec<T>) -> u128 {
    slots.capacity() as u128 * size_of::<T>() as u128
}

fn reserve<T>(count: usize, limit: u128) -> Option<Vec<T>> {
    MeterBytes::check(count as u128 * size_of::<T>() as u128, limit).ok()?;
    let mut slots = Vec::new();
    slots.try_reserve_exact(count).ok()?;
    MeterBytes::check(bytes(&slots), limit).ok().map(|()| slots)
}

impl CompletionExecutor {
    /// Collect the next supplied-region pass under an independent optional
    /// heap allowance. Replaces any previous request or retained receipt.
    /// The driver supplies a distinct identifier for each retry of a slate.
    /// A rejected ticket or malformed region list consumes the request and
    /// produces no receipt. Once authenticated, the attempt retains observed
    /// prefixes even if certification or completion stops.
    ///
    /// Record storage is optional. A byte refusal leaves the original query,
    /// its work allowance and its verdict unchanged. A slot receipt can
    /// retain charged work and split counts when depth storage is unavailable.
    pub fn observe_next_regional_pass(&mut self, attempt: u64, max_bytes: u64) {
        self.regional = None;
        self.request = Some((attempt, max_bytes));
    }
    /// Last authenticated observed attempt, including returned failures.
    #[must_use]
    pub const fn last_regional_observations(&self) -> Option<&RegionalObservations> {
        self.regional.as_ref()
    }
    /// Transfer the last receipt; dropping it releases its optional storage.
    pub fn take_regional_observations(&mut self) -> Option<RegionalObservations> {
        self.regional.take()
    }

    pub(in crate::ferraris) fn take_observation_request(&mut self) -> Option<(u64, u64)> {
        self.regional = None;
        self.request.take()
    }

    pub(in crate::ferraris) fn begin_observations(
        &mut self,
        request: Option<(u64, u64)>,
        slate: SlateId,
        tags: &[BatchVerdict],
        timed: bool,
    ) {
        self.regional = request.map(|(attempt, max_bytes)| {
            let slots = if tags.contains(&BatchVerdict::Residual) {
                reserve(tags.len(), u128::from(max_bytes)).map(|mut slots| {
                    slots.extend(tags.iter().map(|tag| {
                        (*tag == BatchVerdict::Residual).then_some(RegionalSlotObservation {
                            state: RegionalSlotState::Unentered,
                            query: None,
                        })
                    }));
                    slots
                })
            } else {
                Some(Vec::new())
            };
            let peak_bytes = slots.as_ref().map_or(0, |s| {
                u64::try_from(bytes(s)).expect("bounded observation storage")
            });
            let width = slots.as_ref().map(|_| 0);
            RegionalObservations {
                slate,
                attempt,
                max_bytes,
                retained_bytes: peak_bytes,
                peak_bytes,
                slots,
                width,
                timed,
            }
        });
    }

    pub(in crate::ferraris) fn mark_certified(
        &mut self,
        tags: &[BatchVerdict],
        mut stopped: Option<Incomplete>,
    ) {
        if let Some(slots) = self.regional.as_mut().and_then(|r| r.slots.as_deref_mut()) {
            for (slot, tag) in slots.iter_mut().zip(tags) {
                if let Some(slot) = slot {
                    if *tag != BatchVerdict::Residual {
                        slot.state = RegionalSlotState::Certified;
                    } else if let Some(stop) = stopped.take() {
                        // certify_pending visits residuals in order and replaces
                        // each tag only after its certificate check succeeds.
                        slot.state = RegionalSlotState::Stopped(stop);
                    }
                }
            }
        }
    }
}
