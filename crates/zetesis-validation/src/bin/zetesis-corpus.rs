//! Thin command for exact curated-data verification and explicit legacy import.
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use zetesis_validation::curated::{self, Limits};

#[derive(Parser)]
#[command(
    version,
    about = "Verify curated clingo data without a solver or C++ runtime"
)]
struct Options {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Verify exact source bytes, provenance, license and model contracts.
    Verify {
        /// Curated directory containing manifest.json and programs/.
        root: PathBuf,
    },
    /// Verify preserved legacy originals and write a new curated directory.
    Import {
        /// Legacy directory containing cases.jsonl and originals/.
        legacy: PathBuf,
        /// New destination; existing paths are never replaced.
        destination: PathBuf,
    },
}
fn execute(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let corpus = match options.command {
        Action::Verify { root } => curated::open(&root, Limits::default())?,
        Action::Import {
            legacy,
            destination,
        } => curated::import_legacy(&legacy, &destination, Limits::default())?,
    };
    let report = serde_json::json!({"schema":1,"integrity":"verified","semantic_solver_run":false,"manifest_sha256":curated::MANIFEST_SHA256,"cases":corpus.cases().len(),"full_model_occurrences":corpus.cases().iter().map(|case|case.contract().full_models().len()).sum::<usize>()});
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &report)?;
    output.write_all(b"\n")?;
    Ok(())
}
fn main() -> ExitCode {
    match execute(Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-corpus: {error}");
            ExitCode::from(2)
        }
    }
}
