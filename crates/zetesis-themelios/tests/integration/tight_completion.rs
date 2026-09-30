//! Complete ordinary batch enumeration with a checked class oracle injected.
//! These are semantic/work-count experiments, not timing or device benchmarks.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use zetesis_core::{AtomCatalog, Model};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    Theory, TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightVerdict,
};
use zetesis_objective::{ObjectiveProgram, Score, evaluate};
use zetesis_sat::{BatchLimits, BatchVerdict, CompletionExecutor, StableModels};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

type ScoredModels = BTreeMap<Vec<usize>, Score>;

struct Run {
    models: ScoredModels,
    statistics: zetesis_sat::Statistics,
    batch: zetesis_sat::BatchStatistics,
}

fn run(
    theory: &Theory,
    atoms: &AtomCatalog,
    objective: &ObjectiveProgram,
    certificate: Option<&TightPlan>,
    workers: usize,
) -> Run {
    let cancellation = Cancellation::default();
    let mut search =
        StableModels::new(theory, zetesis_sat::Limits::default(), cancellation.clone()).unwrap();
    let mut executor = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
    let batch_limits = BatchLimits {
        max_candidates: NonZeroUsize::new(17).unwrap(),
        max_pending_bytes: 16 * 1024 * 1024,
    };
    let mut models = BTreeMap::new();
    let mut certificate_work = 0;
    while !search.exhausted() {
        let batch = search
            .next_batch_with_completion(batch_limits, &mut executor, |program, pending| {
                let mut verdicts = Vec::new();
                for candidate in pending {
                    let verdict = if let Some(plan) = certificate {
                        assert!(program.same_instance(plan.theory()));
                        let checked = plan.check(
                            candidate,
                            TightCheckLimits {
                                max_work: 100_000_000 - certificate_work,
                                ..TightCheckLimits::default()
                            },
                            &cancellation,
                        )?;
                        certificate_work += checked.work;
                        match checked.verdict {
                            TightVerdict::Stable => BatchVerdict::NoProperSubset,
                            TightVerdict::Residual { .. } => BatchVerdict::Residual,
                            // The producer independently checked original truth. A
                            // disagreement is a failed checker, never model rejection.
                            TightVerdict::NotModel { .. } => {
                                return Err(TightError::Stopped(Stop::InvalidProgram));
                            }
                        }
                    } else {
                        BatchVerdict::Residual
                    };
                    verdicts.push(verdict);
                }
                Ok::<_, TightError>(verdicts)
            })
            .expect("complete candidate batch, including all exact residuals");
        for candidate in batch {
            assert!(candidate.theory().same_instance(theory));
            let model = Model::from_positions(atoms, candidate.atoms()).unwrap();
            let score = evaluate(
                objective,
                &model,
                zetesis_objective::Limits::default(),
                &cancellation,
            )
            .expect("complete model score")
            .score()
            .clone();
            assert!(
                models.len() < 100_000,
                "explicit retained experiment model cap"
            );
            assert!(
                models.insert(candidate.atoms().collect(), score).is_none(),
                "unique complete model"
            );
        }
    }
    let statistics = search.statistics();
    let batch = search.batch_statistics();
    assert_eq!(batch.pending, 0);
    assert_eq!(batch.committed, statistics.candidates);
    assert_eq!(batch.propagated + batch.residuals, batch.committed);
    assert_eq!(models.len() as u64, statistics.stable_models);
    Run {
        models,
        statistics,
        batch,
    }
}

fn optimum(models: &ScoredModels) -> BTreeSet<Vec<usize>> {
    let best = models.values().min_by(|a, b| a.compare_costs(b));
    models
        .iter()
        .filter(|(_, score)| best.is_some_and(|best| score.compare_costs(best) == Ordering::Equal))
        .map(|(atoms, _)| atoms.clone())
        .collect()
}

fn compare(
    theory: &Theory,
    atoms: &AtomCatalog,
    objectives: &ObjectiveProgram,
    workers: usize,
) -> (Run, Run) {
    let plan =
        match TightPlan::compile(theory, TightPlanLimits::default(), &Cancellation::default()) {
            Ok(plan) => Some(plan),
            Err(
                TightError::PositiveCycle { .. }
                | TightError::UnsupportedBody { .. }
                | TightError::UnsupportedRoot { .. },
            ) => None,
            Err(error) => panic!("unexpected incomplete certification: {error}"),
        };
    let baseline = run(theory, atoms, objectives, None, 1);
    let certified = run(theory, atoms, objectives, plan.as_ref(), workers);
    assert_eq!(certified.models, baseline.models);
    assert_eq!(optimum(&certified.models), optimum(&baseline.models));
    assert_eq!(
        certified.statistics.candidates,
        baseline.statistics.candidates
    );
    assert_eq!(
        certified.statistics.countermodel_queries + certified.batch.propagated,
        baseline.statistics.countermodel_queries
    );
    if plan.is_none() {
        assert_eq!(certified.batch.propagated, 0);
    }
    (baseline, certified)
}

#[test]
fn complete_models_scores_ties_and_cyclic_fallback_match_with_one_two_four_workers() {
    for (source, count, ties) in [
        ("1{a;b}1. #minimize{1,k:a;1,k:b}.", 2, 2),
        ("{a;b}. #maximize{1@1,a:a;1@1,b:b}.", 4, 1),
        ("{a;b}. #minimize{-1@1,a:a;0@0,k:b}.", 4, 2),
        ("a|b. a:-b. b:-a.", 1, 1),
        ("{a;b}. :- a,b.", 3, 3),
        ("a. :- a.", 0, 0),
    ] {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        for workers in [1, 2, 4] {
            let (_, actual) = compare(
                admitted.theory(),
                admitted.atom_catalog(),
                admitted.objectives(),
                workers,
            );
            assert_eq!(actual.models.len(), count, "{source}");
            assert_eq!(optimum(&actual.models).len(), ties, "{source}");
        }
    }
}
