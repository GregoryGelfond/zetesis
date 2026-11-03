//! Request, authentication and pooling of separately captured GPU residual samples.
//! The ordinary result document remains the authority for completion and coverage.
mod metrics;
mod pool;
#[cfg(test)]
mod tests;

use crate::performance::matrix::ResidualRunSamples;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use zetesis_backend::residual_evidence::{Batch, Header, World};

pub use pool::{Pool, PooledSamples};
pub use zetesis_backend::residual_evidence::MAX_BYTES;

/// Fixed input filename in the invocation's private directory.
pub const REQUEST_NAME: &str = "request.json";
/// Fixed final artifact filename in the same directory.
pub const ARTIFACT_NAME: &str = "samples.bin";
const REQUEST_LIMIT: u64 = 1024;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Explicit request bound to one invocation. The destination is always a sibling of the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Request schema.
    pub version: u32,
    /// Unique identifier for this qualification slot.
    pub nonce: [u8; 16],
    /// Maximum bytes the child may publish for this optional artifact.
    pub max_bytes: u64,
}

impl Request {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 || self.max_bytes > MAX_BYTES {
            return Err(invalid("unsupported residual evidence request"));
        }
        Ok(())
    }

    /// Read a bounded qualification request.
    ///
    /// # Errors
    /// Returns file, schema and size errors without affecting a solve.
    pub fn read(path: &Path) -> io::Result<Self> {
        let file = regular_file(path)?;
        let mut bytes = Vec::new();
        file.take(REQUEST_LIMIT + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > REQUEST_LIMIT {
            return Err(invalid("residual evidence request exceeds its byte limit"));
        }
        let request: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        request.validate()?;
        Ok(request)
    }

    /// Create a request without replacing any earlier one.
    ///
    /// # Errors
    /// Returns schema, creation or write failures.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
        serde_json::to_writer(&mut file, self).map_err(io::Error::other)?;
        file.flush()
    }
}

fn regular_file(path: &Path) -> io::Result<File> {
    if !path.symlink_metadata()?.file_type().is_file() {
        return Err(invalid("residual evidence requires a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use rustix::fs::OFlags;
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags((OFlags::NOFOLLOW | OFlags::NONBLOCK).bits().cast_signed());
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("residual evidence file changed kind"));
    }
    Ok(file)
}

/// Revalidate the retained bytes immediately before publishing their report.
/// Use the same regular-file boundary as the initial decode, and never read
/// past the offered length plus the single byte needed to detect growth.
pub(crate) fn verify_digest(path: &Path, bytes: u64, expected: &str) -> io::Result<()> {
    let file = regular_file(path)?;
    if bytes > MAX_BYTES || file.metadata()?.len() != bytes {
        return Err(invalid("retained residual evidence changed length"));
    }
    let mut hash = Sha256::new();
    let actual = io::copy(&mut file.take(bytes + 1), &mut hash)?;
    if actual != bytes || crate::digest::spelled(&hash.finalize()) != expected {
        return Err(invalid(
            "retained residual evidence changed after validation",
        ));
    }
    Ok(())
}

/// Validated record values and seal for one invocation.
#[derive(Debug)]
pub struct Artifact {
    header: Header,
    worlds: Vec<World>,
    batches: Vec<Batch>,
    totals: metrics::Totals,
    digest: String,
    bytes: u64,
}

impl Artifact {
    /// SHA-256 of the exact bytes validated.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
    /// Exact size of the sealed artifact.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.bytes
    }
}

/// Authenticate the file against its request and the summary from this same process.
///
/// # Errors
/// Refuses a truncated or oversized file, a mismatched identity, any inconsistent
/// record, or disagreement between the retained observations and the summary.
pub fn read(
    path: &Path,
    request: &Request,
    summary: &ResidualRunSamples,
    complete: bool,
) -> io::Result<Artifact> {
    request.validate()?;
    summary
        .check(None)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    for pool in [&summary.worlds, &summary.batches] {
        // Version one admits each fixed reservoir once. It loses the entire
        // frame on ordinal overflow and never silently thins a retained set.
        let expected = pool.seen.map_or(0, |n| n.min(pool.capacity as u64));
        if pool.retained as u64 != expected || pool.seen.is_none() && pool.capacity != 0 {
            return Err(invalid(
                "residual evidence has a thinned or lost population",
            ));
        }
    }
    let groups = summary
        .groups
        .as_ref()
        .ok_or_else(|| invalid("terminal residual sample coverage unavailable"))?;
    let file = regular_file(path)?;
    let bytes = file.metadata()?.len();
    if bytes > request.max_bytes {
        return Err(invalid("residual evidence exceeds its offered bytes"));
    }
    let mut source = BufReader::new(file);
    let mut hash = Sha256::new();
    let header = Header::decode(&block(&mut source, &mut hash)?)?;
    if header.byte_len() != Some(bytes)
        || header.nonce != request.nonce
        || header.complete != complete
        || header.seed != summary.seed
        || header.worlds != summary.worlds.seen
        || header.batches != summary.batches.seen
        || header.retained_worlds as usize != summary.worlds.retained
        || header.retained_batches as usize != summary.batches.retained
    {
        return Err(invalid(
            "residual evidence header disagrees with request or native result",
        ));
    }
    let worlds = records(
        header.retained_worlds,
        &mut source,
        &mut hash,
        World::decode,
    )?;
    let batches = records(
        header.retained_batches,
        &mut source,
        &mut hash,
        Batch::decode,
    )?;
    if source.read(&mut [0; 1])? != 0 {
        return Err(invalid("trailing residual evidence"));
    }
    let artifact = Artifact {
        header,
        worlds,
        batches,
        totals: metrics::Totals::from_groups(groups),
        bytes,
        digest: crate::digest::spelled(&hash.finalize()),
    };
    metrics::check(&artifact, groups)?;
    Ok(artifact)
}

fn block<const N: usize>(source: &mut impl Read, hash: &mut Sha256) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    source.read_exact(&mut bytes)?;
    hash.update(bytes);
    Ok(bytes)
}

fn records<T, const N: usize>(
    n: u32,
    source: &mut impl Read,
    hash: &mut Sha256,
    decode: impl Fn(&[u8; N]) -> io::Result<T>,
) -> io::Result<Vec<T>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(n as usize)
        .map_err(io::Error::other)?;
    for _ in 0..n {
        rows.push(decode(&block(source, hash)?)?);
    }
    Ok(rows)
}
