//! The comparisons against clingo among the maintained sources: ignored
//! tests whose reason names the oracle, each placed in the integration test
//! target that compiles it, found by following the module declarations from
//! each target's root file, and named as that target's harness names it. An
//! `ignore` written inside `cfg_attr`, and a module declared inside an inline
//! module, are outside this reading and are refused rather than guessed.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::{Expr, Item, ItemFn, Lit, Meta, Token};

/// One ignored test whose reason names clingo.
pub(super) struct Comparison {
    /// The package whose sources hold the test.
    pub package: String,
    /// The integration test target compiling the test, when one does.
    pub target: Option<String>,
    /// The test's name as its target's harness lists it: the module path
    /// from the target's root, then the function. Outside a test target,
    /// the path from the test's own file.
    pub name: String,
    /// The source file, relative to the repository.
    pub path: PathBuf,
    /// The line of the test function, from one.
    pub line: usize,
}

impl fmt::Display for Comparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            Some(target) => write!(
                f,
                "-p {} --test {}: {} ({}:{})",
                self.package,
                target,
                self.name,
                self.path.display(),
                self.line
            ),
            None => write!(
                f,
                "-p {}: {} ({}:{}) is in no test target",
                self.package,
                self.name,
                self.path.display(),
                self.line
            ),
        }
    }
}

/// What one source file holds: its comparisons and its module declarations.
#[derive(Default)]
struct Scan {
    hits: Vec<Hit>,
    declarations: Vec<Declaration>,
}

/// An ignored test whose reason names clingo.
struct Hit {
    /// The inline modules holding the function, then the function.
    name: String,
    line: usize,
}

/// A `mod name;` item, with its `#[path]` when one is written.
struct Declaration {
    name: String,
    path: Option<String>,
    line: usize,
}

/// The comparisons among `sources`, files under `root`, in path order; a
/// comparison in a module several test roots include is listed once for
/// each of them.
///
/// # Errors
/// Returns the first unreadable or unparsable source, a declared module
/// with no source or two among `sources`, circular module declarations, a
/// module declared inside an inline module, an ignored comparison that is
/// not a test, or a source in no package.
pub(super) fn comparisons(root: &Path, sources: &[PathBuf]) -> io::Result<Vec<Comparison>> {
    let root = normalize(root);
    let root = root.as_path();
    let mut scans = BTreeMap::new();
    for source in sources {
        let text = super::authored_sources::read(source)?;
        let scan = scan(&text)
            .map_err(|error| invalid(source, &format!("{}: {error}", error.span().start().line)))?;
        scans.insert(normalize(source), scan);
    }
    // Each file's places in the test targets: the target, and the module path
    // from its root at which the file is compiled.
    let mut targets: BTreeMap<&Path, Vec<(String, String)>> = BTreeMap::new();
    for path in scans.keys() {
        let Some(target) = test_target(path) else {
            continue;
        };
        let mut included = BTreeMap::new();
        include(path, "", &scans, &mut Vec::new(), &mut included)?;
        for (file, modules) in included {
            let places = targets.entry(file).or_default();
            places.extend(modules.into_iter().map(|module| (target.clone(), module)));
        }
    }
    let mut packages = BTreeMap::new();
    let mut found = Vec::new();
    for (path, scan) in &scans {
        if scan.hits.is_empty() {
            continue;
        }
        let package = package_name(root, path, &mut packages)?;
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let places = targets.get(path.as_path()).map_or(&[][..], Vec::as_slice);
        for hit in &scan.hits {
            let comparison = |target, name| Comparison {
                package: package.clone(),
                target,
                name,
                path: relative.clone(),
                line: hit.line,
            };
            if places.is_empty() {
                found.push(comparison(None, hit.name.clone()));
            } else {
                found.extend(places.iter().map(|(target, module)| {
                    comparison(Some(target.clone()), qualified(module, &hit.name))
                }));
            }
        }
    }
    Ok(found)
}

/// The test target whose root `path` is, if it is one: Cargo's
/// `tests/NAME.rs` and `tests/NAME/main.rs` of a package, named `NAME`.
fn test_target(path: &Path) -> Option<String> {
    let directory = path.parent()?;
    let name = if is_tests_directory(directory) {
        path.file_stem()?
    } else if path.file_name()? == "main.rs" && is_tests_directory(directory.parent()?) {
        directory.file_name()?
    } else {
        return None;
    };
    name.to_str().map(str::to_owned)
}

/// A package's `tests` directory.
fn is_tests_directory(directory: &Path) -> bool {
    directory.file_name().is_some_and(|name| name == "tests")
        && directory
            .parent()
            .is_some_and(|package| package.join("Cargo.toml").is_file())
}

/// `name` in the module at path `module`; the root module's path is empty.
fn qualified(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}::{name}")
    }
}

fn stem(path: &Path) -> io::Result<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid(path, "a source file needs a name"))
}

/// Record in `included` that `file` is compiled as the module at path
/// `module`, and every file its declarations reach as that module's
/// descendants; a file reached twice is recorded at both paths. `chain` holds
/// the files from the root to `file`. A declaration with a `path` names a
/// file relative to the declaring file's directory; one without names
/// `name.rs` or `name/mod.rs` beside a root or a `mod.rs`, and under the
/// declaring file's own directory otherwise.
fn include<'a>(
    file: &'a Path,
    module: &str,
    scans: &'a BTreeMap<PathBuf, Scan>,
    chain: &mut Vec<&'a Path>,
    included: &mut BTreeMap<&'a Path, Vec<String>>,
) -> io::Result<()> {
    if chain.contains(&file) {
        return Err(invalid(file, "circular module declarations"));
    }
    included.entry(file).or_default().push(module.to_owned());
    let directory = file
        .parent()
        .ok_or_else(|| invalid(file, "a source file needs a directory"))?;
    let beside =
        test_target(file).is_some() || file.file_name().is_some_and(|name| name == "mod.rs");
    chain.push(file);
    for declaration in &scans[file].declarations {
        let candidates = if let Some(path) = &declaration.path {
            vec![normalize(&directory.join(path))]
        } else {
            let base = if beside {
                directory.to_path_buf()
            } else {
                directory.join(stem(file)?)
            };
            vec![
                base.join(format!("{}.rs", declaration.name)),
                base.join(&declaration.name).join("mod.rs"),
            ]
        };
        let mut present = candidates
            .iter()
            .filter_map(|candidate| scans.get_key_value(candidate))
            .map(|(key, _)| key.as_path());
        let Some(next) = present.next() else {
            return Err(invalid(
                file,
                &format!(
                    "line {}: no source among the inventory for module `{}`",
                    declaration.line, declaration.name
                ),
            ));
        };
        if present.next().is_some() {
            return Err(invalid(
                file,
                &format!(
                    "line {}: two sources for module `{}`",
                    declaration.line, declaration.name
                ),
            ));
        }
        let child = qualified(module, &declaration.name);
        include(next, &child, scans, chain, included)?;
    }
    chain.pop();
    Ok(())
}

/// The name of the package whose manifest is nearest above `path`.
fn package_name(
    root: &Path,
    path: &Path,
    names: &mut BTreeMap<PathBuf, String>,
) -> io::Result<String> {
    let mut directory = path.parent();
    while let Some(package) = directory {
        let manifest = package.join("Cargo.toml");
        if manifest.is_file() {
            if let Some(name) = names.get(package) {
                return Ok(name.clone());
            }
            let name = manifest_name(&super::authored_sources::read(&manifest)?)
                .ok_or_else(|| invalid(&manifest, "the manifest names no package"))?;
            names.insert(package.to_path_buf(), name.clone());
            return Ok(name);
        }
        if package == root {
            break;
        }
        directory = package.parent();
    }
    Err(invalid(path, "the source is in no package"))
}

/// The `name` of a manifest's `[package]` table.
fn manifest_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// Resolve `.` and `..` lexically, so that a declared path compares equal
/// to the inventory's spelling of the same file.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    normal
}

fn invalid(path: &Path, message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {message}", path.display()),
    )
}

fn scan(text: &str) -> syn::Result<Scan> {
    let file = syn::parse_file(text)?;
    let mut scan = Scan::default();
    visit(&file.items, "", &mut scan)?;
    Ok(scan)
}

/// Record the comparisons and module declarations among `items`; `inline`
/// is the path of the inline modules they sit in, empty at the file's top.
fn visit(items: &[Item], inline: &str, scan: &mut Scan) -> syn::Result<()> {
    for item in items {
        match item {
            Item::Fn(function) => scan.hits.extend(hit(function, inline)?),
            Item::Mod(module) => match &module.content {
                Some((_, nested)) => {
                    visit(nested, &qualified(inline, &module.ident.to_string()), scan)?;
                }
                None if !inline.is_empty() => {
                    return Err(syn::Error::new(
                        module.ident.span(),
                        "a module declared inside an inline module is outside this reading",
                    ));
                }
                None => scan.declarations.push(Declaration {
                    name: module.ident.to_string(),
                    path: path_attribute(&module.attrs)?,
                    line: module.ident.span().start().line,
                }),
            },
            _ => {}
        }
    }
    Ok(())
}

/// The function, in the inline modules at path `inline`, as a comparison,
/// when an `ignore` reason names clingo.
fn hit(function: &ItemFn, inline: &str) -> syn::Result<Option<Hit>> {
    let mut test = false;
    let mut reason = None;
    for attribute in &function.attrs {
        if attribute.path().is_ident("test") {
            test = true;
        }
        if attribute.path().is_ident("cfg_attr") && carries_ignore(&attribute.meta)? {
            return Err(syn::Error::new_spanned(
                attribute,
                "an ignore under cfg_attr is outside this reading",
            ));
        }
        if attribute.path().is_ident("ignore") {
            reason = Some(match &attribute.meta {
                Meta::Path(_) => String::new(),
                Meta::NameValue(pair) => string(&pair.value)?,
                Meta::List(list) => {
                    return Err(syn::Error::new_spanned(list, "ignore takes a reason"));
                }
            });
        }
    }
    let Some(reason) = reason else {
        return Ok(None);
    };
    if !reason.to_ascii_lowercase().contains("clingo") {
        return Ok(None);
    }
    if !test {
        return Err(syn::Error::new(
            function.sig.ident.span(),
            "an ignored clingo comparison is not a test",
        ));
    }
    Ok(Some(Hit {
        name: qualified(inline, &function.sig.ident.to_string()),
        line: function.sig.ident.span().start().line,
    }))
}

/// Whether `meta` is an `ignore`, or a `cfg_attr` carrying one at any depth.
fn carries_ignore(meta: &Meta) -> syn::Result<bool> {
    if meta.path().is_ident("ignore") {
        return Ok(true);
    }
    if meta.path().is_ident("cfg_attr")
        && let Meta::List(list) = meta
    {
        let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for entry in entries.iter().skip(1) {
            if carries_ignore(entry)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn path_attribute(attributes: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for attribute in attributes {
        if attribute.path().is_ident("path") {
            return match &attribute.meta {
                Meta::NameValue(pair) => string(&pair.value).map(Some),
                other => Err(syn::Error::new_spanned(other, "path takes a file")),
            };
        }
    }
    Ok(None)
}

fn string(value: &Expr) -> syn::Result<String> {
    if let Expr::Lit(literal) = value
        && let Lit::Str(text) = &literal.lit
    {
        return Ok(text.value());
    }
    Err(syn::Error::new_spanned(value, "expected a string literal"))
}
