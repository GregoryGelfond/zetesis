//! Accumulate the two independent populations at an authenticated commit.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::time::Duration;

use zetesis_stable::{
    RegionalObservations, RegionalQueryObservation, RegionalSlotObservation, RegionalSlotState,
};
use zetesis_wgpu::ResidualReason;

use super::super::decoded::{DecodeJournal, DecodedSlot};
use super::reservoir::{BATCH_TAG, Reservoir, WORLD_TAG, bytes, reserve};
use super::values::{Totals, Value};
use super::{ResidualRunSamples, ResidualSampleGroup};
use crate::FormulaResidualStatistics;

const GROUPS: usize = 4;
const WORLD_METRICS: usize = 7;
const BATCH_METRICS: usize = 3;
const BATCH_TARGET: usize = 4_096;
const WORLDS_PER_BATCH: usize = 16;

fn group(reason: ResidualReason) -> usize {
    match reason {
        ResidualReason::FixedPoint => 1,
        ResidualReason::RoundLimit => 2,
        ResidualReason::WorkLimit => 3,
    }
}

#[derive(Clone, Copy)]
enum Query {
    Unknown,
    Certified,
    Entered(RegionalQueryObservation),
}

impl From<Option<&RegionalSlotObservation>> for Query {
    fn from(observation: Option<&RegionalSlotObservation>) -> Self {
        match observation {
            Some(RegionalSlotObservation {
                state:
                    RegionalSlotState::NoWitnessInSuppliedRegion
                    | RegionalSlotState::ValidatedCountermodel,
                query: Some(query),
            }) => Self::Entered(*query),
            Some(slot) if slot.state == RegionalSlotState::Certified => Self::Certified,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy)]
struct World {
    reason: Option<ResidualReason>,
    open: Option<u64>,
    query: Query,
}

impl World {
    fn value(self, field: usize) -> Value {
        if field == 0 {
            return Value::optional(self.open);
        }
        let q = match self.query {
            Query::Unknown => return Value::UnknownEligibility,
            Query::Certified => return Value::Outside,
            Query::Entered(q) => q,
        };
        match field {
            1 => Value::Known(q.visited as u128),
            2 => Value::Known(q.attempted_splits as u128),
            3 => Value::Known(q.published_splits as u128),
            4 if q.visited == 0 => Value::Outside,
            4 => Value::optional(q.deepest.map(|d| d as u128)),
            5 => Value::Known(u128::from(q.work)),
            6 => Value::optional(q.host_time.map(|t| t.as_nanos())),
            _ => unreachable!("world observation index"),
        }
    }
}

#[derive(Clone, Copy)]
struct Batch {
    residuals: [Option<u64>; GROUPS],
    width: Option<u64>,
    times: [Option<Duration>; GROUPS],
}

impl Batch {
    fn width(self, group: usize) -> Option<u64> {
        if self.residuals[group] == Some(0) || self.width == Some(0) {
            Some(0)
        } else if group == 0
            || self.residuals[group].is_some() && self.residuals[group] == self.residuals[0]
        {
            self.width
        } else {
            None
        }
    }

    fn value(self, group: usize, field: usize) -> Value {
        match field {
            0 => Value::optional(self.residuals[group]),
            1 => Value::optional(self.width(group)),
            2 => Value::optional(self.times[group].map(|t| t.as_nanos())),
            _ => unreachable!("batch observation index"),
        }
    }
}

#[derive(Default)]
struct Group {
    worlds: [Totals; WORLD_METRICS],
    batches: [Totals; BATCH_METRICS],
    mixed_upper: u64,
    unbounded_width: bool,
}

impl Group {
    fn world(&mut self, world: World) {
        for (field, total) in self.worlds.iter_mut().enumerate() {
            total.observe(world.value(field));
        }
    }

    fn unjoined(&mut self, count: Option<u64>) {
        if count == Some(0) {
            return;
        }
        self.worlds[0].missing_members(count);
        for total in &mut self.worlds[1..] {
            total.observe(Value::UnknownEligibility);
        }
    }

    fn batch(&mut self, batch: Batch, index: usize) {
        for (field, total) in self.batches.iter_mut().enumerate() {
            total.observe(batch.value(index, field));
        }
        if batch.width(index).is_none() {
            if let Some(bound) = batch.width {
                self.mixed_upper = self.mixed_upper.max(bound);
            } else {
                self.unbounded_width = true;
            }
        }
    }

    fn finish(
        &self,
        index: usize,
        worlds: &mut [World],
        batches: &mut [Batch],
    ) -> ResidualSampleGroup {
        let [
            open_atoms,
            visited,
            attempted_splits,
            published_splits,
            depth,
            work,
            query_ns,
        ] = std::array::from_fn(|field| {
            self.worlds[field].finish(worlds, |world| {
                if index == 0 || world.reason.map(group) == Some(index) {
                    world.value(field).measured()
                } else {
                    None
                }
            })
        });
        let [residuals, mut width, batch_ns] = std::array::from_fn(|field| {
            self.batches[field].finish(batches, |batch| batch.value(index, field).measured())
        });
        if !self.unbounded_width
            && width.known.is_some()
            && let Some(known) = self.batches[1].observed_maximum()
            && known >= u128::from(self.mixed_upper)
        {
            width.maximum = Some(known);
        }
        ResidualSampleGroup {
            open_atoms,
            visited,
            attempted_splits,
            published_splits,
            depth,
            work,
            query_ns,
            residuals,
            width,
            batch_ns,
        }
    }
}

struct Sampler {
    ceiling: u64,
    seed: u64,
    peak: u128,
    worlds: Reservoir<World>,
    batches: Reservoir<Batch>,
    groups: [Group; GROUPS],
    settled: Option<ResidualRunSamples>,
}

/// Compact session handle. One fallibly allocated owner holds both fixed
/// capacity plans. The public snapshot never owns either raw reservoir.
pub(crate) struct RunSamples {
    state: Vec<Sampler>,
    // Keep transport state outside the charged ordinary sampling header.
    // Its one slot is allocated only for a separately requested export.
    export: Vec<export::Export>,
}

fn capacities(payload: u128) -> (usize, usize) {
    let world = bytes::<World>(1);
    let batch = bytes::<Batch>(1);
    let block = WORLDS_PER_BATCH as u128 * world + batch;
    let full =
        usize::try_from((payload / block).min(BATCH_TARGET as u128)).expect("bounded block count");
    let rest = payload - full as u128 * block;
    if full == BATCH_TARGET || rest < world {
        return (full * WORLDS_PER_BATCH, full);
    }
    if rest < world + batch {
        return (full * WORLDS_PER_BATCH + 1, full);
    }
    // The remainder is less than a full block.
    (
        full * WORLDS_PER_BATCH
            + usize::try_from((rest - batch) / world).expect("bounded partial block"),
        full + 1,
    )
}

impl RunSamples {
    pub(crate) fn new(max_batch_bytes: u64) -> Option<Self> {
        let ceiling = max_batch_bytes / 8;
        if bytes::<Sampler>(1) > u128::from(ceiling) {
            return None;
        }
        Self::seeded(
            ceiling,
            RandomState::new().hash_one("zetesis-residual-samples-v1"),
        )
    }

    fn seeded(ceiling: u64, seed: u64) -> Option<Self> {
        let header = bytes::<Sampler>(1);
        let mut peak = 0;
        let mut state = reserve(1, header.min(u128::from(ceiling)), 0, &mut peak)?;
        // A finalized record and one output copy can overlap. Further clones
        // held by library callers have their own ownership.
        let payload =
            u128::from(ceiling).saturating_sub(header + 2 * bytes::<ResidualSampleGroup>(GROUPS));
        let (worlds, batches) = capacities(payload);
        state.push(Sampler {
            ceiling,
            seed,
            peak,
            worlds: Reservoir::new(worlds, seed, WORLD_TAG),
            batches: Reservoir::new(batches, seed, BATCH_TAG),
            groups: Default::default(),
            settled: None,
        });
        Some(Self {
            state,
            export: Vec::new(),
        })
    }

    pub(crate) fn commit(
        &mut self,
        counts: Option<FormulaResidualStatistics>,
        decoded: &DecodeJournal,
        receipt: Option<&RegionalObservations>,
        timed: bool,
    ) {
        if self.state[0].settled.is_none() {
            self.state[0].commit(counts, decoded, receipt, timed);
        }
    }

    /// Published snapshots share no raw sample records. A missing optional
    /// group copy does not alter the cached terminal statistics.
    pub(crate) fn snapshot(&self) -> ResidualRunSamples {
        let state = &self.state[0];
        state
            .settled
            .as_ref()
            .map_or_else(|| state.snapshot(), ResidualRunSamples::try_snapshot)
    }

    /// Select the terminal ranks once, replace both sample allocations with
    /// the compact terminal cache and leave repeated snapshots read-only.
    pub(crate) fn settle(&mut self) {
        let state = &mut self.state[0];
        if state.settled.is_some() {
            return;
        }
        // Preserve the reservoir order before rank selection permutes the records.
        if let Some(offer) = self.export.first_mut() {
            offer.write(state);
        }
        let live = state.retained_bytes();
        let mut result = state.snapshot();
        let report_bytes = bytes::<ResidualSampleGroup>(GROUPS);
        if live + 2 * report_bytes <= u128::from(state.ceiling)
            && let Some(mut groups) = reserve(GROUPS, report_bytes, live, &mut state.peak)
        {
            for (i, group) in state.groups.iter().enumerate() {
                groups.push(group.finish(i, &mut state.worlds.records, &mut state.batches.records));
            }
            result.groups = Some(groups);
        }
        state.worlds.records = Vec::new();
        state.batches.records = Vec::new();
        let retained = state.retained_bytes()
            + result
                .groups
                .as_ref()
                .map_or(0, |g| bytes::<ResidualSampleGroup>(g.capacity()));
        result.retained_bytes = u64::try_from(retained).expect("admitted cache");
        result.peak_bytes = u64::try_from(state.peak).unwrap_or(u64::MAX);
        state.settled = Some(result);
    }
}

impl Sampler {
    fn retained_bytes(&self) -> u128 {
        bytes::<Self>(1) + self.worlds.retained_bytes() + self.batches.retained_bytes()
    }

    fn snapshot(&self) -> ResidualRunSamples {
        ResidualRunSamples {
            seed: self.seed,
            byte_limit: self.ceiling,
            peak_bytes: u64::try_from(self.peak).unwrap_or(u64::MAX),
            retained_bytes: u64::try_from(self.retained_bytes())
                .expect("admitted observation storage"),
            worlds: self.worlds.population(),
            batches: self.batches.population(),
            groups: None,
        }
    }

    fn world(&mut self, world: World) {
        self.groups[0].world(world);
        if let Some(reason) = world.reason {
            self.groups[group(reason)].world(world);
        }
        self.worlds
            .insert(world, self.retained_bytes(), &mut self.peak);
    }
}

impl Sampler {
    /// Counts come from the complete decode of this ticket. When optional
    /// joins are unavailable, preserve observations that do not need the join.
    fn commit(
        &mut self,
        counts: Option<FormulaResidualStatistics>,
        decoded: &DecodeJournal,
        receipt: Option<&RegionalObservations>,
        timed: bool,
    ) {
        let frame = SlotFrame::new(counts, decoded, receipt);
        let SlotFrame {
            residuals,
            keys,
            slots: known_slots,
            paired,
        } = frame;
        let homogeneous = [
            ResidualReason::FixedPoint,
            ResidualReason::RoundLimit,
            ResidualReason::WorkLimit,
        ]
        .into_iter()
        .find(|&r| residuals[0].is_some_and(|n| n != 0 && residuals[group(r)] == Some(n)));
        let mut batch = Batch {
            residuals,
            width: receipt.and_then(RegionalObservations::width),
            times: [timed.then_some(Duration::ZERO); GROUPS],
        };
        let mut joined = true;
        if let Some(count) = residuals[0] {
            if let Some(keys) = keys.filter(|_| paired || known_slots.is_none()) {
                for key in keys {
                    let q = Query::from(
                        known_slots
                            .and_then(|s| s.get(key.slot))
                            .and_then(Option::as_ref),
                    );
                    add_time(&mut batch.times[0], q);
                    add_time(&mut batch.times[group(key.reason)], q);
                    self.world(World {
                        reason: Some(key.reason),
                        open: Some(key.open_atoms),
                        query: q,
                    });
                }
            } else if let Some(slots) = known_slots {
                joined = homogeneous.is_some() || count == 0;
                for slot in slots.iter().flatten() {
                    let query = Query::from(Some(slot));
                    add_time(&mut batch.times[0], query);
                    self.world(World {
                        reason: homogeneous,
                        open: None,
                        query,
                    });
                }
            } else {
                joined = homogeneous.is_some() || count == 0;
                if count != 0 {
                    batch.times[0] = None;
                }
                for _ in 0..count {
                    self.world(World {
                        reason: homogeneous,
                        open: None,
                        query: Query::Unknown,
                    });
                }
            }
        } else {
            self.worlds.lose_population();
            batch.times[0] = None;
            self.groups[0].unjoined(None);
            joined = false;
        }
        for index in 1..GROUPS {
            if !joined {
                self.groups[index].unjoined(residuals[index]);
            }
            if residuals[index] == Some(0) {
                batch.times[index] = timed.then_some(Duration::ZERO);
            } else if residuals[index].is_some() && residuals[index] == residuals[0] {
                batch.times[index] = batch.times[0];
            } else if !paired {
                batch.times[index] = None;
            }
        }
        for (index, group) in self.groups.iter_mut().enumerate() {
            group.batch(batch, index);
        }
        self.batches
            .insert(batch, self.retained_bytes(), &mut self.peak);
    }
}

/// The decode journal and counts are written together from the same original
/// device verdict. The CPU receipt is independently produced, so validate its
/// ordered indices once before associating per-world observations.
struct SlotFrame<'a> {
    residuals: [Option<u64>; GROUPS],
    keys: Option<&'a [DecodedSlot]>,
    slots: Option<&'a [Option<RegionalSlotObservation>]>,
    paired: bool,
}

impl<'a> SlotFrame<'a> {
    fn new(
        counts: Option<FormulaResidualStatistics>,
        decoded: &'a DecodeJournal,
        receipt: Option<&'a RegionalObservations>,
    ) -> Self {
        let keys = decoded.rows();
        let slots = receipt.and_then(RegionalObservations::slots);
        let counts = counts.or_else(|| {
            keys.map(|rows| {
                let mut totals = FormulaResidualStatistics::default();
                for row in rows {
                    *totals.count_for(row.reason) += 1;
                }
                totals
            })
        });
        let mut residuals = [None; GROUPS];
        if let Some(counts) = counts {
            residuals = [
                counts
                    .fixed_point
                    .checked_add(counts.round_limit)
                    .and_then(|n| n.checked_add(counts.work_limit)),
                Some(counts.fixed_point),
                Some(counts.round_limit),
                Some(counts.work_limit),
            ];
        }
        let keys = keys.filter(|rows| residuals[0] == Some(rows.len() as u64));
        let paired = keys.zip(slots).is_some_and(|(rows, source)| {
            rows.iter().map(|k| k.slot).eq(source
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.as_ref().map(|_| i)))
        });
        let slots = if paired {
            slots
        } else {
            slots.filter(|source| {
                let n = source.iter().flatten().count() as u64;
                if counts.is_none() {
                    residuals[0] = Some(n);
                }
                residuals[0] == Some(n)
            })
        };
        if residuals[0] == Some(0) {
            residuals = [Some(0); GROUPS];
        }
        Self {
            residuals,
            keys,
            slots,
            paired,
        }
    }
}

fn add_time(sum: &mut Option<Duration>, query: Query) {
    match query {
        Query::Unknown => *sum = None,
        Query::Certified => {}
        Query::Entered(q) => *sum = sum.and_then(|n| n.checked_add(q.host_time?)),
    }
}

#[cfg(test)]
mod tests;

mod export;
