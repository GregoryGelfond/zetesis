//! Live documentation links only to what the repository holds.
//!
//! In a live Markdown document, every relative link and every link to this
//! repository's main branch (`blob/main/…` or `tree/main/…`) must name a file
//! or directory of the working tree. The manual's links into the Rust API
//! reference, which rustdoc generates beside the built manual, must name a
//! workspace crate. A dated record ([`invocations::is_record`]) is not checked:
//! its links name what it recorded. Nor are links pinned to a commit, links to
//! other sites, a link's fragment or query, or anything inside code.

use std::path::{Component, Path, PathBuf};

use crate::{Error, invocations, require};

/// The repository whose main-branch links name the working tree; the manual's
/// `git-repository-url`.
pub const REPOSITORY: &str = "https://github.com/GregoryGelfond/zetesis";

/// The manual's source directory. It is built into `target/book`, and rustdoc
/// into `target/doc` beside it, so a manual link resolving to this directory's
/// sibling `doc` names the API reference.
pub const BOOK: &str = "docs/book";

/// The paths after [`REPOSITORY`] that name the main branch's tree.
const MAIN: [&str; 2] = ["/blob/main/", "/tree/main/"];

/// Where a link leads, as far as the working tree can say.
enum Target {
    /// A repository-relative path that must exist.
    Present(PathBuf),
    /// A relative link climbing above the repository.
    Outside,
    /// A link this check does not judge.
    Unchecked,
}

/// Check each live document's links against the working tree.
///
/// `documents` pairs each document's repository-relative path with its text;
/// `exists` answers whether a repository-relative path names a file or
/// directory.
///
/// # Errors
/// Returns [`Error::Invalid`] listing every link to what the repository does
/// not hold.
pub fn check<'a>(
    documents: impl IntoIterator<Item = (&'a str, &'a str)>,
    exists: impl Fn(&Path) -> bool,
) -> Result<(), Error> {
    let mut dead = Vec::new();
    for (path, text) in documents {
        if invocations::is_record(text) {
            continue;
        }
        let document = Path::new(path);
        for (line, link) in links(text) {
            let held = match target(document, link) {
                Target::Present(path) => exists(&path),
                Target::Outside => false,
                Target::Unchecked => true,
            };
            if !held {
                dead.push(format!("{path}:{line}: {link}"));
            }
        }
    }
    require(
        dead.is_empty(),
        format!(
            "live documentation links to what the repository does not hold:\n{}",
            dead.join("\n")
        ),
    )
}

/// Each link outside code, with its one-based line number: the targets of
/// inline links and reference definitions, and every link to this
/// repository's main branch, whether or not it is written as a link.
fn links(text: &str) -> Vec<(usize, &str)> {
    let mut links = Vec::new();
    let mut fence = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        // Backticks alternate between prose and code spans.
        for (segment, prose) in line.split('`').step_by(2).enumerate() {
            let definition = if segment == 0 {
                reference_definition(prose.trim_start())
            } else {
                None
            };
            let written = inline_targets(prose).into_iter().chain(definition);
            // Main-branch links are taken from the prose itself, once each.
            links.extend(
                written
                    .filter(|target| main_branch_path(target).is_none())
                    .map(|target| (index + 1, target)),
            );
            links.extend(main_branch_links(prose).map(|link| (index + 1, link)));
        }
    }
    links
}

/// The targets of the inline links and images in `prose`, as in
/// `[text](target "title")`.
fn inline_targets(prose: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut remaining = prose;
    while let Some(start) = remaining.find("](") {
        let after = &remaining[start + 2..];
        let end = after.find(')').unwrap_or(after.len());
        targets.extend(after[..end].split_whitespace().next().map(unbracketed));
        remaining = &after[end..];
    }
    targets
}

/// The target of a reference definition such as `[label]: path "title"`.
fn reference_definition(line: &str) -> Option<&str> {
    let (label, rest) = line.strip_prefix('[')?.split_once("]:")?;
    if label.is_empty() || label.contains(']') {
        return None;
    }
    rest.split_whitespace().next().map(unbracketed)
}

/// Every link to this repository's main branch in `prose`, without trailing
/// sentence punctuation.
fn main_branch_links(prose: &str) -> impl Iterator<Item = &str> {
    prose.match_indices(REPOSITORY).filter_map(|(start, _)| {
        let link = &prose[start..];
        let end = link
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ')' | '>' | '"' | ']')
            })
            .unwrap_or(link.len());
        let link = link[..end].trim_end_matches(['.', ',', ';', ':']);
        main_branch_path(link).map(|_| link)
    })
}

/// The repository path a main-branch link names, if `link` is one.
fn main_branch_path(link: &str) -> Option<&str> {
    let rest = link.strip_prefix(REPOSITORY)?;
    MAIN.iter().find_map(|main| rest.strip_prefix(main))
}

/// `target` without the angle brackets Markdown allows around it.
fn unbracketed(target: &str) -> &str {
    target.trim_start_matches('<').trim_end_matches('>')
}

/// What a link in `document` names.
fn target(document: &Path, link: &str) -> Target {
    let without_fragment = |path: &'_ str| -> PathBuf {
        PathBuf::from(path.split(['#', '?']).next().unwrap_or_default())
    };
    if let Some(path) = main_branch_path(link) {
        return Target::Present(without_fragment(path));
    }
    let path = without_fragment(link);
    if path.as_os_str().is_empty() || scheme(link) {
        return Target::Unchecked;
    }
    let mut resolved = PathBuf::new();
    for component in document
        .parent()
        .unwrap_or(Path::new(""))
        .join(path)
        .components()
    {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::ParentDir => {
                if !resolved.pop() {
                    return Target::Outside;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return Target::Unchecked,
        }
    }
    let api = Path::new(BOOK).with_file_name("doc");
    if document.starts_with(BOOK)
        && let Ok(inside) = resolved.strip_prefix(&api)
    {
        // rustdoc names a crate's directory after its library.
        return match inside.components().next() {
            Some(Component::Normal(library)) => Target::Present(
                Path::new("crates")
                    .join(library.to_string_lossy().replace('_', "-"))
                    .join("Cargo.toml"),
            ),
            _ => Target::Outside,
        };
    }
    Target::Present(resolved)
}

/// Whether `link` begins with a URL scheme such as `https:` or `mailto:`.
fn scheme(link: &str) -> bool {
    link.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|character: char| character.is_ascii_alphabetic())
            && scheme.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
    })
}
