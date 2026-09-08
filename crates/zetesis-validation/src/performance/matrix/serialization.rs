//! Borrowed lossless stream views keep matrix publication outside measured solves.
//!
//! UTF-8 validation scans each stream once per serialization pass, without copying
//! its bytes. Bounded publication preflights and then writes the same view.
//! Serde writes the borrowed text or byte sequence through the existing bounded
//! publication writer. The legacy capture serializer remains unchanged.
use std::ffi::OsString;
use std::path::Path;

use serde::{Serialize, Serializer, ser::SerializeSeq};

use super::super::Capture;
use crate::{process, selected::InvocationFailure};

#[derive(Serialize)]
#[serde(tag = "encoding", content = "data", rename_all = "snake_case")]
enum Stream<'a> {
    Utf8(&'a str),
    Bytes(&'a [u8]),
}
impl<'a> Stream<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::Utf8(text),
            Err(_) => Self::Bytes(bytes),
        }
    }
}

#[derive(Serialize)]
struct View<'a> {
    executable: &'a Path,
    arguments: &'a [OsString],
    directory: &'a Path,
    started_unix_ns: Option<u128>,
    elapsed_ns: Option<u128>,
    stop: Option<process::Stop>,
    exit: Option<process::Exit>,
    stdout: Stream<'a>,
    stderr: Stream<'a>,
    failure: Option<&'a InvocationFailure>,
    cleanup_failure: Option<&'a InvocationFailure>,
    unresolved_child: Option<u32>,
}
impl<'a> View<'a> {
    fn new(capture: &'a Capture) -> Self {
        Self {
            executable: capture.executable(),
            arguments: capture.arguments(),
            directory: capture.directory(),
            started_unix_ns: capture.started_unix_ns(),
            elapsed_ns: capture.elapsed_ns(),
            stop: capture.stop(),
            exit: capture.exit(),
            stdout: Stream::new(capture.stdout()),
            stderr: Stream::new(capture.stderr()),
            failure: capture.failure(),
            cleanup_failure: capture.cleanup_failure(),
            unresolved_child: capture.unresolved_child,
        }
    }
}

pub(super) fn captures<S: Serializer>(
    values: &[Capture],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(values.len()))?;
    for capture in values {
        sequence.serialize_element(&View::new(capture))?;
    }
    sequence.end()
}
#[expect(
    clippy::ref_option,
    reason = "serde's field serializer receives a borrowed Option"
)]
pub(super) fn optional_capture<S: Serializer>(
    value: &Option<Capture>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value.as_ref().map(View::new).serialize(serializer)
}

#[cfg(test)]
#[path = "../../../tests/support/matrix_serialization.rs"]
mod tests;
