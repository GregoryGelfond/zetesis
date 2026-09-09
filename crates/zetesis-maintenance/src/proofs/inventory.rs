//! Observed source declarations and deterministic views, without assurance flags.
use super::{
    FIXED, Limits,
    source::{self, Declaration},
};
use crate::{
    Error,
    files::{self, Tree},
    require,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// An observed proof source inventory, not a proof-verification result.
/// Files must remain stable while read; this value is not a filesystem snapshot.
#[derive(Debug, Serialize)]
pub struct Inventory {
    modules: BTreeSet<String>,
    declarations: Vec<Declaration>,
    source_sha256: BTreeMap<String, String>,
}
impl Inventory {
    /// Semantic module paths in deterministic lexicographic order.
    #[must_use]
    pub fn modules(&self) -> &BTreeSet<String> {
        &self.modules
    }
    /// Declarations in module order, then source-line order.
    #[must_use]
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }
    /// Hashes of observed semantic modules and fixed source/configuration files.
    /// These describe current bytes, before any rendered view is published.
    #[must_use]
    pub fn source_sha256(&self) -> &BTreeMap<String, String> {
        &self.source_sha256
    }
    /// Render the exact declaration index, without writing it or certifying proofs.
    /// # Errors
    /// Refuses a serialization error.
    pub fn theorem_index(&self) -> Result<Vec<u8>, Error> {
        let mut output = serde_json::to_vec_pretty(&self.declarations).map_err(Error::Json)?;
        output.push(b'\n');
        Ok(output)
    }
    /// Render an umbrella import and one axiom request per observed theorem.
    /// These requests must subsequently be executed by the pinned Lean kernel.
    #[must_use]
    pub fn audit_source(&self) -> String {
        let mut output = String::from("import Zetesis\n\n");
        for declaration in &self.declarations {
            output.push_str("#print axioms ");
            output.push_str(declaration.name());
            output.push('\n');
        }
        output
    }
}
/// Inspect proof sources and produce deterministic index/audit views.
///
/// The restricted scanner is shared with record verification. Every source is
/// read within the supplied limits; symbolic links in the semantic inventory are
/// refused. Retained bytes and ordered indexes bound work and storage. No command
/// runs, file is written, record status is invented or kernel acceptance inferred.
///
/// # Errors
/// Refuses unsupported declarations, duplicate qualified names, empty inventory,
/// missing fixed source files, exceeded limits and filesystem failures.
pub fn inventory(root: &Path, limits: Limits) -> Result<Inventory, Error> {
    let mut tree = Tree::new(root, limits)?;
    let modules = tree.inventory("Zetesis", ".lean")?;
    require(!modules.is_empty(), "missing semantic modules")?;
    let mut declarations = Vec::new();
    let mut source_sha256 = BTreeMap::new();
    for module in &modules {
        let bytes = tree.read(module)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::Invalid(format!("invalid UTF-8: {module}")))?;
        declarations.extend(source::declarations(module, text)?);
        source_sha256.insert(module.clone(), files::digest(&bytes));
    }
    let names: BTreeSet<_> = declarations.iter().map(Declaration::name).collect();
    require(!declarations.is_empty(), "empty source theorem inventory")?;
    require(
        names.len() == declarations.len(),
        "duplicate qualified source theorem",
    )?;
    for file in FIXED {
        source_sha256.insert(file.into(), files::digest(&tree.read(file)?));
    }
    Ok(Inventory {
        modules,
        declarations,
        source_sha256,
    })
}
