//! Checked exact transport shapes, acquired-mask packing and complete readback.

use super::preparation::{Numeric, reserve, signed};
use super::{
    AggregateGpuEvaluation, AggregateGpuLimits, AggregateGpuPlan, AggregateGpuReduction,
    AggregateGpuValue, LANES, PARAM_BYTES, RESULT_WORDS, capacity, poll,
};
use crate::{GpuError, GpuErrorKind};
use std::cmp::Ordering;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::native_aggregate::{Eligibility, Function};

pub(super) struct Plan {
    pub(super) worlds: u32,
    pub(super) stride: u32,
    pub(super) mask_words: usize,
    pub(super) masks: u64,
    pub(super) results: u64,
    pub(super) transport: u64,
    pub(super) accounted: u64,
    pub(super) host_work: u64,
    pub(super) work: u32,
    pub(super) total_work: u64,
    pub(super) epoch: u32,
}

impl Plan {
    pub(super) fn new(
        group: &AggregateGpuPlan<'_>,
        records: &[Eligibility<'_>],
        limits: AggregateGpuLimits,
        device: &wgpu::Limits,
        epoch: u32,
        cancellation: &Cancellation,
    ) -> Result<Self, GpuError> {
        let numeric = &group.numeric;
        let failure = || capacity("aggregate batch dimensions or accounting overflow");
        let worlds = u32::try_from(records.len()).map_err(|_| failure())?;
        let mask_width = numeric.tuple_count.div_ceil(32);
        let stride = mask_width
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(failure)?;
        let mask_words =
            usize::try_from(u64::from(stride) * u64::from(worlds)).map_err(|_| failure())?;
        u32::try_from(mask_words).map_err(|_| failure())?;
        let masks = bytes(mask_words)?;
        let result_words = records
            .len()
            .checked_mul(RESULT_WORDS)
            .ok_or_else(failure)?;
        u32::try_from(result_words).map_err(|_| failure())?;
        let results = bytes(result_words)?;
        let transport = add(&[PARAM_BYTES, masks, results, results])?;
        let decoded = records
            .len()
            .checked_mul(size_of::<AggregateGpuReduction>())
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(failure)?;
        let accounted = add(&[
            numeric.bytes,
            numeric.bytes,
            transport,
            masks,
            results,
            decoded,
        ])?;
        let work = device_work(numeric.tuple_count, numeric.guard_count)?;
        let total_work = u64::from(work) * u64::from(worlds);
        let host_per = add(&[
            u64::from(numeric.tuple_count),
            2,
            2 * u64::from(numeric.guard_count),
        ])?;
        let host_work = host_per
            .checked_mul(u64::from(worlds))
            .and_then(|value| value.checked_add(u64::try_from(mask_words).ok()?))
            .ok_or_else(failure)?;
        if records.len() > limits.max_occurrences
            || worlds > device.max_compute_workgroups_per_dimension
            || accounted > limits.max_batch_bytes
            || host_work > limits.max_host_work
            || total_work > limits.max_device_work
        {
            return Err(capacity("aggregate batch resource ceiling exceeded"));
        }
        crate::runtime::positive_timeout(limits.timeout)?;
        for size in [
            bytes(numeric.tuples.len())?,
            bytes(numeric.guards.len())?,
            masks,
            results,
        ] {
            if size > device.max_storage_buffer_binding_size || size > device.max_buffer_size {
                return Err(capacity(
                    "aggregate storage binding exceeds granted device limits",
                ));
            }
        }
        if PARAM_BYTES > device.max_uniform_buffer_binding_size
            || PARAM_BYTES > device.max_buffer_size
        {
            return Err(capacity("aggregate uniform exceeds granted device limits"));
        }
        for record in records {
            poll(cancellation)?;
            if !record.group().same_group(group.group) {
                return Err(GpuError::new(
                    GpuErrorKind::Seed,
                    "eligibility belongs to a different aggregate Group",
                ));
            }
        }
        Ok(Self {
            worlds,
            stride,
            mask_words,
            masks,
            results,
            transport,
            accounted,
            host_work,
            work,
            total_work,
            epoch,
        })
    }

    pub(super) fn pack(
        &self,
        records: &[Eligibility<'_>],
        cancellation: &Cancellation,
    ) -> Result<Vec<u32>, GpuError> {
        let mut packed = reserve(self.mask_words)?;
        packed.resize(self.mask_words, 0);
        let words = (self.stride as usize - 1) / 2;
        for (record, row) in records
            .iter()
            .zip(packed.chunks_exact_mut(self.stride as usize))
        {
            poll(cancellation)?;
            row[0] = u32::from(record.frozen().is_some());
            for (index, selected) in record.original().iter().copied().enumerate() {
                poll(cancellation)?;
                if selected {
                    row[1 + index / 32] |= 1 << (index % 32);
                }
                if record.frozen().is_some_and(|frozen| frozen[index]) {
                    row[1 + words + index / 32] |= 1 << (index % 32);
                }
            }
        }
        Ok(packed)
    }

    pub(super) fn params(&self, numeric: &Numeric) -> [u32; 8] {
        [
            numeric.tuple_count,
            numeric.guard_count,
            self.worlds,
            function(numeric.function),
            self.stride,
            self.epoch,
            self.work,
            0,
        ]
    }
}

pub(super) fn device_work(tuples: u32, guards: u32) -> Result<u32, GpuError> {
    // Two phase scans plus two fixed reduction trees. This bound also implies
    // tuples + guards <= u32::MAX / 2 - 63: every doubled contribution/guard
    // index and the final tuple += 64 increment therefore fit. Mask/result row
    // addresses have separate complete-buffer u32 checks in Plan::new.
    tuples
        .checked_add(guards)
        .and_then(|value| value.checked_add(LANES - 1))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| capacity("aggregate complete device work exceeds u32"))
}

pub(super) fn decode(
    words: &[u32],
    numeric: &Numeric,
    plan: &Plan,
    masks: &[u32],
    cancellation: &Cancellation,
) -> Result<Vec<AggregateGpuReduction>, GpuError> {
    if bytes(words.len())? != plan.results {
        return Err(malformed());
    }
    let mut results = reserve(plan.worlds as usize)?;
    for (world, record) in words.chunks_exact(RESULT_WORDS).enumerate() {
        poll(cancellation)?;
        let paired = masks[world * plan.stride as usize];
        if record[0] != plan.epoch
            || record[1] as usize != world
            || record[2] != paired
            || record[9] != plan.work
        {
            return Err(malformed());
        }
        let original = evaluation(numeric, record[3], record[4], record[5], cancellation)?;
        let frozen = evaluation(numeric, record[6], record[7], record[8], cancellation)?;
        results.push(AggregateGpuReduction {
            original,
            frozen: (paired == 1).then_some(frozen),
        });
    }
    Ok(results)
}

fn evaluation(
    numeric: &Numeric,
    word: u32,
    present: u32,
    holds: u32,
    cancellation: &Cancellation,
) -> Result<AggregateGpuEvaluation, GpuError> {
    if present > 1 || holds > 1 {
        return Err(malformed());
    }
    let value = if present == 1 {
        let value = signed(word);
        if value < numeric.lower || value > numeric.upper {
            return Err(malformed());
        }
        AggregateGpuValue::Integer(value)
    } else {
        if word != 0 {
            return Err(malformed());
        }
        match numeric.function {
            Function::Min => AggregateGpuValue::Supremum,
            Function::Max => AggregateGpuValue::Infimum,
            _ => return Err(malformed()),
        }
    };
    let mut expected = true;
    for guard in numeric
        .guards
        .chunks_exact(2)
        .take(numeric.guard_count as usize)
    {
        poll(cancellation)?;
        let ordering = match value {
            AggregateGpuValue::Integer(value) => value.cmp(&signed(guard[1])),
            AggregateGpuValue::Infimum => Ordering::Less,
            AggregateGpuValue::Supremum => Ordering::Greater,
        };
        expected &= match guard[0] {
            0 => ordering.is_eq(),
            1 => !ordering.is_eq(),
            2 => ordering.is_lt(),
            3 => !ordering.is_gt(),
            4 => ordering.is_gt(),
            5 => !ordering.is_lt(),
            _ => return Err(malformed()),
        };
    }
    if (holds == 1) != expected {
        return Err(malformed());
    }
    Ok(AggregateGpuEvaluation {
        value,
        holds: expected,
    })
}

fn function(function: Function) -> u32 {
    match function {
        Function::Count => 0,
        Function::Sum => 1,
        Function::SumPlus => 2,
        Function::Min => 3,
        Function::Max => 4,
    }
}
fn malformed() -> GpuError {
    GpuError::new(
        GpuErrorKind::Readback,
        "aggregate result shape, identity, carrier or guard mismatch",
    )
}
fn bytes(count: usize) -> Result<u64, GpuError> {
    u64::try_from(count)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| capacity("aggregate byte count overflow"))
}
fn add(values: &[u64]) -> Result<u64, GpuError> {
    values
        .iter()
        .try_fold(0_u64, |sum, value| sum.checked_add(*value))
        .ok_or_else(|| capacity("aggregate byte/work sum overflow"))
}
