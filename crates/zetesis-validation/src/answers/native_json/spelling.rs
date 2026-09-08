//! Bounded consumer spelling over core atoms; no source parsing or evaluation.
use std::fmt::{self, Write};

use themelios_program::symbol::Name;
use zetesis_core::{Atom, Sign, Value, ValueNode};

use crate::answers::{Error, Issue, invalid};

struct Size(usize);
impl Write for Size {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

pub(super) fn atom_bytes(atom: &Atom) -> Result<usize, Error> {
    let mut size = Size(0);
    write_atom(&mut size, atom)
        .map_err(|_| invalid(Issue::CountOverflow, "atom spelling byte count"))?;
    Ok(size.0)
}

pub(super) fn atom(atom: &Atom, bytes: usize) -> Result<String, Error> {
    validate_names(atom)?;
    let mut output = String::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| Error::Allocation)?;
    write_atom(&mut output, atom).map_err(|_| Error::Allocation)?;
    Ok(output)
}

fn validate_names(atom: &Atom) -> Result<(), Error> {
    name(atom.predicate().name())?;
    for value in atom.values() {
        validate_value(value)?;
    }
    Ok(())
}

fn validate_value(value: &Value) -> Result<(), Error> {
    match value {
        Value::Symbol(symbol) => name(symbol)?,
        Value::Structured(value) => {
            for node in value.nodes() {
                match node {
                    ValueNode::Symbol(symbol) | ValueNode::Function { name: symbol, .. } => {
                        name(symbol)?;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn name(value: &str) -> Result<(), Error> {
    // The containing atom's spelling bytes were admitted before this call.
    // Name::new temporarily holds this owned name and a same-length lexer Source;
    // both copies are bounded by that admitted atom size. No second lexer exists.
    value
        .len()
        .checked_mul(2)
        .ok_or_else(|| invalid(Issue::CountOverflow, "identifier validation text bytes"))?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| Error::Allocation)?;
    owned.push_str(value);
    Name::new(owned).map(|_| ()).map_err(Error::Identifier)
}

fn write_atom(output: &mut impl Write, atom: &Atom) -> fmt::Result {
    if atom.predicate().sign() == Sign::Negative {
        output.write_str("-")?;
    }
    output.write_str(atom.predicate().name())?;
    if !atom.values().is_empty() {
        output.write_str("(")?;
        for (index, value) in atom.values().iter().enumerate() {
            if index != 0 {
                output.write_str(",")?;
            }
            write_value(output, value)?;
        }
        output.write_str(")")?;
    }
    Ok(())
}

fn write_value(output: &mut impl Write, value: &Value) -> fmt::Result {
    match value {
        Value::Infimum => output.write_str("#inf"),
        Value::Supremum => output.write_str("#sup"),
        Value::Number(number) => write!(output, "{number}"),
        Value::Symbol(symbol) => output.write_str(symbol),
        Value::Structured(value) => write!(output, "{value}"),
        Value::String(value) => {
            // This is the core structural renderer's scalar spelling, so a
            // scalar string and the same string inside a tuple remain identical.
            output.write_str("\"")?;
            for character in value.chars() {
                match character {
                    '\\' => output.write_str("\\\\")?,
                    '"' => output.write_str("\\\"")?,
                    '\n' => output.write_str("\\n")?,
                    character => output.write_char(character)?,
                }
            }
            output.write_str("\"")
        }
    }
}

pub(super) fn value_bytes(value: &Value) -> Result<usize, Error> {
    let mut size = Size(0);
    write_value(&mut size, value)
        .map_err(|_| invalid(Issue::CountOverflow, "value spelling byte count"))?;
    Ok(size.0)
}

pub(super) fn value(value: &Value, bytes: usize) -> Result<String, Error> {
    validate_value(value)?;
    let mut output = String::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| Error::Allocation)?;
    write_value(&mut output, value).map_err(|_| Error::Allocation)?;
    Ok(output)
}
