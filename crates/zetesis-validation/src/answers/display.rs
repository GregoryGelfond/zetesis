//! Printed-symbol multiset and reported-cost tokenization.
use super::{Error, Issue, Limits, Model, Resource, check, invalid};

pub(super) fn integers(text: &str, limits: Limits) -> Result<Vec<i64>, Error> {
    check(
        Resource::CostDimensions,
        limits.max_cost_dimensions,
        text.split_whitespace().count(),
    )?;
    text.split_whitespace()
        .map(|value| {
            value.parse().map_err(|_| {
                invalid(
                    Issue::MalformedField,
                    "cost integer outside signed 64-bit report representation",
                )
            })
        })
        .collect()
}

pub(super) fn split_atoms(text: &str, comma_separated: bool) -> Result<Model, Error> {
    let mut result = Model::new();
    let mut atom = String::new();
    let mut frame = Frame::default();
    for character in text.chars() {
        if frame.complete() && (character.is_whitespace() || (comma_separated && character == ','))
        {
            if !atom.is_empty() {
                result.push(std::mem::take(&mut atom));
            }
        } else {
            atom.push(character);
        }
        frame.advance(character)?;
    }
    if !frame.complete() {
        return Err(invalid(
            Issue::MalformedField,
            "unterminated quoted atom or tuple",
        ));
    }
    if !atom.is_empty() {
        result.push(atom);
    }
    result.sort_unstable();
    Ok(result)
}

/// Read one bounded display frame before interpreting later lines as metadata.
/// Every character advances the balance once, then the completed frame is
/// tokenized once. Multiline quoted symbols therefore remain linear in bytes.
pub(super) fn model(lines: &mut std::str::Split<'_, char>) -> Result<Model, Error> {
    let mut text = String::new();
    let mut frame = Frame::default();
    loop {
        let line = lines
            .next()
            .ok_or_else(|| invalid(Issue::MissingField, "missing or unterminated native model"))?;
        if !text.is_empty() {
            text.push('\n');
            frame.advance('\n')?;
        }
        for character in line.chars() {
            frame.advance(character)?;
        }
        text.push_str(line);
        if frame.complete() {
            return split_atoms(&text, false);
        }
    }
}

#[derive(Default)]
struct Frame {
    depth: usize,
    quoted: bool,
    escaped: bool,
}

impl Frame {
    const fn complete(&self) -> bool {
        !self.quoted && self.depth == 0
    }

    fn advance(&mut self, character: char) -> Result<(), Error> {
        if self.quoted {
            if self.escaped {
                self.escaped = false;
            } else if character == '\\' {
                self.escaped = true;
            } else if character == '"' {
                self.quoted = false;
            }
        } else {
            match character {
                '"' => self.quoted = true,
                '(' => {
                    self.depth = self
                        .depth
                        .checked_add(1)
                        .ok_or_else(|| invalid(Issue::CountOverflow, "atom nesting overflow"))?;
                }
                ')' => {
                    self.depth = self.depth.checked_sub(1).ok_or_else(|| {
                        invalid(Issue::MalformedField, "unmatched atom parenthesis")
                    })?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
