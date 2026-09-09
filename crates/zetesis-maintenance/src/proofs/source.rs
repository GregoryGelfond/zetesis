//! Restricted declaration inventory; this deliberately does not parse Lean.
use crate::{Error, require};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

pub(super) const NAME: &str = r"[A-Za-z_][A-Za-z_0-9']*(?:\.[A-Za-z_][A-Za-z_0-9']*)*";
static OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(namespace|section)(?:\s+({NAME}))?$")).unwrap());
static END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^end(?:\s+({NAME}))?$")).unwrap());
static THEOREM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^theorem\s+({NAME})(?:$|[\s:(\[{{])")).unwrap());
static FORBIDDEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:lemma|axiom|sorry|admit|native_decide|macro|elab|syntax|initialize|builtin_initialize|export)\b").unwrap()
});
static SCOPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:theorem|namespace|section|end)\b").unwrap());
static THEOREM_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\btheorem\b").unwrap());

/// A declaration location recognized by the restricted source convention.
/// Recognition is not proof-kernel acceptance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Declaration {
    pub(super) name: String,
    pub(super) file: String,
    pub(super) line: usize,
}

impl Declaration {
    /// Fully qualified ASCII theorem name, including enclosing namespaces.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Canonical path relative to the proof-library root.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }
    /// One-based source line containing the theorem declaration.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }
}

/// Mask comments and strings without changing byte offsets or newlines.
/// The cursor advances at least one byte; nesting cannot exceed input length.
pub(super) fn mask(source: &str) -> Result<String, Error> {
    let input = source.as_bytes();
    let mut output = input.to_vec();
    let mut index = 0;
    let mut depth = 0;
    let mut string = false;
    while index < input.len() {
        let pair = input.get(index..index + 2);
        if depth > 0 {
            if pair == Some(b"/-") {
                depth += 1;
            } else if pair == Some(b"-/") {
                depth -= 1;
            } else {
                blank(&mut output[index]);
                index += 1;
                continue;
            }
            output[index..index + 2].fill(b' ');
            index += 2;
        } else if string {
            blank(&mut output[index]);
            if input[index] == b'\\' {
                index += 1;
                require(index < input.len(), "unterminated string escape")?;
                blank(&mut output[index]);
            } else if input[index] == b'"' {
                string = false;
            }
            index += 1;
        } else if pair == Some(b"/-") {
            depth = 1;
            output[index..index + 2].fill(b' ');
            index += 2;
        } else if pair == Some(b"--") {
            while index < input.len() && input[index] != b'\n' {
                output[index] = b' ';
                index += 1;
            }
        } else if input[index] == b'"' {
            string = true;
            output[index] = b' ';
            index += 1;
        } else {
            index += 1;
        }
    }
    require(depth == 0 && !string, "unterminated comment or string")?;
    Ok(String::from_utf8(output).expect("whole comment/string bytes replaced by ASCII spaces"))
}
fn blank(byte: &mut u8) {
    if *byte != b'\n' {
        *byte = b' ';
    }
}

pub(super) fn declarations(file: &str, source: &str) -> Result<Vec<Declaration>, Error> {
    let mut stack: Vec<(bool, Option<String>)> = Vec::new();
    let mut entries = Vec::new();
    for (index, line) in mask(source)?.lines().enumerate() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let location = format!("{file}:{}", index + 1);
        require(
            !FORBIDDEN.is_match(text) && !text.starts_with(['#', '@']),
            format!("unsupported source convention at {location}"),
        )?;
        if let Some(opening) = OPEN.captures(text) {
            let namespace = &opening[1] == "namespace";
            let name = opening.get(2).map(|name| name.as_str().to_owned());
            require(
                !namespace || name.is_some(),
                format!("unnamed namespace at {location}"),
            )?;
            stack.push((namespace, name));
        } else if let Some(closing) = END.captures(text) {
            let (_, name) = stack
                .pop()
                .ok_or_else(|| Error::Invalid(format!("unmatched end at {location}")))?;
            require(
                closing
                    .get(1)
                    .is_none_or(|value| Some(value.as_str()) == name.as_deref()),
                format!("mismatched end at {location}"),
            )?;
        } else if let Some(theorem) = THEOREM.captures(text) {
            require(
                line.starts_with("theorem ") && THEOREM_WORD.find_iter(text).count() == 1,
                format!("unsupported theorem declaration at {location}"),
            )?;
            let mut names: Vec<&str> = stack
                .iter()
                .filter_map(|(namespace, name)| if *namespace { name.as_deref() } else { None })
                .collect();
            require(
                !names.is_empty(),
                format!("theorem outside namespace at {location}"),
            )?;
            names.push(&theorem[1]);
            entries.push(Declaration {
                name: names.join("."),
                file: file.into(),
                line: index + 1,
            });
        } else {
            require(
                !SCOPE.is_match(text),
                format!("unsupported declaration/scope syntax at {location}"),
            )?;
        }
    }
    require(stack.is_empty(), format!("unclosed scope in {file}"))?;
    Ok(entries)
}
