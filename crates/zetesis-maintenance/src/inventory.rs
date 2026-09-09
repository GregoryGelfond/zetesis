//! Source identity inventories used by external physical qualification.
pub use crate::files::Limits;
use crate::{
    Error,
    files::{self, Tree},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Hash the declared Rust/WGSL workspace source boundary, excluding build output.
/// The inventory covers root Cargo files/toolchain and recursive crate Rust,
/// WGSL and Cargo manifests. It does not seal tools, dependencies or executables.
/// # Errors
/// Refuses escaped/nonregular source paths, read/traversal limits and I/O errors.
pub fn sources(root: &Path, limits: Limits) -> Result<BTreeMap<String, String>, Error> {
    let mut tree = Tree::new(root, limits)?;
    let mut names: BTreeSet<String> = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .map(str::to_owned)
        .into();
    names.extend(tree.inventory("crates", "")?.into_iter().filter(|name| {
        name.ends_with(".rs") || name.ends_with(".wgsl") || name.ends_with("/Cargo.toml")
    }));
    names
        .into_iter()
        .map(|name| {
            let digest = files::digest(&tree.read(&name)?);
            Ok((name, digest))
        })
        .collect()
}
/// Read a small policy/record input through the same bounded file boundary.
/// # Errors
/// Refuses nonregular files, unavailable paths and inputs above `limit` bytes.
pub fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let canonical = path
        .canonicalize()
        .map_err(|error| files::io(path, error))?;
    let parent = canonical
        .parent()
        .ok_or_else(|| Error::Invalid("input has no parent".into()))?;
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::Invalid("non-UTF-8 input name".into()))?;
    Tree::new(
        parent,
        Limits {
            file_bytes: limit,
            total_bytes: limit,
            ..Limits::default()
        },
    )?
    .read(name)
}
