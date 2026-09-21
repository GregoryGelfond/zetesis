//! Named closure capacity and fallible pending tuple copies.
//!
//! A moved tuple's nested payload is charged once, first as pending storage and
//! then as catalog storage. Shared structural buffers are charged per occurrence.
//! Tree nodes, allocator metadata, Arc counters and unrelated frames are outside
//! this named allowance; no inaccessible container node size is guessed.

use std::mem::size_of;

use zetesis_core::{Atom, AtomKey, Predicate, Value};

use super::super::Work;
use crate::Stop;

pub(in crate::oracle) fn admit(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    work.cancellation.poll()?;
    if bytes > work.limits.max_closure_bytes as u128 {
        return Err(Stop::StorageLimit);
    }
    Ok(())
}

/// Record capacity already admitted or acquired, including allocator slack on a
/// failed operation. This does not turn a refusal into successful admission.
pub(in crate::oracle) fn record(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    let bytes = usize::try_from(bytes).map_err(|_| Stop::StorageLimit)?;
    work.statistics.peak_closure_bytes = work.statistics.peak_closure_bytes.max(bytes);
    Ok(())
}

pub(in crate::oracle) fn after_reservation(work: &mut Work<'_>, bytes: u128) -> Result<(), Stop> {
    record(work, bytes)?;
    if bytes > work.limits.max_closure_bytes as u128 {
        Err(Stop::StorageLimit)
    } else {
        Ok(())
    }
}

pub(in crate::oracle) fn atom_bytes(atom: &Atom, work: &mut Work<'_>) -> Result<u128, Stop> {
    for value in atom.values() {
        inspect(value, work)?;
    }
    (size_of::<Atom>() as u128)
        .checked_add(
            atom.checked_payload_capacity_bytes()
                .ok_or(Stop::StorageLimit)?,
        )
        .ok_or(Stop::StorageLimit)
}

fn inspect(value: &Value, work: &mut Work<'_>) -> Result<(), Stop> {
    work.tick()?;
    if let Value::Structured(value) = value {
        work.charge(value.nodes().len())?;
    }
    Ok(())
}

pub(super) fn predicate(
    predicate: &Predicate,
    base: u128,
    work: &mut Work<'_>,
) -> Result<Predicate, Stop> {
    let name = text(predicate.name(), base, work)?;
    Predicate::with_sign(name, predicate.arity(), predicate.sign())
        .map_err(|_| Stop::InvalidProgram)
}

fn text(value: &str, base: u128, work: &mut Work<'_>) -> Result<String, Stop> {
    admit(
        work,
        base.checked_add(value.len() as u128)
            .ok_or(Stop::StorageLimit)?,
    )?;
    work.charge(value.len())?;
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| Stop::Allocation)?;
    after_reservation(
        work,
        base.checked_add(text.capacity() as u128)
            .ok_or(Stop::StorageLimit)?,
    )?;
    text.push_str(value);
    Ok(text)
}

pub(super) fn pending(
    key: AtomKey<'_>,
    base: u128,
    work: &mut Work<'_>,
) -> Result<(Atom, u128), Stop> {
    let width = key.predicate().arity();
    let mut required = size_of::<Atom>() as u128
        + key.predicate().name().len() as u128
        + width as u128 * size_of::<Value>() as u128;
    for column in 0..width {
        let value = key.value(column).ok_or(Stop::InvalidProgram)?;
        inspect(value, work)?;
        let bytes = match value {
            Value::String(text) | Value::Symbol(text) => text.len() as u128,
            _ => value
                .checked_payload_capacity_bytes()
                .ok_or(Stop::StorageLimit)?,
        };
        required = required.checked_add(bytes).ok_or(Stop::StorageLimit)?;
    }
    admit(work, base.checked_add(required).ok_or(Stop::StorageLimit)?)?;
    let mut live = base
        .checked_add(size_of::<Atom>() as u128)
        .ok_or(Stop::StorageLimit)?;
    let predicate = predicate(key.predicate(), live, work)?;
    live = live
        .checked_add(predicate.name_bytes() as u128)
        .ok_or(Stop::StorageLimit)?;
    admit(
        work,
        live.checked_add(width as u128 * size_of::<Value>() as u128)
            .ok_or(Stop::StorageLimit)?,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(width)
        .map_err(|_| Stop::Allocation)?;
    live = live
        .checked_add(values.capacity() as u128 * size_of::<Value>() as u128)
        .ok_or(Stop::StorageLimit)?;
    after_reservation(work, live)?;
    for column in 0..width {
        work.tick()?;
        let value = match key.value(column).ok_or(Stop::InvalidProgram)? {
            Value::String(value) => Value::String(text(value, live, work)?),
            Value::Symbol(value) => Value::Symbol(text(value, live, work)?),
            value => value.clone(),
        };
        inspect(&value, work)?;
        live = live
            .checked_add(
                value
                    .checked_payload_capacity_bytes()
                    .ok_or(Stop::StorageLimit)?,
            )
            .ok_or(Stop::StorageLimit)?;
        after_reservation(work, live)?;
        values.push(value);
    }
    let atom = Atom::new(predicate, values).map_err(|_| Stop::InvalidProgram)?;
    Ok((atom, live.checked_sub(base).ok_or(Stop::InvalidProgram)?))
}
