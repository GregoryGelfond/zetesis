//! Physical kernel choices, independent of the theory and its wire representation.

use crate::{GpuError, GpuErrorKind};
use std::borrow::Cow;

/// Invocations cooperating on one formula candidate, fixed at compilation.
///
/// The choice changes traversal strides, not candidate ownership or semantics.
/// Device limits are checked before compilation. A larger group is not promised
/// to be faster; compare complete solves on the intended device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuWorkgroupSize {
    /// Thirty-two cooperating invocations.
    Threads32,
    /// Sixty-four cooperating invocations; the established default.
    #[default]
    Threads64,
}

impl GpuWorkgroupSize {
    /// Number of invocations; constant time and allocation-free.
    #[must_use]
    pub const fn invocations(self) -> u32 {
        match self {
            Self::Threads32 => 32,
            Self::Threads64 => 64,
        }
    }
}

/// Gate scheduling for the general frozen-reduct primitive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuPropagation {
    /// Visit every gate each sweep; the established execution route.
    #[default]
    Dense,
    /// Visit active bits of gates affected by changed physical domains, without
    /// walking inactive nodes or dependency levels. Retain dense work charges
    /// plus a conservative directory/bitmap allowance. If optional
    /// storage cannot be admitted, use dense propagation on the same device.
    ChangedDependencies,
    /// Active traversal with retained AND/OR permission counts. If the optional
    /// counters cannot fit the complete memory/work envelope, retain scanned
    /// groups under [`Self::ChangedDependencies`], then its dense fallback.
    IncrementalGroups,
}

/// Physical choices shared by the original-truth and frozen-reduct kernels.
///
/// Optional instructions are used only when the retained context granted their
/// features. Turning them off selects the portable implementation on that same
/// device, allowing controlled comparisons without changing the logical input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuKernelOptions {
    /// Cooperating invocations per candidate.
    pub workgroup_size: GpuWorkgroupSize,
    /// Use applicable, granted optional instructions; retain portable fallbacks.
    pub optional_features: bool,
    /// General frozen-reduct gate scheduling. Tight original-truth kernels
    /// accept only [`GpuPropagation::Dense`] because they do not propagate.
    pub propagation: GpuPropagation,
}

impl Default for GpuKernelOptions {
    fn default() -> Self {
        Self {
            workgroup_size: GpuWorkgroupSize::default(),
            optional_features: true,
            propagation: GpuPropagation::Dense,
        }
    }
}

/// Only formula kernels have tunable candidate-local traversal strides.
#[derive(Clone, Copy, Default)]
pub(crate) enum Kernel {
    #[default]
    Fixed,
    Formula(GpuKernelOptions),
    Tight(GpuKernelOptions),
}

impl Kernel {
    pub(crate) fn options(self) -> Option<GpuKernelOptions> {
        match self {
            Self::Fixed => None,
            Self::Formula(options) | Self::Tight(options) => Some(options),
        }
    }

    pub(crate) fn validate(self, limits: &wgpu::Limits) -> Result<(), GpuError> {
        if matches!(self, Self::Tight(options) if options.propagation != GpuPropagation::Dense) {
            return Err(GpuError::new(
                GpuErrorKind::Validation,
                "changed-dependency propagation requires the general formula kernel",
            ));
        }
        if let Some(options) = self.options() {
            let width = options.workgroup_size.invocations();
            if width > limits.max_compute_invocations_per_workgroup
                || width > limits.max_compute_workgroup_size_x
            {
                return Err(GpuError::new(
                    GpuErrorKind::Capacity,
                    format!("formula workgroup width {width} exceeds granted device limits"),
                ));
            }
        }
        Ok(())
    }

    /// Select a bounded source fragment using granted features. Subgroup
    /// operations are confined to a workgroup-uniform reduction; individual
    /// formula rows may have divergent owners and cannot use that collective.
    pub(crate) fn source(
        self,
        source: Cow<'static, str>,
        features: wgpu::Features,
    ) -> Result<Cow<'static, str>, GpuError> {
        let Some(options) = self.options() else {
            return Ok(source);
        };
        if !options.optional_features {
            return Ok(source);
        }
        let mut source = source;
        if matches!(self, Self::Formula(_)) && features.contains(wgpu::Features::SUBGROUP) {
            let body = source
                .strip_suffix(include_str!("formula/subset.wgsl"))
                .ok_or_else(|| boundary("subset reduction"))?;
            source = assemble(&[body, include_str!("formula/subset_subgroup.wgsl")])?;
        }
        if features.contains(wgpu::Features::SHADER_INT64) {
            let (prefix, arithmetic) = source
                .split_once("fn wide_add(")
                .ok_or_else(|| boundary("wide addition"))?;
            let (_, suffix) = arithmetic
                .split_once("fn numeric_row(")
                .ok_or_else(|| boundary("numeric row"))?;
            source = assemble(&[
                prefix,
                include_str!("formula_graph/wide64.wgsl"),
                "fn numeric_row(",
                suffix,
            ])?;
        }
        Ok(source)
    }
}

fn boundary(name: &str) -> GpuError {
    GpuError::new(
        GpuErrorKind::Validation,
        format!("kernel {name} boundary is missing"),
    )
}

/// One checked reservation for fixed authored fragments; no input text is parsed.
fn assemble(parts: &[&str]) -> Result<Cow<'static, str>, GpuError> {
    let length = parts.iter().try_fold(0usize, |total, part| {
        total
            .checked_add(part.len())
            .ok_or_else(|| GpuError::new(GpuErrorKind::Capacity, "kernel source length overflow"))
    })?;
    let mut source = String::new();
    source
        .try_reserve_exact(length)
        .map_err(|error| GpuError::new(GpuErrorKind::Allocation, error.to_string()))?;
    for part in parts {
        source.push_str(part);
    }
    Ok(Cow::Owned(source))
}

#[cfg(test)]
mod tests;
