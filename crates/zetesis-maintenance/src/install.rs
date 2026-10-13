//! The installed tool set has one source, the installer's lists.
//!
//! `scripts/install.sh` names the packages it builds and the tools it installs.
//! INSTALL.md must describe exactly those tools in its table and name exactly
//! those packages in its Cargo commands, and the packages' binary targets, as
//! Cargo reports them, must be exactly those tools. Every input is text the
//! caller has already read; nothing here builds or installs anything.

use std::collections::{BTreeMap, BTreeSet};

use crate::workspace::{self, Package};
use crate::{Error, require};

/// The heading of INSTALL.md's section whose table lists the installed tools.
const TOOLS_HEADING: &str = "## What gets installed";
/// The Cargo command INSTALL.md gives for installing one package's binaries.
const CARGO_INSTALL: &str = "cargo install --locked --path crates/";

/// Check that the installer, INSTALL.md and the packages' binaries agree.
///
/// `installer` is the text of `scripts/install.sh`, `guide` the text of
/// INSTALL.md and `metadata` the output of `cargo metadata --no-deps
/// --format-version 1` for the workspace.
///
/// # Errors
/// Returns [`Error::Invalid`] naming the first disagreement or an unreadable
/// list, and [`Error::Json`] for malformed metadata.
pub fn check(installer: &str, guide: &str, metadata: &[u8]) -> Result<(), Error> {
    let packages = list(installer, "packages")?;
    let tools = list(installer, "tools")?;
    let members = workspace::packages(metadata)?;
    let mut built = BTreeSet::new();
    for name in &packages {
        let package = members.get(name).ok_or_else(|| {
            Error::Invalid(format!(
                "the installer builds {name}, which is not a workspace package"
            ))
        })?;
        built.extend(package.binaries.iter().cloned());
    }
    require(
        built == tools,
        format!("the installer's packages build {built:?}, but it installs {tools:?}"),
    )?;
    let described = described_tools(guide)?;
    require(
        described == tools,
        format!("INSTALL.md describes {described:?}, but the installer installs {tools:?}"),
    )?;
    let named = cargo_packages(guide, &members)?;
    require(
        named == packages,
        format!(
            "INSTALL.md's Cargo commands install {named:?}, but the installer builds {packages:?}"
        ),
    )
}

/// The constant through which the CLI names the directory of its GPU sibling.
const LIBEXEC_CONSTANT: &str = "LIBEXEC";

/// Check that the installer places the GPU sibling in the directory the CLI's
/// front looks in, and that each document names that directory.
///
/// `installer` is the text of `scripts/install.sh`, `front` the text of the
/// CLI's `sibling.rs`, whose `LIBEXEC` constant is read as a parsed item, and
/// `documents` the (path, text) of each document that must name the directory
/// in a code span, alone or as the start of a path.
///
/// # Errors
/// Returns [`Error::Invalid`] naming the first disagreement, an unreadable
/// name or a document that does not name the directory.
pub fn check_sibling_directory(
    installer: &str,
    front: &str,
    documents: &[(&str, &str)],
) -> Result<(), Error> {
    let placed = list(installer, "libexec")?;
    let looked_in = libexec(front)?;
    require(
        placed.len() == 1 && placed.contains(&looked_in),
        format!(
            "the installer places the GPU sibling in {placed:?}, but the front looks in {looked_in:?}"
        ),
    )?;
    for (path, text) in documents {
        require(
            text.contains(&format!("`{looked_in}`")) || text.contains(&format!("`{looked_in}/")),
            format!("{path} does not name the {looked_in} directory"),
        )?;
    }
    Ok(())
}

/// The string value of the CLI's `LIBEXEC` constant in `front`.
fn libexec(front: &str) -> Result<String, Error> {
    let file = syn::parse_file(front)
        .map_err(|error| Error::Invalid(format!("the CLI's sibling.rs does not parse: {error}")))?;
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Const(constant) if constant.ident == LIBEXEC_CONSTANT => {
                match constant.expr.as_ref() {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) => Some(value.value()),
                    _ => None,
                }
            }
            _ => None,
        })
        .ok_or_else(|| {
            Error::Invalid(format!(
                "the CLI's sibling.rs declares no {LIBEXEC_CONSTANT} string constant"
            ))
        })
}

/// The words of the installer's one `NAME='…'` assignment.
fn list(installer: &str, name: &str) -> Result<BTreeSet<String>, Error> {
    let prefix = format!("{name}='");
    let mut values = installer
        .lines()
        .filter_map(|line| line.strip_prefix(prefix.as_str()));
    let (Some(value), None) = (values.next(), values.next()) else {
        return Err(Error::Invalid(format!(
            "the installer must assign {name} exactly once"
        )));
    };
    let value = value.strip_suffix('\'').ok_or_else(|| {
        Error::Invalid(format!(
            "the installer's {name} list is not one quoted line"
        ))
    })?;
    Ok(value.split_whitespace().map(str::to_owned).collect())
}

/// The tools in the first column of INSTALL.md's installed-tools table.
fn described_tools(guide: &str) -> Result<BTreeSet<String>, Error> {
    let (_, section) = guide
        .split_once(TOOLS_HEADING)
        .ok_or_else(|| Error::Invalid(format!("INSTALL.md has no \"{TOOLS_HEADING}\" section")))?;
    let section = section.split("\n## ").next().unwrap_or(section);
    Ok(section
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|cell| cell.split_once('`'))
        .map(|(tool, _)| tool.to_owned())
        .collect())
}

/// The packages whose manifest directories INSTALL.md's Cargo commands name.
fn cargo_packages(
    guide: &str,
    members: &BTreeMap<String, Package>,
) -> Result<BTreeSet<String>, Error> {
    guide
        .lines()
        .filter_map(|line| line.trim().strip_prefix(CARGO_INSTALL))
        .map(|rest| {
            let directory = rest.split_whitespace().next().unwrap_or_default();
            members
                .iter()
                .find(|(_, package)| package.directory == directory)
                .map(|(name, _)| name.clone())
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "INSTALL.md installs crates/{directory}, which is not a workspace package"
                    ))
                })
        })
        .collect()
}
