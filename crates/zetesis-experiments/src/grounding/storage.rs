//! Bounded diagnostic bytes, shared by catalog formatting and JSON preflight.

use std::{fmt, io};

use super::Error;

pub(super) struct Bytes {
    pub(super) data: Vec<u8>,
    limit: usize,
    resource: &'static str,
    pub(super) refusal: Option<Error>,
}

impl Bytes {
    pub(super) fn new(limit: usize, resource: &'static str) -> Self {
        Self {
            data: Vec::new(),
            limit,
            resource,
            refusal: None,
        }
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let required = self
            .data
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Limit {
                resource: self.resource,
                limit: self.limit,
            })?;
        if required > self.limit {
            return Err(Error::Limit {
                resource: self.resource,
                limit: self.limit,
            });
        }
        if required > self.data.capacity() {
            // Geometric requests are capped by the byte ceiling. Allocator
            // rounding/metadata are not logical payload bytes.
            let capacity = self
                .data
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(self.limit);
            self.data
                .try_reserve_exact(capacity - self.data.len())
                .map_err(|_| Error::Allocation)?;
        }
        self.data.extend_from_slice(bytes);
        Ok(())
    }
}

impl io::Write for Bytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.refusal.is_none() {
            self.refusal = self.append(bytes).err();
        }
        if self.refusal.is_some() {
            return Err(io::Error::other("bounded diagnostic buffer refused bytes"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Write for Bytes {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        io::Write::write_all(self, text.as_bytes()).map_err(|_| fmt::Error)
    }
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    Ok(values)
}
