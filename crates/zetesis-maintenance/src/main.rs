//! Thin command views of repository assurance libraries.
use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
use zetesis_maintenance::proofs;

#[derive(Parser)]
#[command(
    version,
    about = "Check repository assurance records without substituting for their execution"
)]
struct Options {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Check the retained Lean record; run the pinned kernel checks separately.
    ProofRecord {
        /// Proof-library root.
        #[arg(long, default_value = "proofs")]
        proofs_dir: PathBuf,
        /// Retained record relative to the proof-library root.
        #[arg(long, default_value = "verification.json")]
        record: String,
    },
}
fn execute(options: Options) -> Result<(), zetesis_maintenance::Error> {
    match options.command {
        Action::ProofRecord { proofs_dir, record } => {
            let result = proofs::verify(&proofs_dir, &record, proofs::Limits::default())?;
            println!(
                "Proof record: PASS: {} theorems; {} semantic modules; {} source/configuration files",
                result.theorems, result.semantic_modules, result.source_files
            );
        }
    }
    Ok(())
}
fn main() -> ExitCode {
    match execute(Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Maintenance: FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
