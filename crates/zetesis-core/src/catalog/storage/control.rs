//! Work permits precede logical reads, hash input, equality probes and reserves.
//! A bulk reserve/copy receives all permits before it starts, so a caller stop
//! cannot leave a partly published row. Standard container internals are one
//! bounded library operation, not a claim about allocator or CPU instructions.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::{
    Failure, Fault, TermId,
    budget::{self, Budget},
};

pub(super) struct Work<'a, E> {
    before: &'a mut dyn FnMut() -> Result<(), E>,
}

impl<'a, E> Work<'a, E> {
    pub(super) fn new(before: &'a mut dyn FnMut() -> Result<(), E>) -> Self {
        Self { before }
    }

    pub(super) fn step(&mut self) -> Result<(), Failure<E>> {
        (self.before)().map_err(Failure::Stopped)
    }

    pub(super) fn steps(&mut self, count: usize) -> Result<(), Failure<E>> {
        for _ in 0..count {
            self.step()?;
        }
        Ok(())
    }

    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        budget: &mut Budget,
    ) -> Result<(), Failure<E>> {
        self.step()?;
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or(Fault::Overflow)?;
        if needed > values.capacity() {
            self.steps(values.len())?;
        }
        budget::reserve(values, additional, budget).map_err(Failure::Storage)
    }

    pub(super) fn reserve_text(
        &mut self,
        text: &mut String,
        additional: usize,
        budget: &mut Budget,
    ) -> Result<(), Failure<E>> {
        self.step()?;
        let needed = text.len().checked_add(additional).ok_or(Fault::Overflow)?;
        if needed > text.capacity() {
            self.steps(text.len())?;
        }
        budget::reserve_text(text, additional, budget).map_err(Failure::Storage)
    }

    pub(super) fn text_hash(&mut self, text: &str) -> Result<u64, Failure<E>> {
        let mut state = DefaultHasher::new();
        self.step()?;
        text.len().hash(&mut state);
        for index in 0..text.len() {
            self.step()?;
            state.write_u8(text.as_bytes()[index]);
        }
        self.step()?;
        Ok(state.finish())
    }

    pub(super) fn key_hash(
        &mut self,
        key: impl Hash,
        children: &[TermId],
    ) -> Result<u64, Failure<E>> {
        let mut state = DefaultHasher::new();
        self.step()?;
        key.hash(&mut state);
        self.step()?;
        children.len().hash(&mut state);
        for child in children {
            self.step()?;
            child.hash(&mut state);
        }
        self.step()?;
        Ok(state.finish())
    }

    pub(super) fn text_equal(&mut self, left: &str, right: &str) -> Result<bool, Failure<E>> {
        self.step()?;
        if left.len() != right.len() {
            return Ok(false);
        }
        for index in 0..left.len() {
            self.step()?;
            if left.as_bytes()[index] != right.as_bytes()[index] {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn quoted_bytes(&mut self, text: &str) -> Result<usize, Failure<E>> {
        self.step()?;
        let mut bytes = text.len().checked_add(2).ok_or(Fault::Overflow)?;
        for index in 0..text.len() {
            self.step()?;
            if matches!(text.as_bytes()[index], b'"' | b'\\' | b'\n') {
                bytes = bytes.checked_add(1).ok_or(Fault::Overflow)?;
            }
        }
        Ok(bytes)
    }
}

pub(super) fn uncontrolled<T>(
    result: Result<T, Failure<std::convert::Infallible>>,
) -> Result<T, Fault> {
    match result {
        Ok(value) => Ok(value),
        Err(Failure::Storage(error)) => Err(error),
        Err(Failure::Stopped(never)) => match never {},
    }
}
