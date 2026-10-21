//! Physical kernel choices, independent of the theory and its wire representation.

use crate::{GpuError, GpuErrorKind};
use std::borrow::Cow;

/// Invocations cooperating on one formula candidate, compiled into a kernel.
///
/// The size changes which invocation performs which iteration and the stride
/// every candidate-local loop takes, which equals the size; it does not change
/// candidate ownership or answer sets. Under frozen-reduct propagation sibling
/// gates of a level interact through atomic domain narrowing, so the number of
/// sweeps to quiescence, and with it whether a candidate reaches a sweep or work
/// limit, can differ between sizes; a residual is completed exactly on the host.
/// A larger size is not promised to be faster; compare complete solves on the
/// intended device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuWorkgroupSize {
    /// Thirty-two cooperating invocations.
    Threads32,
    /// Sixty-four cooperating invocations.
    #[default]
    Threads64,
    /// One hundred twenty-eight cooperating invocations.
    Threads128,
    /// Two hundred fifty-six cooperating invocations, the most a WebGPU device
    /// must grant per workgroup.
    Threads256,
}

impl GpuWorkgroupSize {
    /// Every size the formula and tight kernels compile, in ascending order.
    /// Its largest sets the address headroom every formula and tight packer
    /// reserves. The fitted sizing selects among its own ruled subset.
    pub const ALL: [Self; 4] = [
        Self::Threads32,
        Self::Threads64,
        Self::Threads128,
        Self::Threads256,
    ];

    /// Number of invocations; constant time and allocation-free.
    #[must_use]
    pub const fn invocations(self) -> u32 {
        match self {
            Self::Threads32 => 32,
            Self::Threads64 => 64,
            Self::Threads128 => 128,
            Self::Threads256 => 256,
        }
    }
}

/// How a formula or tight kernel chooses its workgroup size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuWorkgroupSizing {
    /// For each retained theory, the size its dependency-level schedule fits:
    /// among the fitted sizes 64, 128 and 256 that the device grants, those
    /// whose invocation slots over the schedule stay within twice the
    /// 64-invocation slots, the smallest with the least strided depth. A
    /// serial schedule, or one whose levels each hold at most 64 nodes, keeps
    /// 64; where 128 is granted, a widest level of 65 to 128 nodes selects it.
    /// Batch statistics report the size dispatched.
    ///
    /// Every granted fitted size is compiled when the kernel is created. A
    /// failure to compile any of them fails construction and, like every
    /// compilation failure, invalidates the context it was compiled on, a
    /// shared context included; [`Self::Fixed`] compiles one size.
    #[default]
    Fitted,
    /// One size for every theory, checked against the granted limits before
    /// the kernel compiles it, the only size compiled.
    Fixed(GpuWorkgroupSize),
}

impl GpuWorkgroupSizing {
    /// The fixed size, or `None` when each retained theory selects its own.
    #[must_use]
    pub const fn fixed(self) -> Option<GpuWorkgroupSize> {
        match self {
            Self::Fitted => None,
            Self::Fixed(size) => Some(size),
        }
    }
}

/// The sizes a fitted kernel may select, in ascending order: the ruled set,
/// owned apart from [`GpuWorkgroupSize::ALL`] so that a size compiled only for
/// fixed comparisons never joins the fitted rule.
const FITTED: [GpuWorkgroupSize; 3] = [
    GpuWorkgroupSize::Threads64,
    GpuWorkgroupSize::Threads128,
    GpuWorkgroupSize::Threads256,
];

/// The smallest size a fitted kernel selects, and the one it keeps for a
/// schedule that a larger size cannot shorten.
const FITTED_BASE: GpuWorkgroupSize = FITTED[0];

/// The size a kernel of `sizing` dispatches before a schedule selects one: the
/// fixed size, or the fitted base.
pub(crate) const fn base_size(sizing: GpuWorkgroupSizing) -> GpuWorkgroupSize {
    match sizing.fixed() {
        Some(size) => size,
        None => FITTED_BASE,
    }
}

/// The largest stride a formula or tight kernel loop adds after its last valid
/// index: the largest compiled workgroup size.
pub(crate) const MAX_FORMULA_STRIDE: u32 = largest_size();

/// Whether a formula or tight kernel loop bounded by `end`, one past its last
/// valid index, leaves [`MAX_FORMULA_STRIDE`] of headroom below `u32::MAX`, so
/// no strided index can wrap. Every packer address reservation applies it.
pub(crate) const fn fits_strided_loop(end: u32) -> bool {
    end <= u32::MAX - MAX_FORMULA_STRIDE
}

const fn largest_size() -> u32 {
    let mut largest = 0;
    let mut index = 0;
    while index < GpuWorkgroupSize::ALL.len() {
        let invocations = GpuWorkgroupSize::ALL[index].invocations();
        if invocations > largest {
            largest = invocations;
        }
        index += 1;
    }
    largest
}

/// The sizes `sizing` may dispatch on a device granting `limits`, in ascending
/// order: the fixed size, or each fitted size within both granted limits.
pub(crate) fn candidate_sizes(
    sizing: GpuWorkgroupSizing,
    limits: &wgpu::Limits,
) -> impl Iterator<Item = GpuWorkgroupSize> {
    let granted = limits
        .max_compute_invocations_per_workgroup
        .min(limits.max_compute_workgroup_size_x);
    let fitted = sizing.fixed().is_none();
    sizing.fixed().into_iter().chain(
        FITTED
            .into_iter()
            .filter(move |size| fitted && size.invocations() <= granted),
    )
}

/// The fitted size for a parallel schedule whose levels have the node counts
/// `widths`, among `candidates` (ascending, starting at the fitted base): with
/// `I(W)` the strided depth `sum(ceil(width / W))`, the smallest candidate
/// attaining the least `I(W)` among those with `W * I(W) <= 2 * 64 * I(64)`.
/// A serial schedule passes no levels and keeps the base. One pass over the
/// levels; exact integer arithmetic.
///
/// The selected size never lengthens the strided depth. Where a batch leaves
/// the device latency-bound, the dense level pass takes time following that
/// depth; where a batch oversubscribes the device, it follows invocation
/// slots, `W * I(W)`, which the bound keeps within twice the 64-invocation
/// slots. Since `128 * I(128) <= 128 * I(64)`, 128 always meets the bound: a
/// widest level of 65 to 128 nodes selects 128, and 256 needs a level of more
/// than 128 nodes and `I(256) <= I(64) / 2`.
fn fitted(widths: impl Iterator<Item = u32>, candidates: &[GpuWorkgroupSize]) -> GpuWorkgroupSize {
    let mut depths = [0_u64; FITTED.len()];
    for width in widths {
        for (depth, size) in depths.iter_mut().zip(candidates) {
            *depth += u64::from(width.div_ceil(size.invocations()));
        }
    }
    let base_slots = 2 * u64::from(FITTED_BASE.invocations()) * depths[0];
    let mut selected = FITTED_BASE;
    let mut least = depths[0];
    for (size, depth) in candidates.iter().zip(depths) {
        if depth < least && u64::from(size.invocations()) * depth <= base_slots {
            selected = *size;
            least = depth;
        }
    }
    selected
}

/// Gate scheduling for the general frozen-reduct primitive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
    /// How the workgroup size, the invocations cooperating on each candidate,
    /// is chosen: [`GpuWorkgroupSizing::Fitted`] by default.
    pub workgroup_sizing: GpuWorkgroupSizing,
    /// Use applicable, granted optional instructions; retain portable fallbacks.
    pub optional_features: bool,
    /// General frozen-reduct gate scheduling. Tight original-truth kernels
    /// accept only [`GpuPropagation::Dense`] because they do not propagate.
    pub propagation: GpuPropagation,
}

impl Default for GpuKernelOptions {
    fn default() -> Self {
        Self {
            workgroup_sizing: GpuWorkgroupSizing::default(),
            optional_features: true,
            propagation: GpuPropagation::Dense,
        }
    }
}

/// The size a formula or tight kernel dispatches for one retained schedule
/// whose levels have the node counts `widths`, on a device granting `limits`:
/// the fixed size, or the [`fitted`] size among the granted candidates.
pub(crate) fn dispatch_size(
    sizing: GpuWorkgroupSizing,
    widths: impl Iterator<Item = u32>,
    limits: &wgpu::Limits,
) -> GpuWorkgroupSize {
    if let Some(size) = sizing.fixed() {
        return size;
    }
    let mut candidates = [FITTED_BASE; FITTED.len()];
    let mut count = 0;
    for size in candidate_sizes(sizing, limits) {
        candidates[count] = size;
        count += 1;
    }
    fitted(widths, &candidates[..count])
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
            let size = base_size(options.workgroup_sizing).invocations();
            if size > limits.max_compute_invocations_per_workgroup
                || size > limits.max_compute_workgroup_size_x
            {
                return Err(GpuError::new(
                    GpuErrorKind::Capacity,
                    format!("formula workgroup size {size} exceeds granted device limits"),
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
