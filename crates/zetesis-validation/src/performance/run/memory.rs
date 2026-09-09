//! Separate helper invocation and bounded child-resource evidence.
use super::{Capture, Report, launch};
use crate::process;
use std::path::Path;
use std::time::Instant;

pub(super) fn invoke(
    helper: &Path,
    solver: process::Invocation<'_>,
    record: &Path,
    deadline: Instant,
    report: &mut Report,
) -> Option<Capture> {
    let mut arguments = vec![
        "__measure-child".into(),
        record.as_os_str().to_owned(),
        solver.executable.as_os_str().to_owned(),
    ];
    arguments.extend_from_slice(solver.arguments);
    launch(helper, arguments, solver.directory, deadline, report, true)
}

pub(super) fn read(
    path: &Path,
    helper: Option<u32>,
) -> (Vec<u8>, Result<process::memory::Measurement, String>) {
    use std::io::Read;
    const RECORD_BYTES: u64 = 4096;
    let mut bytes = Vec::new();
    let result = (|| {
        std::fs::File::open(path)
            .map_err(|error| error.to_string())?
            .take(RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u128 > u128::from(RECORD_BYTES) {
            return Err("child RSS record exceeds 4096 bytes".into());
        }
        let record: process::memory::Measurement =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if !record.valid() {
            return Err("child RSS record contradicts its exit or unit conversion".into());
        }
        if helper.is_none_or(|helper| record.child == helper)
            || Some(record.raw_unit) != process::memory::Unit::current()
        {
            return Err("child RSS record contradicts helper identity or platform units".into());
        }
        Ok(record)
    })();
    let result = match std::fs::remove_file(path) {
        Ok(()) => result,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => result,
        Err(error) => Err(format!("child RSS record cleanup: {error}")),
    };
    (bytes, result)
}
