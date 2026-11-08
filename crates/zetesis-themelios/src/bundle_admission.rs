//! Source-graph admission limits and checked include identities.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use themelios_base::diagnostic::Diagnostic;
use themelios_base::span::Location;
use zetesis_core::AdmissionLimits;

use crate::{AdmissionFailure, AdmissionOptions, ExpansionFailure, SourceBundle};

/// Admission budgets across an already bounded, loaded source graph. File,
/// byte, and include-depth limits belong to [`crate::BundleLimits`] at loading.
#[derive(Clone, Copy, Debug)]
pub struct BundleAdmissionOptions {
    /// Total syntax nodes visited across all unique original files.
    pub max_syntax_nodes: usize,
    /// Maximum syntax depth within any original file.
    pub max_syntax_depth: usize,
    /// Maximum original body elements in each rule, before deduplication.
    pub max_body_elements: usize,
    /// Formula template, variable, predicate-arity and value ceilings.
    pub core_limits: AdmissionLimits,
}

impl Default for BundleAdmissionOptions {
    fn default() -> Self {
        let single = AdmissionOptions::default();
        Self {
            max_syntax_nodes: single.max_syntax_nodes,
            max_syntax_depth: single.max_syntax_depth,
            max_body_elements: single.max_body_elements,
            core_limits: single.core_limits,
        }
    }
}

/// A typed semantic bundle refusal, separate from source-graph loading errors.
#[derive(Debug)]
pub enum BundleAdmissionError {
    /// Source normalization or expansion refused its stage.
    Expansion(ExpansionFailure),
    /// A later explicit root has a different lexical identity from the same
    /// source's first root or include occurrence.
    RootAlias {
        /// First selected lexical filename.
        first: PathBuf,
        /// Later original root spelling.
        repeated: PathBuf,
        /// Original source span of the repeated root.
        location: Location,
    },
    /// Different lexical include paths refer to the same canonical source.
    /// clingo can parse these separately, which the deduplicated graph does not
    /// represent; admission refuses instead of dropping repeated definitions.
    IncludeAlias {
        /// First resolved lexical path in include traversal order.
        first: PathBuf,
        /// Later resolved lexical path to the same canonical source.
        repeated: PathBuf,
        /// Original include occurrence that revisits the source.
        location: Location,
    },
    /// A lexical path's dot-normalized identity differs from the canonical
    /// source, for example through a symlink. Relative include semantics under
    /// that spelling are not represented by the canonical graph.
    IncludeRedirection {
        /// Original include path resolved from its including lexical path.
        requested: PathBuf,
        /// Canonical source identity retained by the loader.
        canonical: PathBuf,
        /// Original include occurrence.
        location: Location,
    },
}

impl From<ExpansionFailure> for BundleAdmissionError {
    fn from(error: ExpansionFailure) -> Self {
        Self::Expansion(error)
    }
}
impl From<AdmissionFailure> for BundleAdmissionError {
    fn from(error: AdmissionFailure) -> Self {
        Self::Expansion(error.into())
    }
}

impl BundleAdmissionError {
    /// Located diagnostics whose identities resolve in the retained bundle.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Expansion(error) => error.diagnostics(),
            Self::RootAlias { location, .. }
            | Self::IncludeAlias { location, .. }
            | Self::IncludeRedirection { location, .. } => {
                vec![crate::diagnostic::diagnostic(
                    "include-identity",
                    self.to_string(),
                    *location,
                )]
            }
        }
    }
}

impl fmt::Display for BundleAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expansion(error) => error.fmt(f),
            Self::RootAlias {
                first, repeated, ..
            } => write!(
                f,
                "bundle admission cannot merge lexical root aliases {} and {}",
                first.display(),
                repeated.display()
            ),
            Self::IncludeAlias {
                first, repeated, ..
            } => write!(
                f,
                "bundle admission cannot merge lexical include aliases {} and {}",
                first.display(),
                repeated.display()
            ),
            Self::IncludeRedirection {
                requested,
                canonical,
                ..
            } => write!(
                f,
                "bundle admission cannot redirect lexical include {} to {}",
                requested.display(),
                canonical.display()
            ),
        }
    }
}
impl std::error::Error for BundleAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Expansion(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn check_include_identity(bundle: &SourceBundle) -> Result<(), BundleAdmissionError> {
    let mut paths = BTreeMap::new();
    for root in bundle.roots() {
        if let Some(first) = paths.get(&root.id()) {
            let first: &PathBuf = first;
            if first.as_os_str() != root.requested_path().as_os_str() {
                let source = bundle.get(root.id()).expect("a loaded root has a source");
                return Err(BundleAdmissionError::RootAlias {
                    first: first.clone(),
                    repeated: root.requested_path().to_path_buf(),
                    location: Location {
                        source: source.id(),
                        span: source.source().span(),
                    },
                });
            }
            continue;
        }
        paths.insert(root.id(), root.requested_path().to_path_buf());
        let mut pending = vec![(root.id(), 0)];
        while let Some((id, next)) = pending.last_mut() {
            let source = bundle.get(*id).expect("a loaded graph has every source");
            let Some(edge) = source.includes().get(*next) else {
                pending.pop();
                continue;
            };
            *next += 1;
            let requested = edge.resolved_path();
            if let Some(first) = paths.get(&edge.target()) {
                if first.as_os_str() != requested.as_os_str() {
                    return Err(BundleAdmissionError::IncludeAlias {
                        first: first.clone(),
                        repeated: requested.to_path_buf(),
                        location: edge.location(),
                    });
                }
                continue;
            }
            let target = bundle
                .get(edge.target())
                .expect("a loaded include has its target");
            let checked = match edge.resolution() {
                crate::IncludeResolution::IncludingDirectory => source
                    .path()
                    .parent()
                    .expect("a canonical source has a parent")
                    .join(edge.requested_path()),
                crate::IncludeResolution::Absolute | crate::IncludeResolution::WorkingDirectory => {
                    bundle.working_directory().join(edge.requested_path())
                }
            };
            if dot_normalized(&checked) != target.path() {
                return Err(BundleAdmissionError::IncludeRedirection {
                    requested: requested.to_path_buf(),
                    canonical: target.path().to_path_buf(),
                    location: edge.location(),
                });
            }
            paths.insert(edge.target(), requested.to_path_buf());
            pending.push((edge.target(), 0));
        }
    }
    Ok(())
}

fn dot_normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}
