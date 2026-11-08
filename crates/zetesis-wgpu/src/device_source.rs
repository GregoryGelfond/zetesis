//! Where a GPU primitive obtains its device.

use crate::{GpuContext, GpuDiscovery, GpuOptions, GpuSelection};

/// Where a GPU primitive obtains its device: a selection of its own, the same
/// selection over an adapter enumeration a [`GpuDiscovery`] began earlier, or
/// an existing context.
///
/// A selected or discovered source chooses, among the adapters of the API that
/// the policy admits, the best-ranked one advertising compute shaders and
/// limits the constructing primitive accepts, then requests a device of its
/// own from that adapter. A discovered source selects over the adapters present
/// when its discovery began, so it chooses the adapter and reports the failures
/// a selected source would have then, at the same steps; [`GpuDiscovery`]
/// states what that snapshot's age means. A discovery saves only the adapter
/// enumeration's wait. A context source selects nothing: the primitive compiles
/// on that exact context, which keeps the timestamp request it was created with.
///
/// `timing` requests compute-pass timestamps for the primitives that measure
/// compute time, the formula and tight oracles; the other primitives request
/// none. Unsupported timestamps leave solving available and are reported by
/// [`GpuContext::compute_timing`].
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum GpuDeviceSource<'a> {
    /// Enumerate `selection`'s adapters, select one under `options` and
    /// request its device.
    Selected {
        /// The hard adapter policy.
        options: GpuOptions,
        /// The API to enumerate; never replaced by another.
        selection: GpuSelection,
        /// Request compute-pass timestamps.
        timing: bool,
    },
    /// Select under `options` as [`Self::Selected`] does for the discovery's
    /// API, over the adapter enumeration `discovery` began. A discovery serves one
    /// primitive: once taken, or when its thread failed, the primitive
    /// enumerates as [`Self::Selected`] does.
    Discovered {
        /// The adapter enumeration begun earlier.
        discovery: &'a GpuDiscovery,
        /// The hard adapter policy.
        options: GpuOptions,
        /// Request compute-pass timestamps.
        timing: bool,
    },
    /// Compile on this exact context: no selection and no device request.
    Context(&'a GpuContext),
}
