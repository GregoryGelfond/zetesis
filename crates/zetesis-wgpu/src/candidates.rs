//! Owner-checked word export into already admitted candidate transport storage.

use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory};

/// One oracle's initialized candidate upload storage. It carries no theory
/// ownership: the oracle releases it before changing subject or batch shape.
/// A failed export clears its length, so no incomplete prefix is dispatchable.
#[derive(Default)]
pub(crate) struct Staging {
    words: Vec<u32>,
}

impl Staging {
    pub(crate) fn release(&mut self) {
        self.words = Vec::new();
    }

    pub(crate) fn words(&self) -> &[u32] {
        &self.words
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.words.capacity()
    }

    /// Replace the plan's requested host seed bytes with actual capacity. The
    /// device seed allocation remains the requested size. An optional oversized
    /// buffer is dropped before a smaller replacement, never retained alongside it.
    pub(crate) fn pack(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        requested: u64,
        accounted: u64,
        ceiling: u64,
        cancellation: &Cancellation,
    ) -> Result<u64, GpuError> {
        self.pack_with(theory, candidates, requested, accounted, ceiling, || {
            cancellation.poll().map_err(GpuError::interrupted)
        })
    }

    fn pack_with(
        &mut self,
        theory: &Theory,
        candidates: &[Interpretation],
        requested: u64,
        accounted: u64,
        ceiling: u64,
        mut poll: impl FnMut() -> Result<(), GpuError>,
    ) -> Result<u64, GpuError> {
        let result = (|| {
            poll()?;
            let actual = self.reserve_with(requested, accounted, ceiling, reserve)?;
            pack(theory, candidates, &mut self.words, poll)?;
            Ok(actual)
        })();
        if result.is_err() {
            self.words.clear();
        }
        result
    }

    fn reserve_with(
        &mut self,
        requested: u64,
        accounted: u64,
        ceiling: u64,
        allocate: impl FnOnce(usize) -> Result<Vec<u32>, GpuError>,
    ) -> Result<u64, GpuError> {
        let base = accounted
            .checked_sub(requested)
            .ok_or_else(|| capacity("candidate host allowance exceeds its batch charge"))?;
        if requested == 0 || !requested.is_multiple_of(4) || accounted > ceiling {
            return Err(capacity(
                "candidate staging exceeds its complete byte allowance",
            ));
        }
        let length = usize::try_from(requested / 4)
            .map_err(|_| capacity("candidate staging exceeds host address space"))?;
        let fits = |words| capacity_charge(base, words).is_ok_and(|charge| charge <= ceiling);
        if self.words.capacity() < length || !fits(self.words.capacity()) {
            self.release();
            self.words = allocate(length)?;
        }
        let charge = capacity_charge(base, self.words.capacity());
        if self.words.capacity() < length || !charge.as_ref().is_ok_and(|charge| *charge <= ceiling)
        {
            self.release();
            return Err(capacity(
                "candidate staging capacity exceeds its byte allowance",
            ));
        }
        // Equal-shape reuse performs no zero filling. The exporter overwrites
        // every logical word; only fresh or previously interrupted storage grows.
        self.words.resize(length, 0);
        charge
    }
}

fn reserve(length: usize) -> Result<Vec<u32>, GpuError> {
    let mut words = Vec::new();
    words.try_reserve_exact(length).map_err(|_| {
        GpuError::new(
            GpuErrorKind::Allocation,
            "candidate host reservation failed",
        )
    })?;
    Ok(words)
}

fn capacity_charge(base: u64, words: usize) -> Result<u64, GpuError> {
    u64::try_from(words)
        .ok()
        .and_then(|words| words.checked_mul(4))
        .and_then(|bytes| base.checked_add(bytes))
        .ok_or_else(|| capacity("candidate staging capacity sum overflows"))
}

fn capacity(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Capacity, detail)
}

/// Fill exactly one packed row per occurrence, preserving input order. The
/// caller owns reservation and byte admission; this operation allocates nothing.
/// Zero logical words retain the device ABI's one zero padding word. A stopped
/// prefix remains private to the caller and must never be submitted.
fn pack(
    theory: &Theory,
    candidates: &[Interpretation],
    output: &mut [u32],
    mut poll: impl FnMut() -> Result<(), GpuError>,
) -> Result<(), GpuError> {
    poll()?;
    let width = theory.atom_count().div_ceil(32);
    let length = width
        .checked_mul(candidates.len())
        .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "candidate word count overflows"))?;
    if output.len() != length.max(1) {
        return Err(GpuError::new(
            GpuErrorKind::Capacity,
            "candidate storage differs from its complete word shape",
        ));
    }
    for (world, candidate) in candidates.iter().enumerate() {
        poll()?;
        let words = candidate.words32();
        if !theory.same_instance(words.theory()) {
            return Err(GpuError::new(
                GpuErrorKind::Seed,
                "candidate belongs to another Theory",
            ));
        }
        for (index, word) in words.enumerate() {
            poll()?;
            output[world * width + index] = word;
        }
    }
    if length == 0 {
        output[0] = 0;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
