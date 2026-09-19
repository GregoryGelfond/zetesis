//! Compare gate projections through ordinary, fully collected formula sessions.
//!
//! Source admission and shared pipeline compilation precede timing. Each timed
//! session includes fresh search/transport, residual completion, scoring and
//! retention. Family comparison and JSON publication follow timing. Input must
//! be self-contained: this example does not load includes or measure grounding.

use std::{
    collections::BTreeSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Instant,
};

use clap::Parser;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zetesis_cpu::Control;
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, ExecutionResources, Grounder,
    Oracle, PreparedInput, Session, SolveConfig, WorldView, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};
use zetesis_wgpu::{
    AdapterMetadata, GateProjection, GpuBackendPreference, GpuContext, GpuFormulaProfile,
    GpuOptions, GpuSelection,
};

const MAX_SOURCE_BYTES: u64 = 1_048_576;
type Family = BTreeSet<(Vec<usize>, Option<Vec<(i32, i64)>>)>;

#[derive(Parser)]
#[command(about = "Compare ordinary CPU/Rayon and GPU formula sessions")]
struct Options {
    /// One self-contained input source; admission occurs outside timing.
    source: PathBuf,
    /// Require this physical API, or collect only CPU baselines.
    #[arg(long, value_enum, default_value = "cpu")]
    backend: zetesis_experiments::Backend,
    /// Rayon completion workers; must be at least two to name a Rayon route.
    #[arg(long, default_value = "4")]
    workers: NonZeroUsize,
    #[arg(long, default_value = "32")]
    batch_size: NonZeroUsize,
    /// Each repetition runs scalar/Rayon, then enumerated/bitwise/bitwise/enumerated.
    #[arg(long, default_value = "3")]
    repetitions: NonZeroUsize,
    #[arg(long, default_value = "1")]
    warmups: usize,
}

struct Route<'a> {
    expected: Option<&'a GpuFormulaProfile>,
    config: SolveConfig,
    selected: bool,
    parallel: bool,
}

impl ExecutionObserver for Route<'_> {
    type Error = io::Error;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::CpuFormula {
                oracle,
                grounder,
                search,
            } => {
                if self.expected.is_some()
                    || self.selected
                    || oracle != self.config.oracle
                    || grounder != self.config.grounder
                    || search != self.config.search
                {
                    return Err(io::Error::other("unexpected CPU formula route"));
                }
                self.selected = true;
            }
            ExecutionObservation::ExactCompletion {
                workers,
                max_scratch_bytes,
            } => {
                if workers != self.config.completion_workers
                    || max_scratch_bytes != self.config.max_completion_scratch_bytes
                {
                    return Err(io::Error::other("unexpected completion configuration"));
                }
                self.parallel = true;
            }
            ExecutionObservation::DeviceFormula {
                adapter,
                projection,
                grounder,
                search,
                batch_size,
                completion_workers,
            } => {
                let expected = self
                    .expected
                    .ok_or_else(|| io::Error::other("unexpected device"))?;
                if self.selected
                    || projection != expected.projection()
                    || adapter != expected.info().metadata()
                    || grounder != self.config.grounder
                    || search != self.config.search
                    || batch_size != self.config.batch_size
                    || completion_workers != self.config.completion_workers
                {
                    return Err(io::Error::other(
                        "device route differs from supplied profile",
                    ));
                }
                self.selected = true;
            }
            _ => {}
        }
        Ok(())
    }
}

struct Run {
    family: WorldView,
    elapsed_ns: u128,
}

fn measure(
    admitted: &AdmittedFormula,
    config: SolveConfig,
    profile: Option<&GpuFormulaProfile>,
    collection: WorldViewLimits,
) -> Result<Run, Box<dyn Error>> {
    let resources = profile.map(ExecutionResources::with_formula_profile);
    let mut route = Route {
        expected: profile,
        config,
        selected: false,
        parallel: false,
    };
    let started = Instant::now();
    let builder = Session::builder(PreparedInput::formula(admitted), config, Control::default());
    let builder = if let Some(resources) = &resources {
        builder.resources(resources)
    } else {
        builder
    };
    // Preserve the original WorldViewFailure, including its checked prefix.
    let family = builder.collect_observed(collection, &mut route)?;
    let elapsed_ns = started.elapsed().as_nanos();
    if family.outcome().completion() != Some(Completion::Exhausted) || !route.selected {
        return Err(
            io::Error::other("session did not establish complete coverage and route").into(),
        );
    }
    if profile.is_none() && route.parallel != (config.completion_workers.get() > 1) {
        return Err(io::Error::other("CPU completion route was not observed").into());
    }
    validate_accounting(&family, profile.is_some())?;
    if profile.is_some()
        && family
            .outcome()
            .formula_execution()
            .and_then(|s| s.gpu_limits)
            != Some(zetesis_solve::FormulaDeviceLimits::from(&config))
    {
        return Err(io::Error::other("effective device limits differ from configuration").into());
    }
    Ok(Run { family, elapsed_ns })
}

fn validate_accounting(family: &WorldView, physical: bool) -> Result<(), io::Error> {
    let Some(execution) = family.outcome().formula_execution() else {
        return if physical {
            Err(io::Error::other("missing physical receipt"))
        } else {
            Ok(())
        };
    };
    if execution.pending_candidates != 0
        || execution.queued_models != 0
        || execution.completion.overflowed
        || execution.completion.failed != 0
        || execution.completion.residual_failed != 0
        || execution.completion.entered != execution.completion.completed
    {
        return Err(io::Error::other("incomplete formula completion accounting"));
    }
    if physical {
        if execution.gpu_limits.is_none()
            || execution.gpu_candidates == 0
            || execution.gpu_work == 0
            || execution.gpu_submitted_batches != execution.gpu_batches
            || execution.gpu_submitted_candidates != execution.gpu_candidates
            || Some(execution.gpu_candidates)
                != execution.gpu_decided.checked_add(execution.cpu_residuals)
        {
            return Err(io::Error::other(
                "incomplete or absent physical candidate accounting",
            ));
        }
    } else if execution.gpu_limits.is_some() {
        return Err(io::Error::other("CPU baseline used a device"));
    }
    Ok(())
}

fn family(run: &Run, admitted: &AdmittedFormula) -> Result<Family, io::Error> {
    let mut result = Family::new();
    for answer in run.family.answer_sets() {
        let model = answer.interpretation();
        if !model.catalog().same_owner(admitted.atom_catalog()) {
            return Err(io::Error::other("answer catalog has another identity"));
        }
        // IDs identify typed atoms in this exact shared catalog, including hidden atoms.
        let record = (
            model.positions().to_vec(),
            answer.score().map(|s| s.costs().to_vec()),
        );
        if !result.insert(record) {
            return Err(io::Error::other("duplicate full answer in complete family"));
        }
    }
    Ok(result)
}

fn adapter(metadata: AdapterMetadata<'_>) -> Value {
    json!({
        "name": metadata.name, "backend": metadata.backend.label(),
        "category": metadata.category.label(), "vendor_id": metadata.vendor_id,
        "device_id": metadata.device_id, "pci_bus_id": metadata.pci_bus_id,
        "driver": metadata.driver, "driver_info": metadata.driver_info,
    })
}

fn native_admission(limits: zetesis_sat::AdmissionLimits) -> Value {
    json!({
        "variables": limits.max_variables,
        "clauses": limits.max_clauses,
        "literals": limits.max_literals,
    })
}

fn configuration(config: SolveConfig, collection: WorldViewLimits) -> Value {
    // Ordinary formula search leaves both native CNF shapes at these library
    // defaults; SolveConfig overrides work/storage but exposes neither shape.
    let native = zetesis_sat::Limits::default();
    json!({
        "backend": if config.backend == Backend::Gpu { "gpu" } else { "cpu" },
        "oracle": "countermodel", "grounder": "eager", "selection": "all",
        "models": config.models, "phase_timings": config.stats,
        "workers": config.workers.get(), "batch_size": config.batch_size.get(),
        "completion_workers": config.completion_workers.get(),
        "max_candidates": config.max_candidates, "max_search_work": config.max_search_work,
        "max_search_decisions": config.max_search_decisions, "max_verification_work": config.max_work,
        "candidate_admission": native_admission(native.admission),
        "reduct_admission": native_admission(native.reduct_admission),
        "max_reduct_bytes": config.max_reduct_bytes,
        "max_completion_scratch_bytes": config.max_completion_scratch_bytes,
        "max_pending_batch_bytes": config.max_batch_bytes,
        "projection": { "entries": config.max_projection_entries, "nodes": config.max_projection_nodes,
            "bytes": config.max_projection_bytes },
        "objective": { "work": config.max_objective_work, "bindings": config.max_objective_bindings,
            "keys": config.max_objective_keys, "key_bytes": config.max_objective_key_bytes,
            "bound_work": 0 },
        "collection": { "answer_sets": collection.max_answer_sets, "atoms": collection.max_atoms,
            "bytes": collection.max_bytes },
        "device": (config.backend == Backend::Gpu).then(|| json!({
            "max_batch_bytes": config.max_batch_bytes, "work_per_candidate": config.gpu_formula_work,
            "rounds_per_candidate": config.gpu_formula_rounds,
            "dispatch_timeout_ns": zetesis_wgpu::FormulaLimits::default().timeout.as_nanos(),
        })),
    })
}

fn execution(run: &Run, profile: Option<&GpuFormulaProfile>) -> Value {
    let receipt = run.family.outcome().formula_execution();
    json!({
        "adapter": profile.map(|p| adapter(p.info().metadata())),
        "gpu": receipt.filter(|s| s.gpu_limits.is_some()).map(|s| json!({
            "batches": s.gpu_batches, "candidates": s.gpu_candidates, "work": s.gpu_work,
            "rounds": s.gpu_rounds, "decided": s.gpu_decided,
            "peak_accounted_bytes": s.peak_accounted_bytes,
        })),
        "completion": receipt.map(|s| json!({
            "cpu_residuals": s.cpu_residuals, "entered": s.completion.entered,
            "completed": s.completion.completed, "residual_completed": s.completion.residual_completed,
            "requested_workers": s.completion.requested_workers,
            "effective_workers": s.completion.effective_workers,
            "requested_scratch_bytes": s.completion.requested_scratch_bytes,
            "peak_scratch_bytes": s.completion.peak_scratch_bytes,
        })),
    })
}

struct Sample<'a> {
    mode: &'static str,
    repetition: usize,
    position: usize,
    config: SolveConfig,
    profile: Option<&'a GpuFormulaProfile>,
}

fn write_record(
    output: &mut impl Write,
    run: &Run,
    identity: &Value,
    sample: &Sample<'_>,
    collection: WorldViewLimits,
) -> Result<(), Box<dyn Error>> {
    let statistics = run
        .family
        .outcome()
        .countermodel_statistics()
        .ok_or_else(|| io::Error::other("formula search receipt is absent"))?;
    serde_json::to_writer(
        &mut *output,
        &json!({
            "event": "sample", "mode": sample.mode, "repetition": sample.repetition,
            "position": sample.position, "identity": identity,
            "configuration": configuration(sample.config, collection),
            "completion": "exhausted", "session_ns": run.elapsed_ns,
            "answer_sets": run.family.len(), "candidates": statistics.candidates,
            "search_work": statistics.search.work, "countermodel_queries": statistics.countermodel_queries,
            "reduct": { "preparation": statistics.reduct.preparation.map(|s| json!({
                "work": s.work, "retained_bytes": s.retained_bytes, "peak_bytes": s.peak_bytes,
            })), "original_work": statistics.reduct.original_work,
                "parameter_work": statistics.reduct.parameter_work,
                "peak_workspace_bytes": statistics.reduct.peak_workspace_bytes },
            "execution": execution(run, sample.profile),
        }),
    )?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

fn profiles(
    backend: zetesis_experiments::Backend,
) -> Result<Vec<GpuFormulaProfile>, Box<dyn Error>> {
    let backend = match backend {
        zetesis_experiments::Backend::Cpu => return Ok(Vec::new()),
        zetesis_experiments::Backend::Metal => GpuBackendPreference::Metal,
        zetesis_experiments::Backend::Vulkan => GpuBackendPreference::Vulkan,
    };
    let context = GpuContext::new_selected(
        GpuOptions::default(),
        GpuSelection {
            backend,
            vendor_id: None,
        },
    )?;
    let profiles = [GateProjection::Enumerated, GateProjection::Bitwise]
        .into_iter()
        .map(|projection| GpuFormulaProfile::from_context_with_projection(&context, projection))
        .collect::<Result<Vec<_>, _>>()?;
    if !context.info().is_hardware_gpu()
        || profiles
            .iter()
            .any(|profile| !profile.context().same_instance(&context))
    {
        return Err(io::Error::other("profiles must retain the same physical context").into());
    }
    Ok(profiles)
}

fn source(path: &Path) -> Result<(AdmittedFormula, String), Box<dyn Error>> {
    let mut source = String::new();
    File::open(path)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_string(&mut source)?;
    if source.len() as u128 > u128::from(MAX_SOURCE_BYTES) {
        return Err(io::Error::other("source exceeds the 1 MiB experiment limit").into());
    }
    let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
    let admitted = admit_formula(
        source,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    Ok((admitted, hash))
}

fn executable_hash() -> Result<String, io::Error> {
    let mut file = File::open(std::env::current_exe()?)?;
    let mut hash = Sha256::new();
    let mut bytes = [0_u8; 16_384];
    loop {
        let read = file.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        hash.update(&bytes[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn write_configuration(
    output: &mut impl Write,
    options: &Options,
    admitted: &AdmittedFormula,
    identity: &Value,
    reference_answers: usize,
) -> Result<(), Box<dyn Error>> {
    serde_json::to_writer(
        &mut *output,
        &json!({
            "event": "configuration", "identity": identity, "warmups": options.warmups,
            "requested_backend": match options.backend {
                zetesis_experiments::Backend::Cpu => "cpu",
                zetesis_experiments::Backend::Metal => "metal",
                zetesis_experiments::Backend::Vulkan => "vulkan",
            },
            "repetitions": options.repetitions.get(), "routes_per_round": if options.backend == zetesis_experiments::Backend::Cpu { 2 } else { 6 },
            "max_source_bytes": MAX_SOURCE_BYTES, "admission": "library defaults for this executable",
            "matched_context": (options.backend != zetesis_experiments::Backend::Cpu).then_some(true), "reference_answer_sets": reference_answers,
            "atoms": admitted.theory().atom_count(), "nodes": admitted.theory().nodes().len(),
            "roots": admitted.theory().roots().len(),
        }),
    )?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse();
    if options.workers.get() < 2 {
        return Err(
            io::Error::other("--workers must be at least two for the Rayon baseline").into(),
        );
    }
    let rounds = options
        .warmups
        .checked_add(options.repetitions.get())
        .ok_or_else(|| io::Error::other("repetition count overflow"))?;
    let (admitted, hash) = source(&options.source)?;
    let device = profiles(options.backend)?;
    let collection = WorldViewLimits::default();
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        oracle: Oracle::Countermodel,
        models: 0,
        workers: options.workers,
        completion_workers: options.workers,
        batch_size: options.batch_size,
        ..SolveConfig::default()
    };
    let scalar = SolveConfig {
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..config
    };
    let expected = family(&measure(&admitted, scalar, None, collection)?, &admitted)?;
    let identity = json!({
        "schema": "zetesis.session-projection.v1", "version": env!("CARGO_PKG_VERSION"),
        "source_sha256": hash, "executable_sha256": executable_hash()?,
    });
    let mut output = io::stdout().lock();
    write_configuration(&mut output, &options, &admitted, &identity, expected.len())?;
    let mut samples = 0_usize;
    for round in 0..rounds {
        for (position, (mode, selected, profile)) in [
            ("scalar", scalar, None),
            ("rayon", config, None),
            ("enumerated", config, device.first()),
            ("bitwise", config, device.get(1)),
            ("bitwise", config, device.get(1)),
            ("enumerated", config, device.first()),
        ]
        .into_iter()
        .enumerate()
        {
            if position >= 2 && profile.is_none() {
                continue;
            }
            let selected = if profile.is_some() {
                SolveConfig {
                    backend: Backend::Gpu,
                    ..selected
                }
            } else {
                selected
            };
            let run = measure(&admitted, selected, profile, collection)?;
            if family(&run, &admitted)? != expected {
                return Err(
                    io::Error::other("complete models or objective scores disagree").into(),
                );
            }
            if round >= options.warmups {
                write_record(
                    &mut output,
                    &run,
                    &identity,
                    &Sample {
                        mode,
                        repetition: round - options.warmups,
                        position,
                        config: selected,
                        profile,
                    },
                    collection,
                )?;
                samples = samples
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("sample count overflow"))?;
            }
        }
    }
    serde_json::to_writer(
        &mut output,
        &json!({ "event": "complete", "samples": samples }),
    )?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> AdmittedFormula {
        admit_formula(
            "{a;b}. #minimize {1,k:a}. #show a/0.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
    }

    fn config() -> SolveConfig {
        SolveConfig {
            backend: Backend::Cpu,
            oracle: Oracle::Countermodel,
            grounder: Grounder::Eager,
            models: 0,
            batch_size: NonZeroUsize::new(4).unwrap(),
            workers: NonZeroUsize::MIN,
            completion_workers: NonZeroUsize::MIN,
            ..SolveConfig::default()
        }
    }

    #[test]
    fn ordinary_cpu_routes_preserve_the_full_scored_family() {
        let admitted = input();
        let scalar = measure(&admitted, config(), None, WorldViewLimits::default()).unwrap();
        // The batched completion receipts read below belong to the clause
        // method; regions with several workers decide leaves on the workers.
        let parallel_config = SolveConfig {
            search: zetesis_solve::SearchMethod::Clauses,
            workers: NonZeroUsize::new(2).unwrap(),
            completion_workers: NonZeroUsize::new(2).unwrap(),
            ..config()
        };
        let parallel =
            measure(&admitted, parallel_config, None, WorldViewLimits::default()).unwrap();
        let expected = BTreeSet::from([
            (vec![], Some(vec![(0, 0)])),
            (vec![0], Some(vec![(0, 1)])),
            (vec![1], Some(vec![(0, 0)])),
            (vec![0, 1], Some(vec![(0, 1)])),
        ]);
        assert_eq!(family(&scalar, &admitted).unwrap(), expected);
        assert_eq!(family(&parallel, &admitted).unwrap(), expected);
        assert!(scalar.family.outcome().formula_execution().is_none());
        assert!(
            parallel
                .family
                .outcome()
                .formula_execution()
                .unwrap()
                .completion
                .effective_workers
                > 0
        );
    }

    #[test]
    fn cpu_records_keep_device_observations_absent() {
        let admitted = input();
        for workers in [1, 2] {
            let config = SolveConfig {
                completion_workers: NonZeroUsize::new(workers).unwrap(),
                ..config()
            };
            let run = measure(&admitted, config, None, WorldViewLimits::default()).unwrap();
            let mut bytes = Vec::new();
            write_record(
                &mut bytes,
                &run,
                &json!({"source_sha256": "fixture"}),
                &Sample {
                    mode: "cpu",
                    repetition: 0,
                    position: 0,
                    config,
                    profile: None,
                },
                WorldViewLimits::default(),
            )
            .unwrap();
            let record: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(record["execution"]["adapter"].is_null());
            assert!(record["execution"]["gpu"].is_null());
            assert_eq!(record["execution"]["completion"].is_null(), workers == 1);
            assert_eq!(
                record["configuration"]["max_reduct_bytes"],
                config.max_reduct_bytes
            );
            let native = zetesis_sat::Limits::default();
            assert_eq!(
                record["configuration"]["candidate_admission"],
                native_admission(native.admission)
            );
            assert_eq!(
                record["configuration"]["reduct_admission"],
                native_admission(native.reduct_admission)
            );
        }
    }

    #[test]
    fn collection_refusal_keeps_the_original_checked_prefix() {
        let admitted = input();
        let error = measure(
            &admitted,
            config(),
            None,
            WorldViewLimits {
                max_answer_sets: 1,
                ..WorldViewLimits::default()
            },
        )
        .err()
        .unwrap();
        let failure = error
            .downcast_ref::<zetesis_solve::WorldViewFailure>()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            zetesis_solve::WorldViewError::AnswerSets
        ));
        assert_eq!(failure.answer_sets().len(), 1);
        assert!(failure.outcome().is_some());
    }

    #[test]
    #[ignore = "requires a physical Metal device"]
    fn matched_metal_profiles_preserve_the_full_scored_family() {
        let admitted = input();
        let expected = family(
            &measure(&admitted, config(), None, WorldViewLimits::default()).unwrap(),
            &admitted,
        )
        .unwrap();
        let profiles = profiles(zetesis_experiments::Backend::Metal).unwrap();
        for profile in &profiles {
            let run = measure(
                &admitted,
                SolveConfig {
                    backend: Backend::Gpu,
                    ..config()
                },
                Some(profile),
                WorldViewLimits::default(),
            )
            .unwrap();
            assert_eq!(family(&run, &admitted).unwrap(), expected);
        }
    }
}
