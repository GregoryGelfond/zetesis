//! Thin command views of repository assurance libraries.
use clap::{Parser, Subcommand};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
use zetesis_maintenance::{
    Error,
    coverage::{self, Floor, Metadata, Mode, Observation, Tool},
    inventory, proofs,
};

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
    /// Check a retained Lean record; run pinned kernel checks separately.
    ProofRecord {
        #[arg(long, default_value = "proofs")]
        proofs_dir: PathBuf,
        #[arg(long, default_value = "verification.json")]
        record: String,
    },
    /// Validate a committed coverage floor.
    CoverageFloor {
        #[arg(long)]
        mode: String,
        #[arg(long)]
        path: PathBuf,
    },
    /// Select the previous GitHub commit for the coverage-floor check.
    CoveragePrevious {
        #[arg(long)]
        event: PathBuf,
    },
    /// Refuse a reduction of a previously measured coverage floor.
    CoverageRatchet {
        #[arg(long)]
        previous: PathBuf,
        #[arg(long)]
        current: PathBuf,
    },
    /// Validate observed tool identities and serialize coverage metadata.
    CoverageMetadata {
        #[arg(long)]
        mode: String,
        #[arg(long)]
        floor: String,
        #[arg(long)]
        rustc_version: String,
        #[arg(long)]
        llvm_cov: PathBuf,
        #[arg(long)]
        llvm_cov_version: String,
        #[arg(long)]
        llvm_profdata: PathBuf,
        #[arg(long)]
        llvm_profdata_version: String,
        #[arg(long)]
        metal_groups: Option<String>,
    },
    /// Require every selected physical test and its complete passing summary.
    CoveragePhysical {
        #[arg(long)]
        log: PathBuf,
        #[arg(long)]
        group: String,
        #[arg(long)]
        table: String,
    },
    /// Hash the declared Rust/WGSL workspace source inventory.
    Sources {
        #[arg(long)]
        root: PathBuf,
    },
}
fn text(path: &std::path::Path) -> Result<String, Error> {
    String::from_utf8(inventory::read(path, 16_777_216)?)
        .map_err(|_| Error::Invalid(format!("invalid UTF-8: {}", path.display())))
}
fn execute(action: Action, output: &mut impl Write) -> Result<(), Error> {
    let value = match action {
        Action::ProofRecord { proofs_dir, record } => {
            let result = proofs::verify(&proofs_dir, &record, proofs::Limits::default())?;
            format!("Proof record: PASS: {} theorems; {} semantic modules; {} source/configuration files\n", result.theorems, result.semantic_modules, result.source_files).into_bytes()
        }
        Action::CoverageFloor { mode, path } => {
            let source = text(&path)?;
            Floor::parse(&source)?.admit(Mode::parse(&mode)?)?;
            format!("{}\n", source.trim()).into_bytes()
        }
        Action::CoveragePrevious { event } => {
            coverage::previous_revision(&inventory::read(&event, 16_777_216)?)?
                .map_or_else(Vec::new, |revision| format!("{revision}\n").into_bytes())
        }
        Action::CoverageRatchet { previous, current } => {
            coverage::ratchet(
                Floor::parse(&text(&previous)?)?,
                Floor::parse(&text(&current)?)?,
            )?;
            Vec::new()
        }
        Action::CoverageMetadata {
            mode,
            floor,
            rustc_version,
            llvm_cov,
            llvm_cov_version,
            llvm_profdata,
            llvm_profdata_version,
            metal_groups,
        } => {
            let (cov_path, cov_hash) = coverage::executable_identity(&llvm_cov)?;
            let (prof_path, prof_hash) = coverage::executable_identity(&llvm_profdata)?;
            let request = Metadata {
                mode: Mode::parse(&mode)?,
                floor: &floor,
                physical_table: metal_groups.as_deref(),
                observation: Observation {
                    rustc: &rustc_version,
                    llvm_cov: Tool {
                        path: &cov_path,
                        sha256: &cov_hash,
                        version: &llvm_cov_version,
                    },
                    llvm_profdata: Tool {
                        path: &prof_path,
                        sha256: &prof_hash,
                        version: &llvm_profdata_version,
                    },
                },
            };
            coverage::render(&coverage::metadata(request)?)?
        }
        Action::CoveragePhysical { log, group, table } => {
            let groups = coverage::selection(&table)?;
            let selected = groups
                .iter()
                .find(|entry| entry.group == group)
                .ok_or_else(|| Error::Invalid("unknown physical group".into()))?;
            coverage::physical_result(&text(&log)?, selected)?;
            Vec::new()
        }
        Action::Sources { root } => {
            let sources = inventory::sources(&root, inventory::Limits::default())?;
            coverage::render(&serde_json::to_value(sources).map_err(Error::Json)?)?
        }
    };
    output.write_all(&value).map_err(|source| Error::Io {
        path: "<stdout>".into(),
        source,
    })
}
fn main() -> ExitCode {
    match execute(Options::parse().command, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Maintenance: FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
