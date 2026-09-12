//! Test-only bounded work counters; there is no clock or production observer.
//! The ordinary optimizer and reduct checks are outside this classical-prefix
//! profile. Every source uses the real bundle loader, admission and encoder.

use std::cell::Cell;
use std::path::Path;

use super::{Budget, Cursor, LocalQuota};
use crate::{
    AdmissionLimits, Control, Incomplete, SearchLimits, SearchStatistics, Solve, encoding,
};
use zetesis_themelios::{
    BundleAdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits, SourceBundle,
    admit_bundle_formula,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Counts {
    pub(super) watch_visits: u64,
    pub(super) binary_attempts: u64,
    pub(super) longer_attempts: u64,
    pub(super) ternary_elided_positions: u64,
}

thread_local! {
    // Fixed counters per test thread. Each diagnostic resets before its finite
    // source query; no candidates, source values or results are cached here.
    static COUNTS: Cell<Counts> = const { Cell::new(Counts {
        watch_visits: 0,
        binary_attempts: 0,
        longer_attempts: 0,
        ternary_elided_positions: 0,
    }) };
}

pub(super) fn ternary_inspection(watches: [usize; 2], available: impl Fn(usize) -> bool) {
    // Independently replay the former ascending scan, including its watched
    // positions. The production specialization always performs one test.
    let mut inspections = 0;
    for position in 0..3 {
        inspections += 1;
        if !watches.contains(&position) && available(position) {
            break;
        }
    }
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        counts.ternary_elided_positions = counts
            .ternary_elided_positions
            .checked_add(inspections - 1)
            .expect("bounded profile count");
        cell.set(counts);
    });
}

pub(super) fn visit() {
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        counts.watch_visits = counts
            .watch_visits
            .checked_add(1)
            .expect("bounded profile count");
        cell.set(counts);
    });
}

pub(super) fn reset() {
    COUNTS.set(Counts::default());
}

pub(super) fn snapshot() -> Counts {
    COUNTS.get()
}

pub(super) fn replacement_attempt(length: usize) {
    COUNTS.with(|cell| {
        let mut counts = cell.get();
        let counter = if length == 2 {
            &mut counts.binary_attempts
        } else {
            &mut counts.longer_attempts
        };
        *counter = counter.checked_add(1).expect("bounded profile count");
        cell.set(counts);
    });
}

#[derive(Debug)]
enum End {
    Exhausted,
    CandidatePrefix,
    Incomplete(Incomplete),
}

impl std::fmt::Display for End {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted => output.write_str("exhausted"),
            Self::CandidatePrefix => output.write_str("candidate-prefix"),
            Self::Incomplete(reason) => write!(output, "incomplete({reason:?})"),
        }
    }
}

const CANDIDATES: usize = 128;
const CASES: [&str; 8] = [
    "standalone/send-money/send-money.lp",
    "standalone/n-queens/variant-01.lp",
    "standalone/n-queens/variant-02.lp",
    "standalone/n-queens/variant-03.lp",
    "standalone/n-queens/variant-04.lp",
    "standalone/n-queens/variant-05.lp",
    "standalone/n-queens/variant-06.lp",
    "scenarios/task-allocation/variant-04/05-larger-mix.lp",
];

#[test]
#[ignore = "bounded classical-prefix work profile; no elapsed time, objective feedback or reduct checks"]
fn profile_finite_candidate_prefixes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kr-domains");
    for case in CASES {
        let bundle = SourceBundle::load(root.join(case), BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let theory = admitted.theory();
        let control = Control::default();
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits::default(),
            control: &control,
            statistics: SearchStatistics::default(),
        };
        let mut cnf =
            encoding::encode(theory, None, AdmissionLimits::default(), &mut budget).unwrap();
        let clauses = cnf.clauses().len();
        let binary = cnf.clauses().filter(|clause| clause.len() == 2).count();
        reset();
        let mut cursor = Cursor::projected(theory.atom_count());
        let mut candidates = 0;
        let mut end = End::CandidatePrefix;
        for _ in 0..CANDIDATES {
            match cursor.query(&cnf, &mut budget) {
                Solve::Sat(assignment) => {
                    candidates += 1;
                    let candidate =
                        encoding::interpretation(theory, &assignment, &mut budget).unwrap();
                    if let Err(error) = cursor.exclude(&mut cnf, &candidate, &mut budget) {
                        end = End::Incomplete(error);
                        break;
                    }
                }
                Solve::Unsat => {
                    end = End::Exhausted;
                    break;
                }
                Solve::Inconclusive(error) => {
                    end = End::Incomplete(error);
                    break;
                }
            }
        }
        let counts = snapshot();
        println!(
            "PROPAGATION_PROFILE case={case} atoms={} variables={} clauses={clauses} binary_clauses={binary} candidates={candidates} end={end} statistics={:?} profile={counts:?}",
            theory.atom_count(),
            cnf.variables(),
            budget.statistics,
        );
    }
}
