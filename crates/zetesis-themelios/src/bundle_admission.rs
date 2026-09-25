//! Semantic admission of canonical source graphs under the extended profile.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use themelios_base::diagnostic::Diagnostic;
use themelios_base::span::Location;
use themelios_program::program::{Program as SourceProgram, Statement};
use themelios_program::raise::raise;
use zetesis_core::{AdmissionLimits, Program};

use crate::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionUsage,
    InputLimit, SourceBundle, SourceMetadata, extended, metadata, profile,
};

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
    /// Independent limits on the compiled relational program.
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

/// An admitted relational program and its complete original source catalog.
/// Every template origin resolves through [`Self::bundle`].
#[derive(Debug)]
pub struct AdmittedBundle {
    program: Program,
    bundle: SourceBundle,
    template_origins: Vec<Vec<Location>>,
    metadata: SourceMetadata,
    expansion: ExpansionUsage,
}

impl AdmittedBundle {
    /// The independently checked core program.
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Original source bytes, identities, paths, and include occurrences.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }
    /// Original parsed rule locations in compiled template order. Definitions
    /// remain in the original source catalog; they are not claimed as rule spans.
    #[must_use]
    pub fn template_origins(&self) -> &[Vec<Location>] {
        &self.template_origins
    }
    /// Global source declarations and display policy, retaining every original
    /// metadata occurrence. The underlying program and model identity are full.
    #[must_use]
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }
    /// Expansion charges this admission accepted, each under its ceiling.
    #[must_use]
    pub fn expansion_usage(&self) -> &ExpansionUsage {
        &self.expansion
    }
    /// Consume the result when original source evidence is no longer required.
    #[must_use]
    pub fn into_program(self) -> Program {
        self.program
    }
}

/// A typed semantic bundle refusal, separate from source-graph loading errors.
#[derive(Debug)]
pub enum BundleAdmissionError {
    /// The extended source or independent core admission door refused its stage.
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

/// A failure retains the full loaded source catalog, so diagnostics never lose
/// the original bytes behind their source identities. No partial core program
/// is returned.
#[derive(Debug)]
pub struct BundleAdmissionFailure {
    bundle: SourceBundle,
    error: Box<BundleAdmissionError>,
}
impl BundleAdmissionFailure {
    /// Original files needed to render every diagnostic.
    #[must_use]
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }
    /// The typed semantic refusal.
    #[must_use]
    pub fn error(&self) -> &BundleAdmissionError {
        &self.error
    }
    /// Source-qualified diagnostic view.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.error.diagnostics()
    }
    /// Recover the loaded sources, for example to retry with different budgets.
    #[must_use]
    pub fn into_bundle(self) -> SourceBundle {
        self.bundle
    }
}
impl fmt::Display for BundleAdmissionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for BundleAdmissionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

/// Admit the extended profile across an original include graph, interpreting
/// its ordered roots and captured include resolutions. Rules remain in the
/// parameter-free base part, whether implicit or explicitly declared. Every file
/// is checked before raising; owned statements are combined
/// through themelios's provenance-merging constructor without concatenating or
/// regenerating text. Scalar constants are global, unambiguous, and acyclic.
///
/// Identical resolved lexical include paths are included once, as clingo does.
/// Alias paths and symlink redirections are conservatively refused because the
/// canonical loader cannot recover their separate parse occurrences. Ordinary
/// `..` traversal to a unique source is supported. Positive `#defined` and
/// signature/empty `#show` are global source/display metadata. Named or
/// parameterized `#program` parts and other unsupported constructs are refused.
/// No external engine is invoked.
///
/// # Errors
/// Returns a typed refusal together with the complete original source bundle.
/// Syntax/expansion budgets count globally; no successful partial program is
/// exposed after a refusal.
pub fn admit_bundle_extended(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    limits: ExpansionLimits,
) -> Result<AdmittedBundle, BundleAdmissionFailure> {
    match compile_bundle(&bundle, options, limits) {
        Ok(compiled) => Ok(AdmittedBundle {
            program: compiled.program,
            bundle,
            template_origins: compiled.template_origins,
            metadata: compiled.metadata,
            expansion: compiled.expansion,
        }),
        Err(error) => Err(BundleAdmissionFailure {
            bundle,
            error: Box::new(error),
        }),
    }
}

fn compile_bundle(
    bundle: &SourceBundle,
    options: BundleAdmissionOptions,
    limits: ExpansionLimits,
) -> Result<extended::Compilation, BundleAdmissionError> {
    check_include_identity(bundle)?;
    let mut definitions = BTreeMap::new();
    let mut statements = Vec::new();
    let mut visited_nodes = 0;
    let mut metadata_count = 0;
    let mut source_metadata = metadata::Builder::default();
    for source in bundle.sources() {
        let local = AdmissionOptions {
            source_id: source.id(),
            max_source_bytes: 0,
            max_syntax_nodes: options.max_syntax_nodes - visited_nodes,
            max_syntax_depth: options.max_syntax_depth,
            max_body_elements: options.max_body_elements,
            core_limits: options.core_limits,
        };
        let nodes = profile::check_bundle(source.parsed(), local).map_err(|error| match error {
            AdmissionFailure::Limit {
                resource: InputLimit::SyntaxNodes,
                observed,
                location,
                ..
            } => AdmissionFailure::Limit {
                resource: InputLimit::SyntaxNodes,
                limit: options.max_syntax_nodes,
                observed: visited_nodes.saturating_add(observed),
                location,
            },
            other => other,
        })?;
        visited_nodes += nodes;
        extended::check_definitions_in(source.parsed(), limits, &mut definitions)?;
        metadata::check_count(source.parsed(), limits, &mut metadata_count)?;
        let raised = raise(source.parsed());
        if !raised.diagnostics().is_empty() {
            return Err(AdmissionFailure::Raise(raised.diagnostics().to_vec()).into());
        }
        metadata::collect(raised.program(), &mut source_metadata)?;
        statements.extend(
            raised
                .program()
                .statements()
                .filter(|carrier| !matches!(carrier.get(), Statement::Include(_)))
                .cloned(),
        );
    }
    let entry = bundle
        .get(bundle.entry())
        .expect("a loaded bundle has its entry");
    let location = Location {
        source: entry.id(),
        span: entry.source().span(),
    };
    let source = SourceProgram::of_nodes(statements);
    Ok(extended::compile_owned(
        &source,
        options.core_limits,
        limits,
        location,
        source_metadata.finish(location)?,
    )?)
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
