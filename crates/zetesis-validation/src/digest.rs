//! How every seal, of a file or a workload, spells its SHA-256 digest.

use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes`, spelled as every seal spells a digest.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    spelled(&Sha256::digest(bytes))
}

/// A finished SHA-256 digest in lowercase hexadecimal, as every seal spells it.
pub(crate) fn spelled(digest: &sha2::digest::Output<Sha256>) -> String {
    format!("{digest:x}")
}
