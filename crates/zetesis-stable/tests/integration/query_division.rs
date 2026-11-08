//! Dividing candidates' proper-subset queries among the region workers
//! preserves the answer-set family on the 2QBF workloads, whose subset queries
//! are the solve, and on the random 2QBF workload, whose many queries are
//! small and often refuted.
//!
//! Under forced division every query that reaches the subset query on a
//! worker is published, so the published and served receipts are a property
//! of the run, not of its schedule. Which worker serves a region, and whether
//! any was stolen, is the schedule's: the deterministic unit tests of the
//! walk's published queries exercise stealing, and these tests do not claim
//! it. Each configuration runs several times to sample interleavings; this is
//! not exhaustive schedule coverage.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zetesis_ferraris::Theory;
use zetesis_reference_support::formula;
use zetesis_stable::{
    Cancellation, Engagement, LearnedStore, Limits, QueryDivision, QueryDivisionStatistics,
    StableModels,
};

/// Far above any correct run of these small instances, so a counting fault
/// that hangs a walk fails the test instead of wedging the suite.
const HANG_GUARD: Duration = Duration::from_mins(1);
/// Runs per configuration.
const RUNS: usize = 4;

const PARITY: &str =
    include_str!("../../../../examples/scalability/two-qbf-parity/two_qbf_parity.lp");
const TSEITIN: &str =
    include_str!("../../../../examples/scalability/two-qbf-tseitin/two_qbf_tseitin.lp");
const PIGEONHOLE: &str =
    include_str!("../../../../examples/scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp");
const RANDOM: &str = include_str!("../../../../examples/scalability/two-qbf/two_qbf.lp");

/// `source` with one constant's definition replaced, as a `#const` amendment
/// would set it.
fn with_constant(source: &str, name: &str, from: u32, to: u32) -> String {
    let old = format!("#const {name} = {from}.");
    assert!(source.contains(&old), "{name} is defined as {from}");
    source.replace(&old, &format!("#const {name} = {to}."))
}

/// Each workload's default and the variants whose families differ: with no
/// answer set, and with a second one, where the variant has it.
fn workloads() -> Vec<(String, Theory)> {
    let mut sources = vec![("random".to_owned(), RANDOM.to_owned())];
    for (name, source) in [
        ("parity", PARITY),
        ("tseitin", TSEITIN),
        ("pigeonhole", PIGEONHOLE),
    ] {
        sources.push((name.to_owned(), source.to_owned()));
        sources.push((
            format!("{name} stable=0"),
            with_constant(source, "stable", 1, 0),
        ));
        sources.push((
            format!("{name} stable=2"),
            with_constant(source, "stable", 1, 2),
        ));
    }
    sources.push(("parity n=9".to_owned(), with_constant(PARITY, "n", 6, 9)));
    sources
        .into_iter()
        .map(|(name, source)| {
            let theory = formula(&source).theory().clone();
            (name, theory)
        })
        .collect()
}

/// The complete family under `workers` region walkers dividing their queries
/// as `division`, with the walk's division receipts; one worker is the scalar
/// walk, which has none. With `learned`, the walk shares a fresh learned store.
fn family(
    theory: &Theory,
    workers: usize,
    engagement: Engagement,
    division: QueryDivision,
    learned: bool,
) -> (BTreeSet<Vec<usize>>, Option<QueryDivisionStatistics>) {
    let mut search = StableModels::with_region_workers_engaging(
        theory,
        NonZeroUsize::new(workers).unwrap(),
        engagement,
        Limits::default(),
        Cancellation::with_deadline(Instant::now() + HANG_GUARD).unwrap(),
    )
    .unwrap();
    search.set_query_division(division);
    if learned {
        search
            .set_learned_store(Arc::new(LearnedStore::new()))
            .unwrap();
    }
    let mut models = BTreeSet::new();
    for model in search.by_ref() {
        assert!(
            models.insert(model.unwrap().atoms().collect()),
            "a model was emitted twice"
        );
    }
    assert!(
        search.exhausted(),
        "the walk stopped before covering the candidates"
    );
    let receipts = search
        .statistics()
        .regions
        .unwrap()
        .workers
        .map(|workers| workers.query_division);
    if let Some(receipts) = receipts {
        assert_eq!(receipts.division, division);
    }
    (models, receipts)
}

/// [`family`] under several workers, which keep division receipts.
fn divided(
    theory: &Theory,
    workers: usize,
    engagement: Engagement,
    division: QueryDivision,
) -> (BTreeSet<Vec<usize>>, QueryDivisionStatistics) {
    let (models, receipts) = family(theory, workers, engagement, division, false);
    (
        models,
        receipts.expect("several workers keep division receipts"),
    )
}

#[test]
fn forced_division_returns_the_undivided_family() {
    for (name, theory) in workloads() {
        let (expected, _) = family(
            &theory,
            1,
            Engagement::default(),
            QueryDivision::Never,
            false,
        );
        let (undivided, receipts) =
            divided(&theory, 2, Engagement::Immediate, QueryDivision::Never);
        assert_eq!(undivided, expected, "{name}");
        assert_eq!(
            (receipts.published, receipts.served),
            (0, 0),
            "{name}: an undivided walk published"
        );
        for workers in [2, 14] {
            for _ in 0..RUNS {
                let (models, receipts) = divided(
                    &theory,
                    workers,
                    Engagement::Immediate,
                    QueryDivision::Immediate,
                );
                assert_eq!(models, expected, "{name} at {workers} workers");
                // Every query a worker entered was published, and each
                // published query's first region was narrowed.
                assert!(receipts.published >= 1, "{name}: {receipts:?}");
                assert!(
                    receipts.served >= receipts.published,
                    "{name}: {receipts:?}"
                );
                assert!(receipts.servers >= 1 && receipts.peak_served <= receipts.served);
                assert!(receipts.stolen <= receipts.served);
            }
        }
    }
}

#[test]
fn division_when_idle_returns_the_undivided_family() {
    for (name, theory) in workloads() {
        let (expected, _) = family(
            &theory,
            1,
            Engagement::default(),
            QueryDivision::Never,
            false,
        );
        for _ in 0..RUNS {
            let (models, receipts) =
                divided(&theory, 14, Engagement::default(), QueryDivision::WhenIdle);
            assert_eq!(models, expected, "{name}");
            assert!(receipts.stolen <= receipts.served, "{name}: {receipts:?}");
        }
    }
}

#[test]
fn the_parity_workload_has_its_recorded_families() {
    // The families the oracle comparison records for these variants: the
    // first `stable` existential assignments, as atoms of the shown `nx`.
    let sizes: Vec<usize> = [0, 1, 2]
        .into_iter()
        .map(|stable| {
            let source = with_constant(PARITY, "stable", 1, stable);
            let theory = formula(&source).theory().clone();
            divided(&theory, 14, Engagement::Immediate, QueryDivision::Immediate)
                .0
                .len()
        })
        .collect();
    assert_eq!(sizes, [0, 1, 2]);
}

/// The candidate walk's learned tiers seed and deliver while workers serve
/// published query regions, whose narrowing reads a frozen reduct and so
/// suspends the ledger: sharing changes neither the family nor the division's
/// published and served receipts.
#[test]
fn division_beside_a_learned_store_returns_the_undivided_family() {
    for (name, theory) in workloads() {
        let (expected, _) = family(
            &theory,
            1,
            Engagement::default(),
            QueryDivision::Never,
            false,
        );
        for workers in [2, 14] {
            for _ in 0..RUNS {
                let (models, receipts) = family(
                    &theory,
                    workers,
                    Engagement::Immediate,
                    QueryDivision::Immediate,
                    true,
                );
                let receipts = receipts.expect("several workers keep division receipts");
                assert_eq!(models, expected, "{name} at {workers} workers");
                assert!(receipts.published >= 1, "{name}: {receipts:?}");
                assert!(
                    receipts.served >= receipts.published,
                    "{name}: {receipts:?}"
                );
            }
        }
    }
}
