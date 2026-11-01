//! Fixed-width interchange for the optional GPU handover evidence artifact.
//! Each absent field has a clear presence bit and a zero payload. Integers are little endian.
use std::io;
mod draw;
pub use draw::Draw;

#[cfg(test)]
mod tests;

/// Largest artifact at the world and batch targets of the version-one sampler.
pub const MAX_BYTES: u64 = 128 + 72 * 65_536 + 112 * 4_096;

fn malformed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid residual evidence record",
    )
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed wire field"),
    )
}

fn u128_at(bytes: &[u8], offset: usize) -> u128 {
    u128::from_le_bytes(
        bytes[offset..offset + 16]
            .try_into()
            .expect("fixed wire field"),
    )
}

fn present<T: Default + PartialEq>(flags: u64, bit: u32, value: T) -> io::Result<Option<T>> {
    if flags & (1 << bit) != 0 {
        Ok(Some(value))
    } else if value == T::default() {
        Ok(None)
    } else {
        Err(malformed())
    }
}

/// Run identity and population sizes accompanying the separately retained answer document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    /// Nonce from this invocation's qualification request.
    pub nonce: [u8; 16],
    /// Initial seed of the two independently tagged sampling streams.
    pub seed: u64,
    /// Original residual-world count if known.
    pub worlds: Option<u64>,
    /// Original committed-batch count if known.
    pub batches: Option<u64>,
    /// World slots stored after the header.
    pub retained_worlds: u32,
    /// Batch slots following the worlds.
    pub retained_batches: u32,
    /// Whether the native answer document covers the complete family.
    pub complete: bool,
}

impl Header {
    /// Exact length, absent if a count exceeds the limits of this version.
    #[must_use]
    pub fn byte_len(&self) -> Option<u64> {
        if self.retained_worlds > 65_536
            || self.retained_batches > 4_096
            || self
                .worlds
                .is_some_and(|n| n < u64::from(self.retained_worlds))
            || self
                .batches
                .is_some_and(|n| n < u64::from(self.retained_batches))
        {
            return None;
        }
        Some(128 + 72 * u64::from(self.retained_worlds) + 112 * u64::from(self.retained_batches))
    }

    /// Encode the header, including the schema and sampling-algorithm identifiers.
    #[must_use]
    pub fn encode(&self) -> [u8; 128] {
        let mut b = [0; 128];
        b[..8].copy_from_slice(b"ZETRES01");
        b[8..12].copy_from_slice(&1u32.to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        let flags = u64::from(self.complete)
            | (u64::from(self.worlds.is_some()) << 1)
            | (u64::from(self.batches.is_some()) << 2);
        b[16..24].copy_from_slice(&flags.to_le_bytes());
        b[24..40].copy_from_slice(&self.nonce);
        b[40..48].copy_from_slice(&self.seed.to_le_bytes());
        b[48..56].copy_from_slice(&self.worlds.unwrap_or_default().to_le_bytes());
        b[56..64].copy_from_slice(&self.batches.unwrap_or_default().to_le_bytes());
        b[64..68].copy_from_slice(&self.retained_worlds.to_le_bytes());
        b[68..72].copy_from_slice(&self.retained_batches.to_le_bytes());
        b
    }

    /// Decode and check a single header.
    ///
    /// # Errors
    /// Refuses unsupported versions, flags, noncanonical fields or impossible sizes.
    pub fn decode(b: &[u8; 128]) -> io::Result<Self> {
        let flags = u64_at(b, 16);
        if &b[..8] != b"ZETRES01"
            || b[8..12] != 1u32.to_le_bytes()
            || b[12..16] != 1u32.to_le_bytes()
            || flags & !7 != 0
            || b[72..].iter().any(|&n| n != 0)
        {
            return Err(malformed());
        }
        let h = Self {
            nonce: b[24..40].try_into().map_err(|_| malformed())?,
            seed: u64_at(b, 40),
            worlds: present(flags, 1, u64_at(b, 48))?,
            batches: present(flags, 2, u64_at(b, 56))?,
            retained_worlds: u32::from_le_bytes(b[64..68].try_into().map_err(|_| malformed())?),
            retained_batches: u32::from_le_bytes(b[68..72].try_into().map_err(|_| malformed())?),
            complete: flags & 1 != 0,
        };
        h.byte_len().ok_or_else(malformed)?;
        Ok(h)
    }
}

/// Why the GPU returned a residual world to the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    /// Propagation stopped changing the world.
    FixedPoint,
    /// The round allowance was spent.
    RoundLimit,
    /// The work allowance was spent.
    WorkLimit,
}

/// Availability of a world's CPU completion query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Query {
    /// The optional query observation was unavailable.
    Unknown,
    /// A certificate settled the world without a query.
    Certified,
    /// An entered query with exact counters and optional depth and time.
    Entered {
        /// Visited regions.
        visited: u64,
        /// Attempted splits.
        attempted_splits: u64,
        /// Published splits.
        published_splits: u64,
        /// Deepest visited region.
        depth: Option<u64>,
        /// Actual query work.
        work: u64,
        /// Measured query interval in nanoseconds.
        host_ns: Option<u128>,
    },
}

/// One uniformly sampled committed residual world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct World {
    /// Decoded stopping reason, when joined.
    pub reason: Option<Reason>,
    /// Open atoms in the supplied region, when measured.
    pub open: Option<u64>,
    /// CPU query evidence.
    pub query: Query,
}

impl World {
    /// Encode presence and numerical fields.
    #[must_use]
    pub fn encode(&self) -> [u8; 72] {
        let mut bytes = [0; 72];
        let mut flags: u64 = match self.reason {
            None => 0,
            Some(Reason::FixedPoint) => 1,
            Some(Reason::RoundLimit) => 2,
            Some(Reason::WorkLimit) => 3,
        };
        flags |= u64::from(self.open.is_some()) << 2;
        bytes[8..16].copy_from_slice(&self.open.unwrap_or_default().to_le_bytes());
        match self.query {
            Query::Unknown => {}
            Query::Certified => flags |= 1 << 3,
            Query::Entered {
                visited,
                attempted_splits,
                published_splits,
                depth,
                work,
                host_ns,
            } => {
                flags |=
                    2 << 3 | u64::from(depth.is_some()) << 5 | u64::from(host_ns.is_some()) << 6;
                for (field, n) in bytes[16..56].chunks_exact_mut(8).zip([
                    visited,
                    attempted_splits,
                    published_splits,
                    depth.unwrap_or_default(),
                    work,
                ]) {
                    field.copy_from_slice(&n.to_le_bytes());
                }
                bytes[56..72].copy_from_slice(&host_ns.unwrap_or_default().to_le_bytes());
            }
        }
        bytes[..8].copy_from_slice(&flags.to_le_bytes());
        bytes
    }

    /// Decode a world, preserving the distinction between missing and zero.
    ///
    /// # Errors
    /// Refuses inconsistent splits, depth, flags and inactive fields.
    pub fn decode(b: &[u8; 72]) -> io::Result<Self> {
        let flags = u64_at(b, 0);
        if flags & !0x7f != 0 {
            return Err(malformed());
        }
        let reason = match flags & 3 {
            1 => Some(Reason::FixedPoint),
            2 => Some(Reason::RoundLimit),
            3 => Some(Reason::WorkLimit),
            _ => None,
        };
        let open = present(flags, 2, u64_at(b, 8))?;
        let query = match (flags >> 3) & 3 {
            k @ (0 | 1) if flags & 0x60 == 0 && b[16..].iter().all(|&n| n == 0) => {
                if k == 0 {
                    Query::Unknown
                } else {
                    Query::Certified
                }
            }
            2 => {
                let visited = u64_at(b, 16);
                let attempted_splits = u64_at(b, 24);
                let published_splits = u64_at(b, 32);
                let depth = present(flags, 5, u64_at(b, 40))?;
                if published_splits > attempted_splits || depth.is_some_and(|d| d >= visited) {
                    return Err(malformed());
                }
                Query::Entered {
                    visited,
                    attempted_splits,
                    published_splits,
                    depth,
                    work: u64_at(b, 48),
                    host_ns: present(flags, 6, u128_at(b, 56))?,
                }
            }
            _ => return Err(malformed()),
        };
        Ok(Self {
            reason,
            open,
            query,
        })
    }
}

/// One uniformly sampled committed batch. Array order is all, fixed point, round limit, work limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Batch {
    /// Residual counts for this batch.
    pub residuals: [Option<u64>; 4],
    /// Maximum joint width across depths.
    pub width: Option<u64>,
    /// Sum of host query intervals in nanoseconds.
    pub host_ns: [Option<u128>; 4],
}

impl Batch {
    /// Encode the batch without exposing Rust's in-memory layout.
    #[must_use]
    pub fn encode(&self) -> [u8; 112] {
        let mut bytes = [0; 112];
        let mut flags = u64::from(self.width.is_some()) << 4;
        for i in 0..4 {
            flags |= u64::from(self.residuals[i].is_some()) << i;
            flags |= u64::from(self.host_ns[i].is_some()) << (5 + i);
            bytes[8 + i * 8..16 + i * 8]
                .copy_from_slice(&self.residuals[i].unwrap_or_default().to_le_bytes());
            bytes[48 + i * 16..64 + i * 16]
                .copy_from_slice(&self.host_ns[i].unwrap_or_default().to_le_bytes());
        }
        bytes[..8].copy_from_slice(&flags.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.width.unwrap_or_default().to_le_bytes());
        bytes
    }

    /// Decode one batch and check the count and interval partitions when known.
    ///
    /// # Errors
    /// Refuses invalid presence flags, partitions and payloads.
    pub fn decode(b: &[u8; 112]) -> io::Result<Self> {
        let flags = u64_at(b, 0);
        if flags & !0x01ff != 0 {
            return Err(malformed());
        }
        let mut residuals = [None; 4];
        let mut host_ns = [None; 4];
        for i in 0..4_u32 {
            let j = i as usize;
            residuals[j] = present(flags, i, u64_at(b, 8 + j * 8))?;
            host_ns[j] = present(flags, 5 + i, u128_at(b, 48 + j * 16))?;
            if residuals[j] == Some(0) && host_ns[j].is_some_and(|t| t != 0) {
                return Err(malformed());
            }
        }
        let width = present(flags, 4, u64_at(b, 40))?;
        if residuals[0] == Some(0) && width.is_some_and(|w| w != 0) {
            return Err(malformed());
        }
        if let Some(total) = residuals[0] {
            let measured: u128 = residuals[1..]
                .iter()
                .flatten()
                .map(|&n| u128::from(n))
                .sum();
            if measured > u128::from(total)
                || (residuals[1..].iter().all(Option::is_some) && measured != u128::from(total))
            {
                return Err(malformed());
            }
        }
        if let Some(time) = host_ns[0] {
            let subtotal = host_ns[1..]
                .iter()
                .flatten()
                .try_fold(0u128, |sum, &n| sum.checked_add(n))
                .ok_or_else(malformed)?;
            if subtotal > time || (host_ns[1..].iter().all(Option::is_some) && subtotal != time) {
                return Err(malformed());
            }
        }
        Ok(Self {
            residuals,
            width,
            host_ns,
        })
    }
}
