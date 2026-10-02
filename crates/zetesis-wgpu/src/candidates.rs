//! Owner-checked word export into already admitted candidate transport storage.

use crate::{GpuError, GpuErrorKind};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Theory};

/// Fill exactly one packed row per occurrence, preserving input order. The
/// caller owns reservation and byte admission; this operation allocates nothing.
/// Zero logical words retain the device ABI's one zero padding word. A stopped
/// prefix remains private to the caller and must never be submitted.
pub(crate) fn pack(
    theory: &Theory,
    candidates: &[Interpretation],
    output: &mut [u32],
    cancellation: &Cancellation,
) -> Result<(), GpuError> {
    pack_with(theory, candidates, output, || {
        cancellation.poll().map_err(GpuError::interrupted)
    })
}

fn pack_with(
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
