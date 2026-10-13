//! The CLI run on a source bundle written by the shared bundle fixture.

use std::process::{Command, Output, Stdio};

use zetesis_reference_support::BundleFixture;

/// The CLI run on `fixture`'s `entry.lp` on the CPU, enumerating every
/// model, with `arguments` before the input and no standard input.
pub(crate) fn process(fixture: &BundleFixture, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args(["--backend", "cpu", "--models", "0"])
        .args(arguments)
        .arg(fixture.path("entry.lp"))
        .stdin(Stdio::null())
        .output()
        .unwrap()
}
