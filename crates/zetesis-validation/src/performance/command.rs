//! Reusable campaign selection for application commands.
//!
//! Command adapters resolve paths and map arguments to the existing typed plan.
//! This module owns maintained corpus and series expansion; measurement,
//! qualification, resource limits and publication remain the matrix contracts.

use super::{Error, corpus, matrix, series};

/// Run a matrix, expanding the maintained corpus or generated/constant series.
///
/// Limits are used exactly as supplied. A caller choosing the maintained series
/// can explicitly apply [`series::limits`] and [`series::native_answers`] before
/// constructing its request. This operation never silently raises a ceiling.
/// The returned report is not published; the caller chooses when to publish it.
/// A series case selection names workload entries, including generated paths;
/// each entry expands to all its cells in series order, with entries visited in
/// the caller's order. The report retains the original selection and each
/// expanded workload's identity.
///
/// # Errors
/// Returns corpus, workload preparation or campaign configuration failures.
pub fn run(
    request: &matrix::Request<'_>,
    invocation: matrix::NativeInvocation,
) -> Result<matrix::Report, Error> {
    run_with_cancellation(
        request,
        invocation,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

/// Select and run a campaign with explicit caller-owned cancellation.
///
/// Cancellation and partial work are retained by the matrix owner; no global
/// signal handlers are installed and the returned report remains unpublished.
///
/// # Errors
/// Returns the same preparation refusals as [`run`].
pub fn run_with_cancellation(
    request: &matrix::Request<'_>,
    invocation: matrix::NativeInvocation,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<matrix::Report, Error> {
    if request.plan.suite() == matrix::Suite::Corpus {
        let catalog =
            crate::examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
        let root = catalog.root().parent().ok_or(Error::Configuration(
            "the correctness catalog has no enclosing examples directory",
        ))?;
        let workloads = corpus::workloads(
            &catalog,
            root,
            matrix::WorkloadLimits {
                source_bytes: request.limits.corpus.source_bytes,
                total_source_bytes: request.limits.corpus.total_source_bytes,
                metadata_bytes: request.limits.corpus.manifest_bytes,
                amendments: 0,
            },
        )?;
        matrix::run_selected_workloads_with_cancellation(request, &workloads, invocation, cancelled)
    } else if request.plan.suite() == matrix::Suite::Series {
        let corpus =
            crate::examples::load(request.corpus, request.limits.corpus).map_err(Error::Corpus)?;
        let workloads = series::workloads(&corpus, matrix::WorkloadLimits::default())?;
        matrix::run_selected_workloads_with_cancellation(request, &workloads, invocation, cancelled)
    } else {
        matrix::run_with_cancellation(request, invocation, cancelled)
    }
}
