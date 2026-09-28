//! Compare eager and lazy grounding with the same executable and CPU workers.
//!
//! Clingo qualifies complete selected families but is not timed. Native full
//! identities are checked across profiles and repetitions. Two fixed populations
//! examine retained storage or early region refutation, including controls where
//! repeated constraint scans may cost more than they save. No speedup is presumed.

use std::{
    error::Error,
    io::{self, Write},
    num::NonZeroUsize,
    path::PathBuf,
    time::Duration,
};

use clap::{Parser, ValueEnum};
use zetesis_validation::{
    answers::native_json,
    examples,
    performance::{self, families::Family, matrix},
    selected::{Backend, FormulaJoins, Grounder, NativeExecution, SearchMethod},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Study {
    /// Original nine cases: six small queens workloads and three generated controls.
    Storage,
    /// Twenty cases: all six queens encodings at n=4/5, selected n=6 and six generated controls.
    Refutation,
}

impl Study {
    const fn queens(self) -> &'static [(u8, i32)] {
        match self {
            Self::Storage => &[(1, 4), (1, 5), (3, 4), (3, 5), (2, 4), (6, 4)],
            Self::Refutation => &[
                (1, 4),
                (1, 5),
                (1, 6),
                (3, 4),
                (3, 5),
                (3, 6),
                (2, 4),
                (2, 5),
                (4, 4),
                (4, 5),
                (5, 4),
                (5, 5),
                (6, 4),
                (6, 5),
            ],
        }
    }

    const fn generated(self) -> &'static [(Family, u32)] {
        match self {
            Self::Storage => &[
                (Family::MonotoneChoices, 6),
                (Family::RedundantTransitivity, 8),
                (Family::RedundantTransitivity, 12),
            ],
            Self::Refutation => &[
                (Family::MonotoneChoices, 6),
                (Family::MonotoneChoices, 8),
                (Family::MonotoneChoices, 10),
                (Family::RedundantTransitivity, 8),
                (Family::RedundantTransitivity, 12),
                (Family::RedundantTransitivity, 16),
            ],
        }
    }

    const fn campaign_timeout(self) -> Duration {
        match self {
            Self::Storage => Duration::from_mins(3),
            Self::Refutation => Duration::from_mins(5),
        }
    }
}

#[derive(Parser)]
#[command(about = "Compare eager/lazy CPU profiles on a fixed, bounded source population")]
struct Options {
    /// Finite workload population; storage preserves the original nine-case order.
    #[arg(long, value_enum, default_value = "storage")]
    study: Study,
    /// Verified repository corpus; no files are downloaded or edited.
    #[arg(long, default_value = "examples/correctness")]
    corpus: PathBuf,
    /// Absolute current zetesis executable supporting the solve command.
    #[arg(long)]
    zetesis: PathBuf,
    /// Absolute stock clingo executable used for qualification only.
    #[arg(long)]
    clingo: PathBuf,
    /// Absolute `zetesis-bench` executable, whose measurement helper runs the
    /// bounded RSS rounds.
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

fn queen(
    corpus: &examples::Corpus,
    variant: u8,
    size: i32,
) -> Result<matrix::Workload, performance::Error> {
    let entry = format!("standalone/n-queens/variant-{variant:02}.lp");
    matrix::Workload::amended(
        corpus,
        &entry,
        &[matrix::ConstantAmendment {
            source_path: &entry,
            name: "n",
            expected: 8,
            replacement: size,
        }],
        matrix::WorkloadLimits::default(),
    )
}

fn workloads(
    corpus: &examples::Corpus,
    study: Study,
) -> Result<Vec<matrix::Workload>, performance::Error> {
    let limits = matrix::WorkloadLimits::default();
    let mut workloads = Vec::with_capacity(study.queens().len() + study.generated().len());
    for &(variant, size) in study.queens() {
        workloads.push(queen(corpus, variant, size)?);
    }
    for &(family, size) in study.generated() {
        workloads.push(matrix::Workload::generated(family, size, limits)?);
    }
    Ok(workloads)
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = Options::parse();
    let mut limits = performance::Limits::default();
    limits.process.timeout = Duration::from_secs(10);
    limits.campaign_timeout = options.study.campaign_timeout();
    let corpus = examples::load(&options.corpus, limits.corpus)?;
    let workloads = workloads(&corpus, options.study)?;
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
        "Recording {} workloads in {}",
        workloads.len(),
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

    fn corpus() -> examples::Corpus {
        examples::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/correctness"),
            examples::Limits::default(),
        )
        .unwrap()
    }

    #[test]
    fn default_study_preserves_storage() {
        let options = Options::try_parse_from([
            "grounding_comparison",
            "--zetesis",
            "zetesis",
            "--clingo",
            "clingo",
            "--helper",
            "zetesis",
            "--report",
            "report.json",
        ])
        .unwrap();
        assert_eq!(options.study, Study::Storage);
    }

    #[test]
    fn storage_population_has_nine_distinct_identities() {
        let workloads = workloads(&corpus(), Study::Storage).unwrap();
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
    fn refutation_population_has_twenty_distinct_identities() {
        let workloads = workloads(&corpus(), Study::Refutation).unwrap();
        assert_eq!(workloads.len(), 20);
        assert_eq!(
            workloads
                .iter()
                .map(matrix::Workload::identity)
                .collect::<BTreeSet<_>>()
                .len(),
            20
        );
    }

    #[test]
    fn refutation_covers_each_queens_variant_at_both_sizes() {
        let corpus = corpus();
        let workloads = workloads(&corpus, Study::Refutation).unwrap();
        let identities: BTreeSet<_> = workloads.iter().map(matrix::Workload::identity).collect();
        for variant in 1..=6 {
            for size in [4, 5] {
                let expected = queen(&corpus, variant, size).unwrap();
                assert!(identities.contains(expected.identity()));
            }
        }
    }

    #[test]
    fn refutation_retains_storage_workload_identities() {
        let corpus = corpus();
        let storage = workloads(&corpus, Study::Storage).unwrap();
        let refutation = workloads(&corpus, Study::Refutation).unwrap();
        let identities: BTreeSet<_> = refutation.iter().map(matrix::Workload::identity).collect();
        assert!(
            storage
                .iter()
                .all(|workload| identities.contains(workload.identity()))
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

    #[test]
    fn refutation_schedule_accounts_for_all_twenty_cases() {
        let slots = plan(NonZeroUsize::MIN).unwrap().slots(20).unwrap();
        assert_eq!(slots.len(), 340);
        for case in 0..20 {
            let positions: Vec<_> = slots.iter().filter(|slot| slot.case == case).collect();
            assert_eq!(positions.len(), 17);
            let reference: Vec<_> = positions
                .iter()
                .filter(|slot| slot.producer == matrix::Producer::Reference)
                .collect();
            assert_eq!(reference.len(), 1);
            assert_eq!(reference[0].phase, Phase::Qualification);
        }
    }
}
