//! Thin command views of repository assurance libraries.
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
use zetesis_maintenance::{
    Error, book,
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

#[derive(Clone, Copy, ValueEnum)]
enum InventoryView {
    Inventory,
    Index,
    Audit,
}
#[derive(Subcommand)]
enum Action {
    /// Publish only libraries named by the current successful Cargo build.
    BookLibraries {
        #[arg(long)]
        messages: PathBuf,
        #[arg(long)]
        build_directory: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long = "crate", required = true)]
        required_crates: Vec<String>,
    },
    /// Observe Lean source declarations; no proof success is inferred.
    ProofInventory {
        #[arg(long, default_value = "proofs")]
        proofs_dir: PathBuf,
        #[arg(long, value_enum, default_value = "inventory")]
        view: InventoryView,
    },
    /// Run pinned kernel checks and transactionally refresh their record.
    ProofCapture {
        #[arg(long, default_value = ".")]
        repository: PathBuf,
        /// Existing empty directory outside the repository, retained on failure.
        #[arg(long)]
        evidence: PathBuf,
        /// Actual Lean toolchain bin directory, not an elan shim directory.
        #[arg(long)]
        lean_bin: PathBuf,
        /// Actual Rust 1.97.1 bin directory, not a rustup shim directory.
        #[arg(long)]
        rust_bin: PathBuf,
    },
    /// Check a retained Lean record; run pinned kernel checks separately.
    ProofRecord {
        #[arg(long, default_value = "proofs")]
        proofs_dir: PathBuf,
        #[arg(long, default_value = "verification.json")]
        record: String,
        /// Complete stdout of a newly successful strict Audit command.
        #[arg(long)]
        live_audit: Option<PathBuf>,
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
        cargo_llvm_cov_version: String,
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
fn proof_record(
    directory: &std::path::Path,
    record: &str,
    live_audit: Option<&std::path::Path>,
) -> Result<Vec<u8>, Error> {
    let limits = proofs::Limits::default();
    let result = if let Some(path) = live_audit {
        let bytes = inventory::read(path, limits.file_bytes)?;
        proofs::verify_with_audit(directory, record, &bytes, limits)?
    } else {
        proofs::verify(directory, record, limits)?
    };
    Ok(format!(
        "Proof record: PASS: {} theorems; {} semantic modules; {} source/configuration files\n",
        result.theorems, result.semantic_modules, result.source_files
    )
    .into_bytes())
}
fn book_libraries(
    messages: &std::path::Path,
    build: &std::path::Path,
    destination: &std::path::Path,
    roots: &[String],
) -> Result<Vec<u8>, Error> {
    let limits = book::Limits::default();
    let bytes = inventory::read(messages, limits.message_bytes)?;
    let required: Vec<_> = roots.iter().map(String::as_str).collect();
    let libraries = book::select(&bytes, &required, limits)?;
    libraries.publish(build, destination)?;
    Ok(format!(
        "Book libraries: {} current artifacts\n",
        libraries.paths().len()
    )
    .into_bytes())
}
fn proof_capture(
    repository: &std::path::Path,
    evidence: &std::path::Path,
    lean_bin: &std::path::Path,
    rust_bin: &std::path::Path,
) -> Result<Vec<u8>, Error> {
    let maintenance = std::env::current_exe().map_err(|source| Error::Io {
        path: "<current executable>".into(),
        source,
    })?;
    let summary = proofs::capture::capture(proofs::capture::Request {
        repository,
        evidence,
        lean_bin,
        rust_bin,
        maintenance: &maintenance,
        command_limits: zetesis_validation::process::Limits {
            timeout: std::time::Duration::from_mins(5),
            max_output_bytes: 16 * 1024 * 1024,
            cleanup_timeout: std::time::Duration::from_secs(2),
        },
    })
    .map_err(|error| Error::Invalid(error.to_string()))?;
    Ok(format!(
        "Proof capture: PASS: {} theorems; {} semantic modules; evidence {}\n",
        summary.theorems,
        summary.semantic_modules,
        evidence.display()
    )
    .into_bytes())
}
fn execute(action: Action, output: &mut impl Write) -> Result<(), Error> {
    let value = match action {
        Action::BookLibraries {
            messages,
            build_directory,
            destination,
            required_crates,
        } => book_libraries(&messages, &build_directory, &destination, &required_crates)?,
        Action::ProofInventory { proofs_dir, view } => {
            let inventory = proofs::inventory(&proofs_dir, proofs::Limits::default())?;
            match view {
                InventoryView::Inventory => {
                    coverage::render(&serde_json::to_value(&inventory).map_err(Error::Json)?)?
                }
                InventoryView::Index => inventory.theorem_index()?,
                InventoryView::Audit => inventory.audit_source().into_bytes(),
            }
        }
        Action::ProofCapture {
            repository,
            evidence,
            lean_bin,
            rust_bin,
        } => proof_capture(&repository, &evidence, &lean_bin, &rust_bin)?,
        Action::ProofRecord {
            proofs_dir,
            record,
            live_audit,
        } => proof_record(&proofs_dir, &record, live_audit.as_deref())?,
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
            cargo_llvm_cov_version,
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
                    cargo_llvm_cov: &cargo_llvm_cov_version,
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
            let groups = coverage::selection(&table)?.groups;
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
