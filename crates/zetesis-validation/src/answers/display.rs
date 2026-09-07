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
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for character in text.chars() {
        if quoted {
            atom.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
            atom.push(character);
        } else if character == '(' {
            depth = depth
                .checked_add(1)
                .ok_or_else(|| invalid(Issue::CountOverflow, "atom nesting overflow"))?;
            atom.push(character);
        } else if character == ')' {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| invalid(Issue::MalformedField, "unmatched atom parenthesis"))?;
            atom.push(character);
        } else if depth == 0 && (character.is_whitespace() || (comma_separated && character == ','))
        {
            if !atom.is_empty() {
                result.push(std::mem::take(&mut atom));
            }
        } else {
            atom.push(character);
        }
    }
    if quoted || depth != 0 {
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
