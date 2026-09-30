//! Restricted decoding of preserved assertion excerpts, not C++ source files.
//!
//! Only the pinned REQUIRE/solve literal forms and helper output are accepted.
//! Every cursor advances within its manifest-bounded excerpt; balanced delimiters
//! use an explicit stack. Decoding retains at most linear excerpt-sized storage.

use super::Error;

pub(super) fn trivia(text: &str, mut offset: usize) -> Result<usize, Error> {
    let bytes = text.as_bytes();
    loop {
        while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
            offset += 1;
        }
        if text[offset..].starts_with("//") {
            offset = text[offset..]
                .find('\n')
                .map_or(text.len(), |end| offset + end + 1);
        } else if text[offset..].starts_with("/*") {
            offset += 2;
            offset += text[offset..]
                .find("*/")
                .ok_or_else(|| Error::Literal("unterminated comment".into()))?
                + 2;
        } else {
            return Ok(offset);
        }
    }
}
fn string_end(text: &str, start: usize) -> Result<usize, Error> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return Err(Error::Literal("expected ordinary string literal".into()));
    }
    let mut offset = start + 1;
    while let Some(&byte) = bytes.get(offset) {
        match byte {
            b'"' => return Ok(offset + 1),
            b'\\' => {
                offset += 1;
                if !matches!(bytes.get(offset), Some(b'\\' | b'"' | b'n' | b'r' | b't')) {
                    return Err(Error::Literal("escape outside declared subset".into()));
                }
            }
            b'\n' | b'\r' => {
                return Err(Error::Literal(
                    "literal line break outside declared subset".into(),
                ));
            }
            _ => {}
        }
        offset += 1;
    }
    Err(Error::Literal("unterminated string".into()))
}
fn literal_prefix(text: &str) -> Result<(String, usize), Error> {
    let mut offset = trivia(text, 0)?;
    let mut output = String::new();
    let mut found = false;
    while text.as_bytes().get(offset) == Some(&b'"') {
        let end = string_end(text, offset)?;
        let value: String = serde_json::from_str(&text[offset..end])?;
        output.push_str(&value);
        found = true;
        offset = trivia(text, end)?;
    }
    if !found {
        return Err(Error::Literal("empty literal sequence".into()));
    }
    Ok((output, offset))
}
pub(super) fn literals(text: &str) -> Result<String, Error> {
    let (output, end) = literal_prefix(text)?;
    if end != text.len() {
        return Err(Error::Literal("trailing literal expression".into()));
    }
    Ok(output)
}
/// Balanced delimiters and top-level commas, excluding strings/comments.
pub(super) fn balanced(text: &str, opening: usize) -> Result<(usize, Vec<usize>), Error> {
    if !matches!(text.as_bytes().get(opening), Some(b'(' | b'[' | b'{')) {
        return Err(Error::Literal("expected opening delimiter".into()));
    }
    let mut stack = Vec::new();
    let mut separators = Vec::new();
    let mut offset = opening;
    let bytes = text.as_bytes();
    while offset < bytes.len() {
        offset = trivia(text, offset)?;
        let Some(&byte) = bytes.get(offset) else {
            break;
        };
        match byte {
            b'"' => {
                offset = string_end(text, offset)?;
                continue;
            }
            b'(' => stack.push(b')'),
            b'[' => stack.push(b']'),
            b'{' => stack.push(b'}'),
            b')' | b']' | b'}' => {
                if stack.pop() != Some(byte) {
                    return Err(Error::Literal("mismatched delimiters".into()));
                }
                if stack.is_empty() {
                    return Ok((offset, separators));
                }
            }
            b',' if stack.len() == 1 => separators.push(offset),
            b'\'' => {
                return Err(Error::Literal(
                    "character literal outside declared subset".into(),
                ));
            }
            byte if !byte.is_ascii() => {
                return Err(Error::Literal("non-ASCII token outside a string".into()));
            }
            _ => {}
        }
        offset += 1;
    }
    Err(Error::Literal("unterminated expression".into()))
}
pub(super) fn assertion(text: &str) -> Result<(String, Vec<String>, String), Error> {
    let text = text
        .strip_prefix("REQUIRE")
        .ok_or_else(|| Error::Literal("expected REQUIRE assertion".into()))?;
    let opening = trivia(text, 0)?;
    if text.as_bytes().get(opening) != Some(&b'(') {
        return Err(Error::Literal("expected REQUIRE opening".into()));
    }
    let (end, _) = balanced(text, opening)?;
    if trivia(text, end + 1)? != text.len() {
        return Err(Error::Literal("trailing assertion text".into()));
    }
    let inner = &text[opening + 1..end];
    let (expected, end) = literal_prefix(inner)?;
    let right = inner[end..]
        .strip_prefix("==")
        .ok_or_else(|| Error::Literal("expected equality assertion".into()))?;
    let right = right
        .trim()
        .strip_prefix("IO::to_string")
        .ok_or_else(|| Error::Literal("expected IO::to_string".into()))?;
    let outer = trivia(right, 0)?;
    let (outer_end, _) = balanced(right, outer)?;
    if trivia(right, outer_end + 1)? != right.len() {
        return Err(Error::Literal("trailing helper text".into()));
    }
    let call = right[outer + 1..outer_end].trim();
    let call = call
        .strip_prefix("solve")
        .ok_or_else(|| Error::Literal("expected solve helper".into()))?;
    let opening = trivia(call, 0)?;
    let (end, separators) = balanced(call, opening)?;
    if trivia(call, end + 1)? != call.len() {
        return Err(Error::Literal("trailing solve expression".into()));
    }
    let mut edges = vec![opening];
    edges.extend(separators);
    edges.push(end);
    let args: Vec<_> = edges
        .windows(2)
        .map(|pair| call[pair[0] + 1..pair[1]].trim().to_owned())
        .collect();
    if args.is_empty() || args.len() > 2 {
        return Err(Error::Literal(
            "expected source plus optional prefix set".into(),
        ));
    }
    let source = literals(&args[0])?;
    Ok((source, args[1..].to_vec(), expected))
}
pub(super) fn prefixes(arguments: &[String]) -> Result<Vec<String>, Error> {
    let [text] = arguments else {
        return if arguments.is_empty() {
            Ok(vec![String::new()])
        } else {
            Err(Error::Literal(
                "objective-bound helper is outside selected contract".into(),
            ))
        };
    };
    let text = text.trim();
    if !text.starts_with('{') {
        return Err(Error::Literal("expected prefix initializer".into()));
    }
    let (end, separators) = balanced(text, 0)?;
    if end + 1 != text.len() {
        return Err(Error::Literal("trailing prefix initializer".into()));
    }
    if text[1..end].trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut edges = vec![0];
    edges.extend(separators);
    edges.push(end);
    edges
        .windows(2)
        .map(|pair| literals(&text[pair[0] + 1..pair[1]]))
        .collect()
}
pub(super) fn helper_models(text: &str) -> Result<Vec<Vec<String>>, Error> {
    if text.contains('"') || !text.starts_with("([") {
        return Err(Error::Literal(
            "helper output outside selected subset".into(),
        ));
    }
    let (end, _) = balanced(text, 1)?;
    let mut offset = 2;
    let mut models = Vec::new();
    while offset < end {
        if text.as_bytes().get(offset) != Some(&b'[') {
            return Err(Error::Literal("expected helper model".into()));
        }
        let (stop, separators) = balanced(text, offset)?;
        let mut edges = vec![offset];
        edges.extend(separators);
        edges.push(stop);
        let mut model: Vec<_> = edges
            .windows(2)
            .filter_map(|pair| {
                let atom = text[pair[0] + 1..pair[1]].trim();
                (!atom.is_empty()).then(|| atom.to_owned())
            })
            .collect();
        model.sort();
        models.push(model);
        offset = stop + 1;
        if offset < end {
            if text.as_bytes().get(offset) != Some(&b',') {
                return Err(Error::Literal("expected model separator".into()));
            }
            offset += 1;
        }
    }
    if !text[end + 1..].starts_with(',') || !text.ends_with(')') {
        return Err(Error::Literal(
            "expected retained helper diagnostics".into(),
        ));
    }
    models.sort();
    Ok(models)
}

#[cfg(test)]
mod tests;
