//! One punctuation and escaping state machine for admitted preorder descriptions.

use std::fmt::{self, Write};

use crate::{Sign, ValueError, ValueNodeRef, ValueResource};

pub(crate) type Frame = (usize, bool, bool);

/// A borrowed spelling operation stopped after an optional output prefix.
#[derive(Debug)]
pub enum ValueWriteError<E> {
    /// The explicit temporary frame-storage allowance or allocation was refused.
    Storage(ValueError),
    /// The caller refused before the next navigation, frame or output operation.
    Stopped(E),
    /// The supplied formatter rejected an output fragment.
    Writer(fmt::Error),
}
impl<E: fmt::Display> fmt::Display for ValueWriteError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => error.fmt(f),
            Self::Stopped(error) => error.fmt(f),
            Self::Writer(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ValueWriteError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Stopped(error) => Some(error),
            Self::Writer(error) => Some(error),
        }
    }
}

pub(crate) fn reserve<E>(
    frames: &mut Vec<Frame>,
    node: ValueNodeRef<'_>,
    limit: usize,
    before: &mut impl FnMut(u128) -> Result<(), E>,
) -> Result<(), ValueWriteError<E>> {
    before(1).map_err(ValueWriteError::Stopped)?;
    if !opens(node) || frames.len() < frames.capacity() {
        return Ok(());
    }
    let target = frames
        .len()
        .saturating_add(1)
        .max(frames.capacity().saturating_mul(2))
        .max(4);
    let old = frames.capacity() as u128 * size_of::<Frame>() as u128;
    bound(old + target as u128 * size_of::<Frame>() as u128, limit)?;
    // Relocation work is admitted even when the allocator can grow in place.
    before(frames.len() as u128 + 1).map_err(ValueWriteError::Stopped)?;
    frames
        .try_reserve_exact(target - frames.len())
        .map_err(|_| ValueWriteError::Storage(ValueError::Allocation))?;
    bound(
        old + frames.capacity() as u128 * size_of::<Frame>() as u128,
        limit,
    )
}
fn bound<E>(observed: u128, limit: usize) -> Result<(), ValueWriteError<E>> {
    if observed > limit as u128 {
        Err(ValueWriteError::Storage(ValueError::Limit {
            resource: ValueResource::Bytes,
            observed,
            limit,
        }))
    } else {
        Ok(())
    }
}
fn arity(node: ValueNodeRef<'_>) -> usize {
    match node {
        ValueNodeRef::Function { arity, .. } | ValueNodeRef::Tuple { arity } => arity,
        _ => 0,
    }
}
fn opens(node: ValueNodeRef<'_>) -> bool {
    arity(node) != 0 || matches!(node, ValueNodeRef::Tuple { .. })
}
fn emit<E>(
    output: &mut impl Write,
    text: &str,
    before: &mut impl FnMut(u128) -> Result<(), E>,
) -> Result<(), ValueWriteError<E>> {
    before(text.len() as u128 + 1).map_err(ValueWriteError::Stopped)?;
    output.write_str(text).map_err(ValueWriteError::Writer)
}

/// Caller reserves a frame before a constructor. All inputs describe an admitted
/// finite tree; this operation spells it and does not establish shape validity.
pub(crate) fn node<E>(
    node: ValueNodeRef<'_>,
    frames: &mut Vec<Frame>,
    output: &mut impl Write,
    before: &mut impl FnMut(u128) -> Result<(), E>,
) -> Result<(), ValueWriteError<E>> {
    before(1).map_err(ValueWriteError::Stopped)?;
    if let Some((left, first, _)) = frames.last_mut() {
        if !*first {
            emit(output, ",", before)?;
        }
        *first = false;
        *left -= 1;
    }
    match node {
        ValueNodeRef::Infimum => emit(output, "#inf", before)?,
        ValueNodeRef::Supremum => emit(output, "#sup", before)?,
        ValueNodeRef::Number(number) => {
            // i32's spelling fits this stack buffer, including its sign.
            let mut bytes = [0_u8; 11];
            let mut at = bytes.len();
            let mut magnitude = number.unsigned_abs();
            loop {
                at -= 1;
                bytes[at] = b'0' + u8::try_from(magnitude % 10).expect("one decimal digit");
                magnitude /= 10;
                if magnitude == 0 {
                    break;
                }
            }
            if number < 0 {
                at -= 1;
                bytes[at] = b'-';
            }
            emit(
                output,
                std::str::from_utf8(&bytes[at..]).expect("decimal ASCII"),
                before,
            )?;
        }
        ValueNodeRef::Symbol(text) => emit(output, text, before)?,
        ValueNodeRef::String(text) => {
            emit(output, "\"", before)?;
            let mut characters = text.chars();
            loop {
                before(1).map_err(ValueWriteError::Stopped)?;
                let Some(character) = characters.next() else {
                    break;
                };
                match character {
                    '\\' => emit(output, "\\\\", before)?,
                    '"' => emit(output, "\\\"", before)?,
                    '\n' => emit(output, "\\n", before)?,
                    other => emit(output, other.encode_utf8(&mut [0; 4]), before)?,
                }
            }
            emit(output, "\"", before)?;
        }
        ValueNodeRef::Function { name, sign, .. } => {
            if sign == Sign::Negative {
                emit(output, "-", before)?;
            }
            emit(output, name, before)?;
        }
        ValueNodeRef::Tuple { .. } => {}
    }
    if opens(node) {
        emit(output, "(", before)?;
        before(1).map_err(ValueWriteError::Stopped)?;
        if frames.len() == frames.capacity() {
            return Err(ValueWriteError::Storage(ValueError::Allocation));
        }
        frames.push((
            arity(node),
            true,
            matches!(node, ValueNodeRef::Tuple { arity: 1 }),
        ));
    }
    loop {
        before(1).map_err(ValueWriteError::Stopped)?;
        if !matches!(frames.last(), Some((0, _, _))) {
            break;
        }
        if let Some((_, _, singleton)) = frames.pop() {
            if singleton {
                emit(output, ",", before)?;
            }
            emit(output, ")", before)?;
        }
    }
    Ok(())
}
