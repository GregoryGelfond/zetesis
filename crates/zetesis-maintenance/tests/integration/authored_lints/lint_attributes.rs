//! Inspect authored attribute tokens, including literal macro templates.
//! Comments and string literals are opaque. This does not expand macros or
//! evaluate configuration predicates; every literal `cfg_attr` branch is checked.

use std::fmt;

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Meta, Token};

const MAX_TOKENS: usize = 1_000_000;
const MAX_DEPTH: usize = 128;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Suppression {
    pub line: usize,
    pub column: usize,
    pub level: &'static str,
    pub lint: &'static str,
}

impl fmt::Display for Suppression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}({}) is prohibited by the authored lint policy",
            self.line, self.column, self.level, self.lint
        )
    }
}

/// A finite token walk recognizes both outer and inner attributes. Unrelated
/// attribute payloads remain opaque except for any literal attribute tokens
/// nested in their macro input. Dynamic attribute names are not macro expansion.
pub(super) fn suppressions(source: &str) -> syn::Result<Vec<Suppression>> {
    let tokens = source
        .parse::<TokenStream>()
        .map_err(|error| syn::Error::new(Span::call_site(), error))?;
    let mut pending = vec![(tokens, 0)];
    let mut visited = 0;
    let mut found = Vec::new();
    while let Some((tokens, depth)) = pending.pop() {
        let mut prefix = Prefix::None;
        for token in tokens {
            visited += 1;
            if visited > MAX_TOKENS {
                return Err(syn::Error::new(token.span(), "attribute token limit"));
            }
            if let TokenTree::Group(group) = &token {
                if prefix != Prefix::None && group.delimiter() == Delimiter::Bracket {
                    attribute(group.stream(), &mut found)?;
                }
                if depth >= MAX_DEPTH {
                    return Err(syn::Error::new(group.span(), "attribute nesting limit"));
                }
                pending.push((group.stream(), depth + 1));
            }
            prefix = match token {
                TokenTree::Punct(mark) if mark.as_char() == '#' => Prefix::Outer,
                TokenTree::Punct(mark) if mark.as_char() == '!' && prefix == Prefix::Outer => {
                    Prefix::Inner
                }
                _ => Prefix::None,
            };
        }
    }
    found.sort();
    Ok(found)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prefix {
    None,
    Outer,
    Inner,
}

fn attribute(tokens: TokenStream, found: &mut Vec<Suppression>) -> syn::Result<()> {
    let Some(TokenTree::Ident(name)) = tokens.clone().into_iter().next() else {
        // A macro metavariable is not a literal attribute name. Its eventual
        // substitution is outside this authored-token audit's stated scope.
        return Ok(());
    };
    if !matches!(
        name.unraw().to_string().as_str(),
        "allow" | "expect" | "cfg_attr"
    ) {
        return Ok(());
    }
    let mut pending = vec![(syn::parse2::<Meta>(tokens)?, 0)];
    while let Some((meta, depth)) = pending.pop() {
        let Some(name) = meta.path().get_ident() else {
            continue;
        };
        let name = name.unraw().to_string();
        let Meta::List(list) = meta else {
            continue;
        };
        if name == "cfg_attr" {
            if depth >= MAX_DEPTH {
                return Err(syn::Error::new(
                    list.delimiter.span().join(),
                    "cfg_attr nesting limit",
                ));
            }
            let entries = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens)?;
            pending.extend(entries.into_iter().skip(1).map(|meta| (meta, depth + 1)));
        } else if let Some(level) = level(&name) {
            let entries = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens)?;
            for entry in entries {
                if let Meta::Path(path) = entry
                    && let Some(name) = path.get_ident()
                    && let Some(lint) = prohibited(&name.unraw().to_string())
                {
                    let start = name.span().start();
                    found.push(Suppression {
                        line: start.line,
                        column: start.column + 1,
                        level,
                        lint,
                    });
                }
            }
        }
    }
    Ok(())
}

fn level(name: &str) -> Option<&'static str> {
    match name {
        "allow" => Some("allow"),
        "expect" => Some("expect"),
        _ => None,
    }
}

fn prohibited(name: &str) -> Option<&'static str> {
    match name {
        "dead_code" => Some("dead_code"),
        "unused" => Some("unused"),
        "warnings" => Some("warnings"),
        _ => None,
    }
}
