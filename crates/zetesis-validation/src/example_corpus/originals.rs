//! Explicit original-byte derivation audit; normal loading never interprets tags.

use std::path::Path;

use super::{
    Annotation, Contract, Corpus, Edit, Error, Family, Limits, Resource, Satisfiability, ceiling,
    document, files,
};
use crate::answers;

/// Check every original hash, exact comment deletion, recorded edit, include coordinate and
/// typed-contract translation against the retained cleaned sources.
/// This reads the preserved original tree only when explicitly requested.
/// Only `limits.source_bytes` applies: the supplied `Corpus` has already passed
/// its manifest, source-count, retained-byte and contract-symbol admission limits.
///
/// # Errors
/// Refuses changed originals, deletion spans outside ordinary leading comments,
/// changed includes, mismatched contracts, unconfined paths or source-byte limits.
pub fn verify_originals(
    corpus: &Corpus,
    original_root: &Path,
    limits: Limits,
) -> Result<(), Error> {
    let root = files::canonical(original_root)?;
    let license = files::read(
        &files::confined(&root, "LICENSE")?,
        limits.source_bytes,
        Resource::SourceBytes,
    )?;
    files::digest("LICENSE", &license, corpus.license_sha256())?;
    for source in corpus.files() {
        let bytes = files::read(
            &files::confined(&root, source.path())?,
            limits.source_bytes,
            Resource::SourceBytes,
        )?;
        files::digest(source.path(), &bytes, source.original_sha256())?;
        let original = String::from_utf8(bytes).map_err(Error::Utf8)?;
        let stripped = remove_annotations(&original, source.removed_annotations())?;
        let cleaned = apply_edits(&stripped, source.edits(), limits.source_bytes)?;
        if cleaned != source.source() {
            return Err(Error::Contract(format!(
                "recorded source derivation differs for {}",
                source.path()
            )));
        }
        for include in source.includes() {
            let expected = format!("#include \"{}\".", include.spelling());
            if include
                .line()
                .checked_sub(1)
                .and_then(|index| original.lines().nth(index))
                .map(str::trim)
                != Some(expected.as_str())
            {
                return Err(Error::Contract(format!(
                    "original include coordinate differs for {}",
                    source.path()
                )));
            }
        }
        if let Some(case) = corpus
            .cases()
            .iter()
            .find(|case| case.path() == source.path())
        {
            let translated = translate(source.removed_annotations())?;
            if &translated != case.contract() {
                return Err(Error::Contract(format!(
                    "original display contract differs for {}",
                    source.path()
                )));
            }
        } else if !source.removed_annotations().is_empty() {
            return Err(Error::Contract(format!(
                "unowned annotations in {}",
                source.path()
            )));
        }
    }
    Ok(())
}

fn apply_edits(source: &str, edits: &[Edit], limit: usize) -> Result<String, Error> {
    let mut previous_end = 0;
    let mut length = source.len() as u128;
    for edit in edits {
        if edit.start_byte < previous_end
            || edit.start_byte >= edit.end_byte
            || source.get(edit.start_byte..edit.end_byte) != Some(edit.before.as_str())
        {
            return Err(invalid(
                "source edits overlap or differ from the recorded bytes",
            ));
        }
        length = length - edit.before.len() as u128 + edit.after.len() as u128;
        previous_end = edit.end_byte;
    }
    ceiling(Resource::SourceBytes, length, limit)?;
    let capacity = usize::try_from(length)
        .map_err(|_| invalid("source derivation exceeds addressable storage"))?;
    let mut result = String::new();
    result
        .try_reserve_exact(capacity)
        .map_err(Error::Allocation)?;
    previous_end = 0;
    for edit in edits {
        // The validation pass established ordered, disjoint UTF-8 boundaries.
        result.push_str(&source[previous_end..edit.start_byte]);
        result.push_str(&edit.after);
        previous_end = edit.end_byte;
    }
    result.push_str(&source[previous_end..]);
    Ok(result)
}

pub(super) fn remove_annotations(
    original: &str,
    annotations: &[Annotation],
) -> Result<String, Error> {
    let mut result = String::with_capacity(original.len());
    let mut previous_end = 0;
    for annotation in annotations {
        let before = original
            .get(..annotation.start_byte)
            .ok_or_else(|| invalid("annotation start is outside source"))?;
        let removed = original
            .get(annotation.start_byte..annotation.end_byte)
            .ok_or_else(|| invalid("annotation span is outside source"))?;
        // This pinned corpus places all contracts in a leading ordinary-comment
        // header. Requiring that shape excludes strings and block comments without
        // introducing a second ASP lexer into the provenance tooling.
        let leading_comments = before.lines().all(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with('%') && !line.starts_with("%*")
        });
        let line_boundary = before.is_empty() || before.ends_with('\n');
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let without_newline = removed.strip_suffix('\n').unwrap_or(removed);
        let spelling = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        if annotation.start_byte < previous_end
            || annotation.start_byte >= annotation.end_byte
            || !leading_comments
            || !line_boundary
            || line != annotation.line
            || !removed.ends_with('\n') && annotation.end_byte != original.len()
            || spelling.contains(['\n', '\r'])
            || spelling != annotation.source
            || !spelling.trim_start().starts_with("% @")
        {
            return Err(invalid(
                "annotation coordinate is not an ordinary leading comment line",
            ));
        }
        result.push_str(
            original
                .get(previous_end..annotation.start_byte)
                .ok_or_else(|| invalid("annotation spans overlap"))?,
        );
        previous_end = annotation.end_byte;
    }
    result.push_str(
        original
            .get(previous_end..)
            .ok_or_else(|| invalid("annotation end is outside source"))?,
    );
    Ok(result)
}

fn invalid(detail: &str) -> Error {
    Error::Contract(detail.into())
}
fn braces(text: &str) -> Result<&str, Error> {
    text.strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map(str::trim)
        .ok_or_else(|| invalid("annotation requires braces"))
}
fn symbols(text: &str) -> Result<Vec<String>, Error> {
    answers::split_display(braces(text)?, true, answers::Limits::for_bytes(text.len()))
        .map_err(|error| Error::Contract(error.to_string()))
}
fn set<T: PartialEq>(slot: &mut Option<T>, value: T) -> Result<(), Error> {
    if slot.as_ref().is_some_and(|previous| previous != &value) {
        return Err(invalid("conflicting annotation contracts"));
    }
    *slot = Some(value);
    Ok(())
}

pub(super) fn translate(annotations: &[Annotation]) -> Result<Contract, Error> {
    let mut satisfiability = None;
    let mut family = None;
    let mut model_count = None;
    let mut cost = None;
    let mut witnesses = Vec::new();
    let mut required_symbols = Vec::new();
    let mut notes = Vec::new();
    for annotation in annotations {
        let text = annotation
            .source
            .trim_start()
            .strip_prefix("% @")
            .ok_or_else(|| invalid("unsupported annotation comment"))?;
        let (tag, argument) = text
            .split_once(char::is_whitespace)
            .ok_or_else(|| invalid("annotation argument is missing"))?;
        let argument = argument.trim();
        match tag {
            "expect" => set(
                &mut satisfiability,
                match argument {
                    "sat" => Satisfiability::Sat,
                    "unsat" => Satisfiability::Unsat,
                    _ => return Err(invalid("unsupported satisfiability annotation")),
                },
            )?,
            "count" => {
                let (scope, count) = argument
                    .strip_prefix("optimal ")
                    .map_or((Family::All, argument), |count| {
                        (Family::Optimal, count.trim())
                    });
                set(&mut family, scope)?;
                set(
                    &mut model_count,
                    count
                        .parse::<u64>()
                        .map_err(|error| Error::Contract(error.to_string()))?,
                )?;
            }
            "cost" => {
                set(&mut family, Family::Optimal)?;
                set(
                    &mut cost,
                    answers::parse_costs(
                        braces(argument)?,
                        answers::Limits::for_bytes(argument.len()),
                    )
                    .map_err(|error| Error::Contract(error.to_string()))?,
                )?;
            }
            "model" | "optimal" => {
                set(
                    &mut family,
                    if tag == "model" {
                        Family::All
                    } else {
                        Family::Optimal
                    },
                )?;
                witnesses.push(symbols(argument)?);
            }
            "cautious" => {
                set(&mut family, Family::Optimal)?;
                let argument = argument
                    .strip_prefix("optimal ")
                    .ok_or_else(|| invalid("unsupported required-symbol annotation scope"))?;
                required_symbols.extend(symbols(argument.trim())?);
            }
            "note" => notes.push(argument.to_owned()),
            _ => return Err(invalid("unsupported original annotation")),
        }
    }
    required_symbols.sort();
    let contract = Contract {
        satisfiability: satisfiability
            .ok_or_else(|| invalid("missing original satisfiability contract"))?,
        family: family.unwrap_or(Family::All),
        model_count,
        cost,
        witnesses,
        required_symbols,
        notes,
    };
    document::validate_contract(&contract)?;
    Ok(contract)
}

#[cfg(test)]
#[path = "../../tests/support/example_originals.rs"]
mod tests;
