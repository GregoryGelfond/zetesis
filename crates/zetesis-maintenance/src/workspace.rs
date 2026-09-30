//! The workspace's packages and binaries, as `cargo metadata` reports them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::Error;

/// A workspace package as Cargo reports it.
pub(crate) struct Package {
    /// The name of the directory holding its manifest.
    pub(crate) directory: String,
    /// The names of its binary targets.
    pub(crate) binaries: BTreeSet<String>,
}

/// Every package in `metadata`, the output of `cargo metadata --no-deps
/// --format-version 1`, by name.
pub(crate) fn packages(metadata: &[u8]) -> Result<BTreeMap<String, Package>, Error> {
    let metadata: serde_json::Value = serde_json::from_slice(metadata).map_err(Error::Json)?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| Error::Invalid("Cargo metadata lists no packages".into()))?;
    let mut workspace = BTreeMap::new();
    for package in packages {
        let (Some(name), Some(manifest)) =
            (package["name"].as_str(), package["manifest_path"].as_str())
        else {
            return Err(Error::Invalid(
                "a Cargo metadata package lacks its name or manifest".into(),
            ));
        };
        let directory = Path::new(manifest)
            .parent()
            .and_then(Path::file_name)
            .and_then(|directory| directory.to_str())
            .ok_or_else(|| Error::Invalid(format!("{name}'s manifest has no directory")))?;
        let binaries = package["targets"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
            })
            .filter_map(|target| target["name"].as_str().map(str::to_owned))
            .collect();
        workspace.insert(
            name.to_owned(),
            Package {
                directory: directory.to_owned(),
                binaries,
            },
        );
    }
    Ok(workspace)
}
