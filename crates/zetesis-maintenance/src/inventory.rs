//! Source identity inventories used by external physical qualification.
pub use crate::files::Limits;
use crate::{
    Error,
    files::{self, Tree},
    require,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Hash the declared Rust/WGSL workspace source boundary.
/// The inventory covers root Cargo files/toolchain and recursive crate Rust,
/// WGSL and Cargo manifests. The repository-level `target` directory is outside
/// this traversal; matching files anywhere beneath `crates` are included.
/// It does not seal tools, dependencies or executables.
/// # Errors
/// Refuses escaped/nonregular source paths, read/traversal limits and I/O errors.
pub fn sources(root: &Path, limits: Limits) -> Result<BTreeMap<String, String>, Error> {
    let mut tree = Tree::new(root, limits)?;
    let mut names: BTreeSet<String> = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .map(str::to_owned)
        .into();
    names.extend(tree.inventory("crates", "")?.into_iter().filter(|name| {
        let path = Path::new(name);
        matches!(
            path.extension().and_then(std::ffi::OsStr::to_str),
            Some("rs" | "wgsl")
        ) || path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml"))
    }));
    names
        .into_iter()
        .map(|name| {
            let digest = files::digest(&tree.read(&name)?);
            Ok((name, digest))
        })
        .collect()
}
/// The maintained standalone packages, which the workspace does not build.
const STANDALONE: [&str; 2] = ["validation/reference", "refinement/membership/rust"];
/// Every maintained Rust source, relative to `root`, in order: the `src`,
/// `tests`, `benches` and `examples` trees and the `build.rs` of each workspace
/// package under `crates` and of each maintained standalone package, and the
/// manual's shared examples. Build outputs, vendored imports and historical
/// evidence lie outside these roots; a directory named `target` inside one is
/// authored source.
/// # Errors
/// Refuses a package without a manifest, symbolic links, more than
/// `limits.entries` directory entries in one tree, and I/O errors.
pub fn authored(root: &Path, limits: Limits) -> Result<Vec<String>, Error> {
    let tree = Tree::new(root, limits)?;
    let mut packages = tree.directories("crates")?;
    packages.extend(STANDALONE.map(str::to_owned));
    let mut sources = BTreeSet::new();
    for package in &packages {
        let manifest = format!("{package}/Cargo.toml");
        require(
            tree.exists(&manifest)?,
            format!("missing maintained package manifest: {package}"),
        )?;
        tree.member(&manifest)?;
        for directory in ["src", "tests", "benches", "examples"] {
            let directory = format!("{package}/{directory}");
            if tree.exists(&directory)? {
                sources.extend(tree.inventory(&directory, ".rs")?);
            }
        }
        let build = format!("{package}/build.rs");
        if tree.exists(&build)? {
            tree.member(&build)?;
            sources.insert(build);
        }
    }
    sources.extend(tree.inventory("docs/book/examples", ".rs")?);
    Ok(sources.into_iter().collect())
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
