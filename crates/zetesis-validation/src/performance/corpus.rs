//! Maintained benchmark inputs with their independent source provenance.
//!
//! The original correctness catalog stays byte-identical. Additional workloads
//! join the same matrix through their sealed source bundles and typed contracts.

use std::path::Path;

use super::{
    Error, bundles,
    matrix::{Workload, WorkloadLimits},
};
use crate::examples;

const FAMILY_MANIFEST_SHA256: &str =
    "92a39c82c8bebb5f17323db9e61084f2b713628b574690bcc0312147395d3f26";

/// Assemble the original 94 cases and integrated Family Crossing in that order.
///
/// `root` is the examples directory containing both `correctness` and
/// `family-crossing`. All source bytes and include spellings stay unchanged.
/// Family Crossing requires fresh reference qualification, as other bundled
/// workloads do. Limits bound preparation; no solver is launched here.
///
/// # Errors
/// Refuses missing or changed sources, invalid contracts and preparation limits.
pub fn workloads(
    catalog: &examples::Corpus,
    root: &Path,
    limits: WorkloadLimits,
) -> Result<Vec<Workload>, Error> {
    let manifest = root.join("family-crossing/manifest.json");
    let mut workloads = catalog
        .cases()
        .iter()
        .map(|case| Workload::original(catalog, case.path(), limits))
        .collect::<Result<Vec<_>, _>>()?;
    workloads.extend(bundles::load_reviewed(
        &manifest,
        limits,
        FAMILY_MANIFEST_SHA256,
    )?);
    Ok(workloads)
}
