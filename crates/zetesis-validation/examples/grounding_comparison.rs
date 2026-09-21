//! Compare eager and lazy grounding with the same executable and CPU workers.
//!
//! Clingo qualifies complete selected families but is not timed. Native full
//! identities are checked across profiles and repetitions. The small queens and
//! generated controls expose both retained-storage opportunities and the cost
//! of checking constraints after core enumeration; no speedup is presumed.

use std::{
    error::Error,
    io::{self, Write},
    num::NonZeroUsize,
    path::PathBuf,
    time::Duration,
};

use clap::Parser;
use zetesis_validation::{
    answers::native_json,
    examples,
    performance::{self, families::Family, matrix},
    selected::{Backend, FormulaJoins, Grounder, NativeExecution, SearchMethod},
};

#[derive(Parser)]
#[command(about = "Compare eager/lazy CPU profiles on nine bounded source workloads")]
struct Options {
    /// Verified repository corpus; no files are downloaded or edited.
    #[arg(long, default_value = "examples/kr-domains")]
    corpus: PathBuf,
    /// Absolute current zetesis executable supporting the solve command.
    #[arg(long)]
    zetesis: PathBuf,
    /// Absolute stock clingo executable used for qualification only.
    #[arg(long)]
    clingo: PathBuf,
    /// Absolute current zetesis executable providing the bounded RSS helper.
    #[arg(long)]
    helper: PathBuf,
    /// New complete evidence file; existing files are never replaced.
    #[arg(long)]
    report: PathBuf,
    /// Identical native worker count in both profiles; completion uses one.
    #[arg(long, default_value = "1")]
    workers: NonZeroUsize,
}

fn plan(workers: NonZeroUsize) -> Result<matrix::Plan, performance::Error> {
    let profile = NativeExecution {
        backend: Backend::Cpu,
        workers,
        completion_workers: NonZeroUsize::MIN,
        formula_joins: Some(FormulaJoins::Indexed),
        search: Some(SearchMethod::Regions),
        ..NativeExecution::default()
    };
    matrix::Plan::new(
        // This suite bounds the allowed corpus entries. Generated workloads
        // carry their independent constructor-checked source contracts.
        matrix::Suite::Queens,
        [Grounder::Eager, Grounder::Lazy]
            .map(|grounder| NativeExecution {
                grounder,
                ..profile
            })
            .to_vec(),
        NonZeroUsize::MIN,
        1,
        4,
    )?
    .with_reference(matrix::ReferencePolicy::QualificationOnly)
    .with_memory(2)
}

fn workloads(corpus: &examples::Corpus) -> Result<Vec<matrix::Workload>, performance::Error> {
    let limits = matrix::WorkloadLimits::default();
    let mut workloads = Vec::with_capacity(9);
    for (variant, size) in [(1, 4), (1, 5), (3, 4), (3, 5), (2, 4), (6, 4)] {
        let entry = format!("standalone/n-queens/variant-{variant:02}.lp");
        workloads.push(matrix::Workload::amended(
            corpus,
            &entry,
            &[matrix::ConstantAmendment {
                source_path: &entry,
                name: "n",
                expected: 8,
                replacement: size,
            }],
            limits,
        )?);
    }
    for (family, size) in [
        (Family::MonotoneChoices, 6),
        (Family::RedundantTransitivity, 8),
        (Family::RedundantTransitivity, 12),
    ] {
        workloads.push(matrix::Workload::generated(family, size, limits)?);
    }
    Ok(workloads)
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse();
    let mut limits = performance::Limits::default();
    limits.process.timeout = Duration::from_secs(10);
    limits.campaign_timeout = Duration::from_mins(3);
    let corpus = examples::load(&options.corpus, limits.corpus)?;
    let workloads = workloads(&corpus)?;
    let request = matrix::Request {
        corpus: &options.corpus,
        native: &options.zetesis,
        reference: &options.clingo,
        report: &options.report,
        plan: plan(options.workers)?,
        limits,
        native_answers: native_json::Limits::default(),
        max_spelling_bytes: limits.answers.max_input_bytes,
        helper: Some(&options.helper),
    };
    writeln!(
        io::stderr().lock(),
        "Recording nine workloads in {}",
        options.report.display()
    )?;
    let report = matrix::run_workloads_with_invocation(
        &request,
        &workloads,
        matrix::NativeInvocation::Solve,
    )?;
    report.publish()?;
    serde_json::to_writer(io::stdout().lock(), &report.summary())?;
    writeln!(io::stdout().lock())?;
    if !report.passed() {
        return Err("comparison did not fully pass; inspect the retained report".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, path::Path};
    use zetesis_validation::performance::Phase;

    #[test]
    fn workload_population_has_nine_distinct_identities() {
        let corpus = examples::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains"),
            examples::Limits::default(),
        )
        .unwrap();
        let workloads = workloads(&corpus).unwrap();
        assert_eq!(workloads.len(), 9);
        assert_eq!(
            workloads
                .iter()
                .map(matrix::Workload::identity)
                .collect::<BTreeSet<_>>()
                .len(),
            9
        );
        assert_eq!(
            workloads
                .iter()
                .filter(|workload| workload.is_amended())
                .count(),
            6
        );
        assert_eq!(
            workloads
                .iter()
                .filter(|workload| workload.is_generated())
                .count(),
            3
        );
    }

    #[test]
    fn reference_census_is_separate_from_native_measurements() {
        let slots = plan(NonZeroUsize::MIN).unwrap().slots(9).unwrap();
        assert_eq!(slots.len(), 153);
        let reference: Vec<_> = slots
            .iter()
            .filter(|slot| slot.producer == matrix::Producer::Reference)
            .collect();
        assert_eq!(reference.len(), 9);
        assert!(
            reference
                .iter()
                .all(|slot| slot.phase == Phase::Qualification)
        );
    }
}
