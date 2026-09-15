//! Match an admitted anonymous key without assigning a value to its wildcard.

use super::{Atom, Error, Reference, Symbol, Work, patterns};

fn matches(pattern: &Symbol, value: &Symbol, work: &mut Work<'_>) -> Result<bool, Error> {
    work.step(1)?;
    let Symbol::Tuple(items) = pattern else {
        unreachable!("tagged anonymous key");
    };
    match items.as_slice() {
        [Symbol::Number(0)] => Ok(true),
        [Symbol::Number(1), expected] => Ok(work
            .compare_reference(Reference::Symbol(expected), Reference::Symbol(value))?
            .is_eq()),
        [
            Symbol::Number(2),
            Symbol::Function {
                sign,
                name,
                arguments,
            },
        ] => {
            let Symbol::Function {
                sign: actual_sign,
                name: actual_name,
                arguments: actual,
            } = value
            else {
                return Ok(false);
            };
            work.step(name.as_str().len() as u128 + actual_name.as_str().len() as u128)?;
            if sign != actual_sign || name != actual_name {
                return Ok(false);
            }
            children(arguments, actual, work)
        }
        [Symbol::Number(3), Symbol::Tuple(arguments)] => {
            let Symbol::Tuple(actual) = value else {
                return Ok(false);
            };
            children(arguments, actual, work)
        }
        _ => unreachable!("compiler publishes only tagged anonymous key forms"),
    }
}
fn children(patterns: &[Symbol], values: &[Symbol], work: &mut Work<'_>) -> Result<bool, Error> {
    if patterns.len() != values.len() {
        return Ok(false);
    }
    for (pattern, value) in patterns.iter().zip(values) {
        if !matches(pattern, value, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn atom(pattern: &Symbol, atom: &Atom, work: &mut Work<'_>) -> Result<bool, Error> {
    let Symbol::Function { arguments, .. } = pattern else {
        unreachable!("atom key");
    };
    // ModelRows::symbol already selects the exact signed predicate range.
    for (pattern, value) in arguments.iter().zip(atom.values()) {
        let value = patterns::symbol(value, work)?;
        if !matches(pattern, &value, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A fully ground constructor pattern has the same key as its evaluated value.
/// Normalize in place using admitted vectors, without allocating another tree.
/// Wildcard-bearing nodes retain their explicit structural shape and tags.
fn normalize(pattern: &mut Symbol, work: &mut Work<'_>) -> Result<bool, Error> {
    work.step(1)?;
    let Symbol::Tuple(items) = pattern else {
        unreachable!("tagged pattern");
    };
    match items.first() {
        Some(Symbol::Number(0)) => return Ok(false),
        Some(Symbol::Number(1)) => return Ok(true),
        Some(Symbol::Number(2 | 3)) => {}
        _ => unreachable!("admitted pattern tag"),
    }
    let (Symbol::Function {
        arguments: children,
        ..
    }
    | Symbol::Tuple(children)) = &mut items[1]
    else {
        unreachable!("structural pattern payload");
    };
    let mut ground = true;
    for child in children.iter_mut() {
        ground &= normalize(child, work)?;
    }
    if ground {
        for child in children {
            work.step(1)?;
            let Symbol::Tuple(encoded) = child else {
                unreachable!("ground value tag");
            };
            let value = encoded
                .pop()
                .expect("ground pattern contains its complete value");
            *child = value;
        }
        items[0] = Symbol::Number(1);
    }
    Ok(ground)
}

pub(super) fn normalize_key(tuple: &mut Symbol, work: &mut Work<'_>) -> Result<(), Error> {
    let Symbol::Tuple(fields) = tuple else {
        unreachable!("aggregate key tuple");
    };
    let Symbol::Function { arguments, .. } = &mut fields[1] else {
        unreachable!("signed atom pattern key");
    };
    for argument in arguments {
        normalize(argument, work)?;
    }
    Ok(())
}
