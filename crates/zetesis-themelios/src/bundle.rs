//! Bounded original-file loading and include graphs, separate from S0 admission.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use themelios_base::line::LineIndex;
use themelios_base::source::{FromBytesRefusal, Source, SourceId, Sources};
use themelios_base::span::Location;
use themelios_syntax::ast::{self, AstToken};
use themelios_syntax::diagnostic::SyntaxError;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::{Parse, parse};
use themelios_syntax::tree::AstNode;

/// Independent source-graph ceilings. Counts refer to unique canonical files;
/// root occurrences have a separate bound. Include depth counts edges from
/// each ordered root, whose depth is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundleLimits {
    /// Maximum explicit root occurrences, including repeated path spellings.
    pub max_roots: usize,
    /// Maximum unique files across all roots and includes.
    pub max_files: usize,
    /// Maximum bytes read from one file before parsing.
    pub max_file_bytes: usize,
    /// Maximum original bytes retained across unique files.
    pub max_total_bytes: usize,
    /// Maximum include-path length, including paths through shared subgraphs.
    pub max_include_depth: usize,
}

impl Default for BundleLimits {
    fn default() -> Self {
        Self {
            max_roots: 256,
            max_files: 256,
            max_file_bytes: 1_048_576,
            max_total_bytes: 8_388_608,
            max_include_depth: 32,
        }
    }
}

/// One explicit input occurrence, preserving command-line or caller order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleRoot {
    id: SourceId,
    requested_path: PathBuf,
}
impl BundleRoot {
    /// Source identity shared with the canonical file catalog.
    #[must_use]
    pub fn id(&self) -> SourceId {
        self.id
    }
    /// Original root spelling. Relative paths use the bundle's captured cwd.
    #[must_use]
    pub fn requested_path(&self) -> &Path {
        &self.requested_path
    }
}

/// Which bounded filesystem lookup selected an original include target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncludeResolution {
    /// The include's original spelling was absolute.
    Absolute,
    /// Its relative spelling resolved in the captured working directory.
    WorkingDirectory,
    /// Direct filesystem lookup failed; resolution used the including source's directory.
    IncludingDirectory,
}

/// One located original include occurrence, resolved to a file identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncludeEdge {
    target: SourceId,
    requested_path: String,
    resolved_path: PathBuf,
    resolution: IncludeResolution,
    location: Location,
}

impl IncludeEdge {
    /// Identity of the included file in this bundle.
    #[must_use]
    pub fn target(&self) -> SourceId {
        self.target
    }
    /// Decoded path value from the original string literal, before resolution.
    #[must_use]
    pub fn requested_path(&self) -> &str {
        &self.requested_path
    }
    /// Exact selected lexical filename, before canonical identity deduplication.
    #[must_use]
    pub fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }
    /// Lookup that selected this target; no later cwd change affects the graph.
    #[must_use]
    pub fn resolution(&self) -> IncludeResolution {
        self.resolution
    }
    /// Span of the original include statement in its including source.
    #[must_use]
    pub fn location(&self) -> Location {
        self.location
    }
}

/// Original bytes, parsing, and include edges for one canonical file. Sources
/// are parsed independently; no concatenation changes spans or program parts.
#[derive(Debug)]
pub struct BundleSource {
    path: PathBuf,
    loaded_path: PathBuf,
    name: String,
    source: Source,
    parsed: Parse<ast::Program>,
    index: LineIndex,
    includes: Vec<IncludeEdge>,
}

impl BundleSource {
    /// Canonical physical file path used for identity and read access.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// First selected lexical filename, used for fallback include lookup.
    #[must_use]
    pub fn loaded_path(&self) -> &Path {
        &self.loaded_path
    }
    /// Stable identity within the bundle.
    #[must_use]
    pub fn id(&self) -> SourceId {
        self.source.id()
    }
    /// Original UTF-8 bytes under their source identity.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }
    /// Pinned themelios parse under the clingo dialect; all diagnostics were
    /// checked at loading. This is not a claim that S0 admits its constructs.
    #[must_use]
    pub fn parsed(&self) -> &Parse<ast::Program> {
        &self.parsed
    }
    /// Every include occurrence in source order, including repeated targets.
    #[must_use]
    pub fn includes(&self) -> &[IncludeEdge] {
        &self.includes
    }
}

/// A faithfully parsed original include graph. Canonicalization deduplicates
/// physical paths; a repeated include still has its own located edge. This
/// graph records source structure, not the semantics of include evaluation,
/// cross-file constants, or program-part composition. [`crate::admit`] remains
/// the separate single-source S0 door.
#[derive(Debug)]
pub struct SourceBundle {
    roots: Vec<BundleRoot>,
    working_directory: PathBuf,
    sources: Vec<BundleSource>,
    total_bytes: usize,
}

impl SourceBundle {
    /// Load one original file and its include closure. See [`Self::load_many`]
    /// for the captured-cwd lookup policy and globally shared resource bounds.
    ///
    /// # Errors
    /// Returns typed I/O, source-model, parse, cycle, include-form, and resource
    /// failures. There is no successful partial bundle.
    pub fn load(path: impl AsRef<Path>, limits: BundleLimits) -> Result<Self, BundleError> {
        Self::load_many([path], limits)
    }
    /// Load ordered original roots using a shared, bounded canonical catalog.
    /// The working directory is captured once. Relative includes first try
    /// that directory, then the including source's lexical parent on a filesystem failure;
    /// absolute includes use their exact spelling. Root and include spellings
    /// remain available for the separate semantic identity check. Text is never
    /// concatenated or rewritten, and no external engine or script is invoked.
    ///
    /// # Errors
    /// Refuses an empty root sequence, unsupported library includes, invalid
    /// original sources, cycles, I/O failures, or any shared resource ceiling.
    /// No successful partial bundle is returned.
    pub fn load_many<P: AsRef<Path>>(
        paths: impl IntoIterator<Item = P>,
        limits: BundleLimits,
    ) -> Result<Self, BundleError> {
        Loader::new(limits)?.load_many(paths)
    }
    /// Ordered explicit roots, including repeated occurrences of one source.
    #[must_use]
    pub fn roots(&self) -> &[BundleRoot] {
        &self.roots
    }
    /// Working directory captured before any root or include lookup.
    #[must_use]
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }
    /// First explicit source identity; a loaded bundle always has source 0.
    #[must_use]
    pub fn entry(&self) -> SourceId {
        self.roots[0].id
    }
    /// Unique source files in deterministic depth-first discovery order.
    #[must_use]
    pub fn sources(&self) -> &[BundleSource] {
        &self.sources
    }
    /// Resolve a source identity, returning no partial catalog entry on failure.
    #[must_use]
    pub fn get(&self, id: SourceId) -> Option<&BundleSource> {
        self.sources.get(usize::try_from(id.get()).ok()?)
    }
    /// Original bytes retained across unique files.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

impl Sources for SourceBundle {
    fn name(&self, id: SourceId) -> Option<&str> {
        self.get(id).map(|source| source.name.as_str())
    }
    fn text(&self, id: SourceId) -> Option<&str> {
        self.get(id).map(|source| source.source.text())
    }
    fn line_index(&self, id: SourceId) -> Option<&LineIndex> {
        self.get(id).map(|source| &source.index)
    }
}

/// A source graph resource, kept separate from solver or admission budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleResource {
    /// Explicit ordered root occurrences, including repeated filenames.
    Roots,
    /// Unique canonical source files.
    Files,
    /// Bytes in one original file.
    FileBytes,
    /// Bytes retained across original files.
    TotalBytes,
    /// Edges in any path from the entry.
    IncludeDepth,
    /// The source model's fixed byte-offset width.
    SourceCoordinates,
    /// The source identity's fixed integer width.
    SourceIdentities,
}

/// The filesystem operation that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleIoOperation {
    /// Capture the process working directory once for the whole source graph.
    CurrentDirectory,
    /// Resolve a path to its canonical physical identity.
    Canonicalize,
    /// Open the source read-only.
    Open,
    /// Inspect an opened file's type and byte length.
    Metadata,
    /// Read bounded original bytes.
    Read,
}

/// A typed bundle-loading refusal. Include locations belong to the original
/// including file; syntax failures retain their original source for rendering.
#[derive(Debug)]
pub enum BundleError {
    /// No original input root was supplied.
    EmptyRoots,
    /// An original file could not be accessed.
    Io {
        /// Requested or canonical file path at the failing operation.
        path: PathBuf,
        /// Operation that failed.
        operation: BundleIoOperation,
        /// The operating system's typed error.
        error: io::Error,
        /// Include origin, absent for an explicit root.
        including: Option<Location>,
    },
    /// Only regular filesystem source files are admitted.
    NotRegularFile {
        /// The path whose opened handle is not a regular file.
        path: PathBuf,
        /// Include origin, absent for an explicit root.
        including: Option<Location>,
    },
    /// A configured or representation ceiling was exceeded.
    Limit {
        /// Counted resource.
        resource: BundleResource,
        /// Inclusive ceiling.
        limit: u128,
        /// Count observed or proposed when loading stopped.
        observed: u128,
        /// File associated with the refusal.
        path: PathBuf,
        /// Include origin, absent for an explicit root.
        including: Option<Location>,
    },
    /// A canonical path was revisited on the active include chain.
    Cycle {
        /// Complete active chain plus the repeated target.
        chain: Vec<PathBuf>,
        /// Include edge that would close the cycle.
        location: Location,
    },
    /// Bytes could not enter the themelios source model.
    Source {
        /// Canonical file path.
        path: PathBuf,
        /// UTF-8 or source-coordinate refusal.
        error: FromBytesRefusal,
        /// Include origin, absent for an explicit root.
        including: Option<Location>,
    },
    /// The parse was not clean; no recovered file enters a successful bundle.
    Syntax {
        /// Canonical original file path.
        path: PathBuf,
        /// Original bytes and identity for every diagnostic below.
        source: Source,
        /// All parser diagnostics in original order.
        diagnostics: Vec<SyntaxError>,
    },
    /// A library include has no configured filesystem interpretation.
    LibraryInclude {
        /// Original library identifier.
        name: String,
        /// Original statement location.
        location: Location,
    },
    /// The include string could not be decoded under its parse's dialect.
    IncludeString {
        /// Typed dialect decoding failure.
        error: ast::InvalidStringLiteral,
        /// Original statement location.
        location: Location,
    },
    /// A clean include node unexpectedly lacked either supported target form.
    IncompleteInclude {
        /// Original statement location.
        location: Location,
    },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoots => f.write_str("source bundle requires at least one input root"),
            Self::Io {
                path,
                operation,
                error,
                ..
            } => write!(f, "{operation:?} failed for {}: {error}", path.display()),
            Self::NotRegularFile { path, .. } => {
                write!(f, "source is not a regular file: {}", path.display())
            }
            Self::Limit {
                resource,
                limit,
                observed,
                path,
                ..
            } => write!(
                f,
                "bundle {resource:?} count {observed} exceeds {limit} at {}",
                path.display()
            ),
            Self::Cycle { chain, .. } => {
                write!(f, "include cycle across {} path entries", chain.len())
            }
            Self::Source { path, error, .. } => {
                write!(f, "invalid source {}: {error}", path.display())
            }
            Self::Syntax {
                path, diagnostics, ..
            } => write!(
                f,
                "{} reported {} syntax diagnostic(s)",
                path.display(),
                diagnostics.len()
            ),
            Self::LibraryInclude { name, .. } => {
                write!(f, "no library include search path is defined for <{name}>")
            }
            Self::IncludeString { error, .. } => error.fmt(f),
            Self::IncompleteInclude { .. } => {
                f.write_str("include statement has no complete target")
            }
        }
    }
}

impl std::error::Error for BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Source { error, .. } => Some(error),
            Self::IncludeString { error, .. } => Some(error),
            _ => None,
        }
    }
}

struct PendingInclude {
    requested_path: String,
    location: Location,
}
struct Frame {
    index: usize,
    next: usize,
    depth: usize,
}
struct Loader {
    limits: BundleLimits,
    bundle: SourceBundle,
    ids: BTreeMap<PathBuf, usize>,
    active: BTreeSet<PathBuf>,
    pending: Vec<Vec<PendingInclude>>,
    heights: Vec<usize>,
}

impl Loader {
    fn new(limits: BundleLimits) -> Result<Self, BundleError> {
        let working_directory = std::env::current_dir().map_err(|error| BundleError::Io {
            path: PathBuf::from("."),
            operation: BundleIoOperation::CurrentDirectory,
            error,
            including: None,
        })?;
        Ok(Self {
            limits,
            bundle: SourceBundle {
                roots: Vec::new(),
                working_directory,
                sources: Vec::new(),
                total_bytes: 0,
            },
            ids: BTreeMap::new(),
            active: BTreeSet::new(),
            pending: Vec::new(),
            heights: Vec::new(),
        })
    }

    fn load_many<P: AsRef<Path>>(
        mut self,
        paths: impl IntoIterator<Item = P>,
    ) -> Result<SourceBundle, BundleError> {
        for requested in paths {
            let requested = requested.as_ref();
            limit(
                BundleResource::Roots,
                count(self.bundle.roots.len()) + 1,
                count(self.limits.max_roots),
                requested,
                None,
            )?;
            let path = canonical(&self.bundle.working_directory.join(requested), None)?;
            let (index, fresh) = if let Some(index) = self.ids.get(&path) {
                (*index, false)
            } else {
                (self.add_source(path, requested.to_path_buf(), None)?, true)
            };
            self.bundle.roots.push(BundleRoot {
                id: self.bundle.sources[index].id(),
                requested_path: requested.to_path_buf(),
            });
            if fresh {
                self.visit(index)?;
            }
        }
        if self.bundle.roots.is_empty() {
            return Err(BundleError::EmptyRoots);
        }
        Ok(self.bundle)
    }

    fn visit(&mut self, entry: usize) -> Result<(), BundleError> {
        self.active.insert(self.bundle.sources[entry].path.clone());
        let mut frames = vec![Frame {
            index: entry,
            next: 0,
            depth: 0,
        }];
        while let Some(frame) = frames.last_mut() {
            if frame.next == self.pending[frame.index].len() {
                let index = frame.index;
                let height = self.bundle.sources[index]
                    .includes
                    .iter()
                    .map(|edge| self.heights[edge.target.get() as usize] + 1)
                    .max()
                    .unwrap_or(0);
                self.heights[index] = height;
                self.active.remove(&self.bundle.sources[index].path);
                frames.pop();
                continue;
            }
            let parent = frame.index;
            let depth = frame.depth + 1;
            let pending = &self.pending[parent][frame.next];
            let requested_path = pending.requested_path.clone();
            let location = pending.location;
            frame.next += 1;
            limit(
                BundleResource::IncludeDepth,
                count(depth),
                count(self.limits.max_include_depth),
                Path::new(&requested_path),
                Some(location),
            )?;
            let (path, resolved_path, resolution) =
                self.resolve(parent, &requested_path, location)?;
            if self.active.contains(&path) {
                let mut chain: Vec<_> = frames
                    .iter()
                    .map(|frame| self.bundle.sources[frame.index].path.clone())
                    .collect();
                chain.push(path);
                return Err(BundleError::Cycle { chain, location });
            }
            let (target, fresh) = if let Some(index) = self.ids.get(&path) {
                let longest = depth + self.heights[*index];
                limit(
                    BundleResource::IncludeDepth,
                    count(longest),
                    count(self.limits.max_include_depth),
                    &path,
                    Some(location),
                )?;
                (*index, false)
            } else {
                (
                    self.add_source(path.clone(), resolved_path.clone(), Some(location))?,
                    true,
                )
            };
            let target_id = self.bundle.sources[target].id();
            self.bundle.sources[parent].includes.push(IncludeEdge {
                target: target_id,
                requested_path,
                resolved_path,
                resolution,
                location,
            });
            if fresh {
                self.active.insert(path);
                frames.push(Frame {
                    index: target,
                    next: 0,
                    depth,
                });
            }
        }
        Ok(())
    }

    fn resolve(
        &self,
        parent: usize,
        requested: &str,
        location: Location,
    ) -> Result<(PathBuf, PathBuf, IncludeResolution), BundleError> {
        let spelling = Path::new(requested);
        let direct = include_candidate(&self.bundle.working_directory.join(spelling), location);
        match direct {
            Ok(path) => Ok((
                path,
                spelling.to_path_buf(),
                if spelling.is_absolute() {
                    IncludeResolution::Absolute
                } else {
                    IncludeResolution::WorkingDirectory
                },
            )),
            Err(BundleError::Io { .. }) if !spelling.is_absolute() => {
                let source = &self.bundle.sources[parent];
                let fallback = source
                    .loaded_path
                    .parent()
                    .unwrap_or(Path::new(""))
                    .join(spelling);
                let path =
                    include_candidate(&self.bundle.working_directory.join(&fallback), location)?;
                Ok((path, fallback, IncludeResolution::IncludingDirectory))
            }
            Err(error) => Err(error),
        }
    }

    fn add_source(
        &mut self,
        path: PathBuf,
        loaded_path: PathBuf,
        including: Option<Location>,
    ) -> Result<usize, BundleError> {
        let index = self.bundle.sources.len();
        limit(
            BundleResource::Files,
            count(index) + 1,
            count(self.limits.max_files),
            &path,
            including,
        )?;
        let id = u32::try_from(index).map_err(|_| BundleError::Limit {
            resource: BundleResource::SourceIdentities,
            limit: u128::from(u32::MAX),
            observed: count(index),
            path: path.clone(),
            including,
        })?;
        let source = read_source(
            &path,
            SourceId::new(id),
            self.limits,
            self.bundle.total_bytes,
            including,
        )?;
        let parsed = parse(&source, Dialect::Clingo);
        if !parsed.diagnostics().is_empty() {
            return Err(BundleError::Syntax {
                path,
                source,
                diagnostics: parsed.diagnostics().to_vec(),
            });
        }
        let pending = includes(&parsed)?;
        let line_index = LineIndex::of(&source);
        self.bundle.total_bytes += source.text().len();
        let name = path.display().to_string();
        self.bundle.sources.push(BundleSource {
            path: path.clone(),
            loaded_path,
            name,
            source,
            parsed,
            index: line_index,
            includes: Vec::new(),
        });
        self.pending.push(pending);
        self.heights.push(0);
        self.ids.insert(path, index);
        Ok(index)
    }
}

fn canonical(path: &Path, including: Option<Location>) -> Result<PathBuf, BundleError> {
    path.canonicalize().map_err(|error| BundleError::Io {
        path: path.to_path_buf(),
        operation: BundleIoOperation::Canonicalize,
        error,
        including,
    })
}

// Lookup may fall back after filesystem failures, but a selected nonregular
// file is an explicit profile refusal. Parsing and byte ceilings occur only
// after selection and must never select a different program on failure.
fn include_candidate(path: &Path, location: Location) -> Result<PathBuf, BundleError> {
    let canonical = canonical(path, Some(location))?;
    regular_metadata(&canonical, Some(location))?;
    File::open(&canonical).map_err(|error| BundleError::Io {
        path: canonical.clone(),
        operation: BundleIoOperation::Open,
        error,
        including: Some(location),
    })?;
    Ok(canonical)
}

fn regular_metadata(
    path: &Path,
    including: Option<Location>,
) -> Result<std::fs::Metadata, BundleError> {
    let metadata = std::fs::metadata(path).map_err(|error| BundleError::Io {
        path: path.to_path_buf(),
        operation: BundleIoOperation::Metadata,
        error,
        including,
    })?;
    if !metadata.is_file() {
        return Err(BundleError::NotRegularFile {
            path: path.to_path_buf(),
            including,
        });
    }
    Ok(metadata)
}

fn read_source(
    path: &Path,
    id: SourceId,
    limits: BundleLimits,
    used: usize,
    including: Option<Location>,
) -> Result<Source, BundleError> {
    let io_failure = |operation, error| BundleError::Io {
        path: path.to_path_buf(),
        operation,
        error,
        including,
    };
    // Refuse known special files before open: opening a FIFO could otherwise
    // block before the source loader can inspect its descriptor. Recheck the
    // opened descriptor below, since pathname metadata is only a preflight.
    let preliminary = regular_metadata(path, including)?;
    byte_limits(u128::from(preliminary.len()), limits, used, path, including)?;
    let file = File::open(path).map_err(|error| io_failure(BundleIoOperation::Open, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_failure(BundleIoOperation::Metadata, error))?;
    if !metadata.is_file() {
        return Err(BundleError::NotRegularFile {
            path: path.to_path_buf(),
            including,
        });
    }
    byte_limits(u128::from(metadata.len()), limits, used, path, including)?;
    let remaining = limits.max_total_bytes - used;
    let allowed = limits.max_file_bytes.min(remaining).min(Source::MAX_LEN);
    let mut bytes = Vec::new();
    file.take(u64::try_from(allowed).expect("the source coordinate bound fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_failure(BundleIoOperation::Read, error))?;
    byte_limits(count(bytes.len()), limits, used, path, including)?;
    Source::from_bytes(id, bytes).map_err(|error| BundleError::Source {
        path: path.to_path_buf(),
        error,
        including,
    })
}

fn byte_limits(
    bytes: u128,
    limits: BundleLimits,
    used: usize,
    path: &Path,
    including: Option<Location>,
) -> Result<(), BundleError> {
    // Counts use u128 so even a metadata length near u64::MAX can be added to
    // already-retained bytes without wrapping the reported proposed total.
    for (resource, observed, ceiling) in [
        (
            BundleResource::FileBytes,
            bytes,
            count(limits.max_file_bytes),
        ),
        (
            BundleResource::TotalBytes,
            count(used) + bytes,
            count(limits.max_total_bytes),
        ),
        (
            BundleResource::SourceCoordinates,
            bytes,
            count(Source::MAX_LEN),
        ),
    ] {
        limit(resource, observed, ceiling, path, including)?;
    }
    Ok(())
}

fn includes(parsed: &Parse<ast::Program>) -> Result<Vec<PendingInclude>, BundleError> {
    let mut result = Vec::new();
    for statement in parsed.tree().statements() {
        let ast::Statement::Include(include) = statement else {
            continue;
        };
        let location = parsed.location(include.syntax().text_range());
        let Some(path) = include.path() else {
            if let Some(library) = include.library() {
                return Err(BundleError::LibraryInclude {
                    name: library.text().to_owned(),
                    location,
                });
            }
            return Err(BundleError::IncompleteInclude { location });
        };
        let requested_path = parsed
            .string_value(&path)
            .map_err(|error| BundleError::IncludeString { error, location })?;
        result.push(PendingInclude {
            requested_path,
            location,
        });
    }
    Ok(result)
}

fn limit(
    resource: BundleResource,
    observed: u128,
    ceiling: u128,
    path: &Path,
    including: Option<Location>,
) -> Result<(), BundleError> {
    if observed > ceiling {
        Err(BundleError::Limit {
            resource,
            limit: ceiling,
            observed,
            path: path.to_path_buf(),
            including,
        })
    } else {
        Ok(())
    }
}

fn count(value: usize) -> u128 {
    value as u128
}
