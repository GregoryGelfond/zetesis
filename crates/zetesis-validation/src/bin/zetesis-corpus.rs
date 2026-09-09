//! Thin views of curated verification and selected comparisons.
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use zetesis_validation::curated::{self, Limits};
use zetesis_validation::{examples, selected};

#[derive(Parser)]
#[command(
    version,
    about = "Verify ASP corpus provenance and compare complete solver results"
)]
struct Options {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Verify the self-contained kr-domains examples and typed contracts.
    VerifyExamples {
        /// Clean example directory containing manifest.json and ASP sources.
        root: PathBuf,
        /// Also verify the exact annotation deletions against preserved originals.
        #[arg(long)]
        originals: Option<PathBuf>,
    },
    /// Compare all 24 selected sources against clingo and pinned full-model contracts.
    Compare {
        /// Curated directory containing manifest.json and programs/.
        root: PathBuf,
        /// Independent clingo executable path.
        #[arg(long)]
        clingo: PathBuf,
        /// Native zetesis executable path.
        #[arg(long)]
        zetesis: PathBuf,
        /// New evidence file; existing paths are never replaced.
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify exact source bytes, provenance, license and model contracts.
    Verify {
        /// Curated directory containing manifest.json and programs/.
        root: PathBuf,
    },
}
fn execute(options: Options) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let corpus = match options.command {
        Action::VerifyExamples { root, originals } => {
            let corpus = examples::load(&root, examples::Limits::default())?;
            if let Some(originals) = &originals {
                examples::verify_originals(&corpus, originals, examples::Limits::default())?;
            }
            let report = serde_json::json!({
                "schema": 1,
                "integrity": "verified",
                "semantic_solver_run": false,
                "manifest_sha256": corpus.manifest_sha256(),
                "revision": corpus.revision(),
                "files": corpus.files().len(),
                "cases": corpus.cases().len(),
                "originals_verified": originals.is_some(),
            });
            write_json(&report)?;
            return Ok(ExitCode::SUCCESS);
        }
        Action::Compare {
            root,
            clingo,
            zetesis,
            report,
        } => {
            let reference = std::path::absolute(clingo)?;
            let native = std::path::absolute(zetesis)?;
            let result = selected::run(&selected::Request {
                corpus: &root,
                reference: &reference,
                native: &native,
                report: &report,
                execution: selected::NativeExecution::default(),
                limits: selected::Limits::default(),
            })?;
            result.publish()?;
            writeln!(
                io::stdout().lock(),
                "{}: {} selected cases; evidence {}",
                if result.passed() { "pass" } else { "fail" },
                result.cases().len(),
                report.display()
            )?;
            return Ok(if result.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
        Action::Verify { root } => curated::open(&root, Limits::default())?,
    };
    let report = serde_json::json!({"schema":1,"integrity":"verified","semantic_solver_run":false,"manifest_sha256":curated::MANIFEST_SHA256,"cases":corpus.cases().len(),"full_model_occurrences":corpus.cases().iter().map(|case|case.contract().full_models().len()).sum::<usize>()});
    write_json(&report)?;
    Ok(ExitCode::SUCCESS)
}

fn write_json(report: &serde_json::Value) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, report)?;
    output.write_all(b"\n")
}
fn main() -> ExitCode {
    match execute(Options::parse()) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "zetesis-corpus: {error}");
            ExitCode::from(2)
        }
    }
}
