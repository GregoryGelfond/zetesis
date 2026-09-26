//! Direct bounded Symbol spelling, without synthetic source or reparsing.

use themelios_program::symbol::{Name, Sign};

use super::evaluate::{Work, terms};
use super::view::AtomChannel;
use super::{
    Cancellation, ConstructionLimits, Error, ErrorKind, Limits, Model, ObservationProgram,
    Rendered, Resource, Statistics, Symbol,
};
use zetesis_core::{ValueNodeRef, catalog::TermRef};

fn append(out: &mut String, text: &str, work: &mut Work<'_>) -> Result<(), Error> {
    work.step(text.len() as u128 + 1)?;
    work.check(
        Resource::OutputBytes,
        out.len() as u128 + text.len() as u128,
        work.limits.max_output_bytes as u128,
    )?;
    out.try_reserve(text.len())
        .map_err(|_| work.error(ErrorKind::Allocation))?;
    out.push_str(text);
    Ok(())
}
fn quoted(out: &mut String, text: &str, work: &mut Work<'_>) -> Result<(), Error> {
    append(out, "\"", work)?;
    for character in text.chars() {
        match character {
            '"' => append(out, "\\\"", work)?,
            '\\' => append(out, "\\\\", work)?,
            '\n' => append(out, "\\n", work)?,
            '\0' => return Err(work.error(ErrorKind::InvalidSymbol)),
            other => append(out, other.encode_utf8(&mut [0; 4]), work)?,
        }
    }
    append(out, "\"", work)
}
fn name(text: &str, output_bytes: usize, work: &mut Work<'_>) -> Result<(), Error> {
    work.step(text.len() as u128 + 1)?;
    work.check(
        Resource::OutputBytes,
        output_bytes as u128 + text.len() as u128,
        work.limits.max_output_bytes as u128,
    )?;
    Name::new(text.to_owned()).map_err(|_| work.error(ErrorKind::InvalidSymbol))?;
    Ok(())
}
fn scalar(out: &mut String, value: TermRef<'_>, work: &mut Work<'_>) -> Result<(), Error> {
    match value.descriptor() {
        ValueNodeRef::Infimum => append(out, "#inf", work),
        ValueNodeRef::Supremum => append(out, "#sup", work),
        ValueNodeRef::Number(number) => append(out, &number.to_string(), work),
        ValueNodeRef::Symbol(text) => {
            name(text, out.len(), work)?;
            append(out, text, work)
        }
        ValueNodeRef::String(text) => quoted(out, text, work),
        ValueNodeRef::Function { .. } | ValueNodeRef::Tuple { .. } => {
            work.check(
                Resource::Depth,
                value.depth() as u128,
                work.limits.max_symbol_depth as u128,
            )?;
            work.check(
                Resource::Nodes,
                value.expanded_nodes() as u128,
                work.limits.max_symbol_nodes as u128,
            )?;
            let mut text_bytes = 0_u128;
            let mut nodes = value.nodes();
            while let Some(node) = nodes.next_with(|| work.step(1))? {
                text_bytes += node.text_bytes() as u128;
                work.check(
                    Resource::Bytes,
                    text_bytes,
                    work.limits.max_symbol_bytes as u128,
                )?;
                work.step(node.text_bytes() as u128)?;
                match node {
                    ValueNodeRef::Symbol(text) | ValueNodeRef::Function { name: text, .. } => {
                        name(text, out.len(), work)?;
                    }
                    ValueNodeRef::String(text) if text.contains('\0') => {
                        return Err(work.error(ErrorKind::InvalidSymbol));
                    }
                    _ => {}
                }
            }
            let bytes = value.rendered_bytes();
            work.check(
                Resource::OutputBytes,
                out.len() as u128 + bytes as u128,
                work.limits.max_output_bytes as u128,
            )?;
            out.try_reserve_exact(bytes)
                .map_err(|_| work.error(ErrorKind::Allocation))?;
            value
                .write_with(out, work.construction.max_bytes, |count| work.step(count))
                .map_err(|error| match error {
                    zetesis_core::ValueWriteError::Stopped(error) => error,
                    zetesis_core::ValueWriteError::Storage(zetesis_core::ValueError::Limit {
                        observed,
                        limit,
                        ..
                    }) => work.error(ErrorKind::Limit {
                        resource: Resource::ConstructionBytes,
                        observed,
                        limit: limit as u128,
                    }),
                    zetesis_core::ValueWriteError::Storage(_)
                    | zetesis_core::ValueWriteError::Writer(_) => work.error(ErrorKind::Allocation),
                })
        }
    }
}
fn symbol(out: &mut String, value: &Symbol, work: &mut Work<'_>) -> Result<(), Error> {
    // Source templates are depth-capped, but a whole public model value can be
    // deeper. Iterative frames keep rendering safe at any admitted runtime depth.
    let mut frames: Vec<(&[Symbol], usize, bool)> = Vec::new();
    let mut current = value;
    loop {
        work.step(1)?;
        work.check(
            Resource::Depth,
            frames.len() as u128 + 1,
            work.limits.max_symbol_depth as u128,
        )?;
        let children = match current {
            Symbol::Infimum => {
                append(out, "#inf", work)?;
                None
            }
            Symbol::Supremum => {
                append(out, "#sup", work)?;
                None
            }
            Symbol::Number(number) => {
                append(out, &number.to_string(), work)?;
                None
            }
            Symbol::String(text) => {
                quoted(out, text, work)?;
                None
            }
            Symbol::Function {
                name,
                arguments,
                sign,
            } => {
                if *sign == Sign::Negative {
                    append(out, "-", work)?;
                }
                append(out, name.as_str(), work)?;
                if arguments.is_empty() {
                    None
                } else {
                    Some((arguments.as_slice(), false))
                }
            }
            Symbol::Tuple(arguments) => Some((arguments.as_slice(), true)),
        };
        if let Some((arguments, tuple)) = children {
            append(out, "(", work)?;
            if let Some(first) = arguments.first() {
                frames
                    .try_reserve_exact(1)
                    .map_err(|_| work.error(ErrorKind::Allocation))?;
                frames.push((arguments, 1, tuple));
                current = first;
                continue;
            }
            append(out, ")", work)?;
        }
        loop {
            let Some((arguments, next, tuple)) = frames.last_mut() else {
                return Ok(());
            };
            if let Some(argument) = arguments.get(*next) {
                append(out, ",", work)?;
                *next += 1;
                current = argument;
                break;
            }
            if *tuple && arguments.len() == 1 {
                append(out, ",", work)?;
            }
            append(out, ")", work)?;
            frames.pop();
        }
    }
}

pub(super) fn render(
    program: &ObservationProgram,
    model: &Model,
    selection: &crate::OutputSelection,
    limits: Limits,
    construction: ConstructionLimits,
    cancellation: &Cancellation,
) -> Result<Rendered, Error> {
    let mut work = Work {
        limits,
        construction,
        cancellation,
        statistics: Statistics::default(),
        local_bytes: 0,
        location: None,
    };
    let symbols = terms(program, model, &mut work)?;
    render_evaluated(model, AtomChannel::Policy(selection), &symbols, work)
}

/// Spell a previously evaluated term channel without evaluating it again.
pub(super) fn render_evaluated(
    model: &Model,
    selection: AtomChannel<'_>,
    symbols: &[Symbol],
    mut work: Work<'_>,
) -> Result<Rendered, Error> {
    work.location = None;
    let mut text = String::new();
    let mut first = true;
    for atom in model.atoms() {
        work.step(1 + atom.predicate().name().len() as u128)?;
        if !selection.try_includes(atom, |units| work.step(units))? {
            continue;
        }
        if !first {
            append(&mut text, " ", &mut work)?;
        }
        first = false;
        if atom.predicate().sign() == zetesis_core::Sign::Negative {
            append(&mut text, "-", &mut work)?;
        }
        name(atom.predicate().name(), text.len(), &mut work)?;
        append(&mut text, atom.predicate().name(), &mut work)?;
        if !atom.values().is_empty() {
            append(&mut text, "(", &mut work)?;
            for (index, value) in atom.values().iter().enumerate() {
                if index != 0 {
                    append(&mut text, ",", &mut work)?;
                }
                scalar(&mut text, value, &mut work)?;
            }
            append(&mut text, ")", &mut work)?;
        }
    }
    for value in symbols {
        if !first {
            append(&mut text, " ", &mut work)?;
        }
        first = false;
        symbol(&mut text, value, &mut work)?;
    }
    work.step(0)?;
    Ok(Rendered {
        text,
        statistics: work.statistics,
    })
}
