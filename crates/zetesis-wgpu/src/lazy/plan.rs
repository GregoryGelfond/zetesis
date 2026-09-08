//! Checked active shapes and retained transport payloads.

use zetesis_cpu::lazy;

use super::{DIMENSION_WORDS, RESULT_METADATA_WORDS, UNIFORM_BYTES};
use crate::{GpuError, GpuErrorKind, GpuLimits};

pub(super) struct Plan {
    pub(super) dimensions: [u32; DIMENSION_WORDS],
    pub(super) result_words: usize,
    pub(super) result_bytes: u64,
    pub(super) uploaded_bytes: u64,
    pub(super) epoch: u32,
    pub(super) capacity: Capacity,
}

/// Requested buffer lengths, independent of the current logical dimensions.
/// Reuse never grants meaning to an inactive tail or preserves membership truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Capacity {
    pub(super) inputs: [u64; 4],
    pub(super) result: u64,
}

/// One capacity decision. Reasons describe the previous retained allocation;
/// several independent conditions may prevent its reuse on the same submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Transition {
    Initial,
    Reuse,
    Replace(Replacement),
}

impl Transition {
    pub(super) const fn is_reuse(self) -> bool {
        matches!(self, Self::Reuse)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Replacement {
    pub(super) growth: [bool; 4],
    pub(super) result_shape: bool,
    pub(super) allowance: Allowance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Allowance {
    Fits,
    Exceeded,
    Overflow,
}

/// Complete decision before allocation: retained inputs never enlarge the
/// active prefix, and retained results always have the exact active shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    pub(super) capacity: Capacity,
    pub(super) transition: Transition,
    pub(super) retention: Retention,
    pub(super) slack: Allowance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Retention {
    pub(super) uniform: bool,
    pub(super) inputs: [bool; 4],
    pub(super) result: bool,
}

impl Selection {
    /// `Plan::new` already admitted the exact shape. Prefer all independently
    /// fitting inputs; if their slack cannot fit, release slack and return to
    /// that admitted shape. Exact-fit buffers remain useful in either case.
    pub(super) fn new(previous: Option<Capacity>, plan: &Plan, maximum: u64) -> Self {
        let Some(previous) = previous else {
            return Self {
                capacity: plan.capacity,
                transition: Transition::Initial,
                retention: Retention {
                    uniform: false,
                    inputs: [false; 4],
                    result: false,
                },
                slack: Allowance::Fits,
            };
        };
        let prospective = Capacity {
            inputs: std::array::from_fn(|index| {
                previous.inputs[index].max(plan.capacity.inputs[index])
            }),
            result: plan.result_bytes,
        };
        let slack = prospective.allowance(plan, maximum);
        let capacity = if slack == Allowance::Fits {
            prospective
        } else {
            plan.capacity
        };
        Self {
            capacity,
            transition: previous.assess(plan, maximum),
            retention: Retention {
                uniform: true,
                inputs: std::array::from_fn(|index| {
                    previous.inputs[index] == capacity.inputs[index]
                }),
                result: previous.result == capacity.result,
            },
            slack,
        }
    }
}

impl Capacity {
    /// Uniform, four input buffers, output and readback; excludes driver-owned
    /// staging/retirement and allocator bookkeeping as specified by `GpuLimits`.
    pub(super) fn bytes(self) -> Option<u64> {
        self.inputs
            .iter()
            .try_fold(UNIFORM_BYTES, |sum, size| sum.checked_add(*size))?
            .checked_add(self.result.checked_mul(2)?)
    }

    /// Retained GPU capacity plus this chunk's host packing/decode allowance.
    /// A fresh exact shape has the same ceiling as the previous transient path.
    pub(super) fn accounted(self, plan: &Plan) -> Option<u64> {
        self.bytes()?
            .checked_add(plan.uploaded_bytes)?
            .checked_add(plan.result_bytes)
    }

    pub(super) fn assess(self, plan: &Plan, maximum: u64) -> Transition {
        let causes = Replacement {
            growth: std::array::from_fn(|index| self.inputs[index] < plan.capacity.inputs[index]),
            result_shape: self.result != plan.result_bytes,
            allowance: self.allowance(plan, maximum),
        };
        if causes.growth.iter().any(|grew| *grew)
            || causes.result_shape
            || causes.allowance != Allowance::Fits
        {
            Transition::Replace(causes)
        } else {
            Transition::Reuse
        }
    }

    fn allowance(self, plan: &Plan, maximum: u64) -> Allowance {
        match self.accounted(plan) {
            Some(bytes) if bytes <= maximum => Allowance::Fits,
            Some(_) => Allowance::Exceeded,
            None => Allowance::Overflow,
        }
    }
}

impl Plan {
    pub(super) fn new(
        chunk: &lazy::Chunk<'_>,
        limits: GpuLimits,
        device: &wgpu::Limits,
    ) -> Result<Self, GpuError> {
        let capacity = || {
            GpuError::new(
                GpuErrorKind::Capacity,
                "lazy chunk exceeds checked host/device dimensions",
            )
        };
        let dimensions = [
            chunk.words(),
            chunk.offsets().len(),
            chunk.worlds(),
            chunk.catalog_atoms(),
        ]
        .map(u32::try_from);
        let [words, rules, candidates, atoms] = dimensions;
        let dimensions = [
            words.map_err(|_| capacity())?,
            rules.map_err(|_| capacity())?,
            candidates.map_err(|_| capacity())?,
            atoms.map_err(|_| capacity())?,
        ];
        u32::try_from(chunk.records().len()).map_err(|_| capacity())?;
        dimensions[1]
            .checked_add(crate::WORKGROUP_SIZE)
            .ok_or_else(capacity)?;
        if chunk.worlds() > limits.max_candidates
            || dimensions[2] > device.max_compute_workgroups_per_dimension
        {
            return Err(capacity());
        }
        let result_words = chunk
            .words()
            .checked_add(RESULT_METADATA_WORDS)
            .and_then(|n| n.checked_mul(chunk.worlds()))
            .ok_or_else(capacity)?;
        u32::try_from(result_words).map_err(|_| capacity())?;
        let bytes = |count: usize| {
            u64::try_from(count)
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(capacity)
        };
        let result_bytes = bytes(result_words)?;
        let sizes = [
            bytes(chunk.offsets().len())?,
            bytes(chunk.records().len())?,
            bytes(chunk.snapshots().len())?,
            bytes(chunk.seeds().len())?,
            result_bytes,
        ];
        for size in sizes {
            if size > device.max_storage_buffer_binding_size || size > device.max_buffer_size {
                return Err(capacity());
            }
        }
        if UNIFORM_BYTES > device.max_uniform_buffer_binding_size
            || UNIFORM_BYTES > device.max_buffer_size
        {
            return Err(capacity());
        }
        if limits.timeout.is_zero() {
            return Err(capacity());
        }
        let uploaded_bytes = sizes[..4]
            .iter()
            .try_fold(UNIFORM_BYTES, |sum, size| sum.checked_add(*size))
            .ok_or_else(capacity)?;
        let storage = Capacity {
            inputs: [sizes[0], sizes[1], sizes[2], sizes[3]],
            result: result_bytes,
        };
        let plan = Self {
            dimensions,
            result_words,
            result_bytes,
            uploaded_bytes,
            epoch: 1,
            capacity: storage,
        };
        let accounted = storage.accounted(&plan).ok_or_else(capacity)?;
        if accounted > limits.max_batch_bytes {
            return Err(capacity());
        }
        Ok(plan)
    }

    pub(super) fn decode(&self, words: &[u32]) -> Result<Vec<u32>, GpuError> {
        let malformed = || {
            GpuError::new(
                GpuErrorKind::Readback,
                "lazy delta shape or submission/world identity mismatch",
            )
        };
        if words.len() != self.result_words {
            return Err(malformed());
        }
        let width = self.dimensions[0] as usize;
        let mut decoded = Vec::new();
        let decoded_words = (width + 1) * self.dimensions[2] as usize;
        decoded
            .try_reserve_exact(decoded_words)
            .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
        for (world, record) in words
            .chunks_exact(width + RESULT_METADATA_WORDS)
            .enumerate()
        {
            if record[width + 1] as usize != world || record[width + 2] != self.epoch {
                return Err(malformed());
            }
            decoded.extend_from_slice(&record[..=width]);
        }
        Ok(decoded)
    }
}
