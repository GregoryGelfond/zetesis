//! Atomic accumulation of concrete shared-CPU evidence.

use std::num::NonZeroUsize;
use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Seed, Template};
use zetesis_cpu::lazy::{SourceSelection, shared};
use zetesis_cpu::{BatchOracle, Control};

#[test]
fn accumulation_overflow_preserves_prior_evidence() {
    let head = AtomPattern::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    let program = Program::new(
        vec![Template::new(Some(head), vec![], vec![], vec![], vec![])],
        AdmissionLimits::default(),
    )
    .unwrap();
    let seed = Seed::new(&program, []).unwrap();
    let pool = BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let batch = pool
        .check_shared(
            &program,
            &[seed],
            shared::Limits::default(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap();
    assert!(batch.statistics.source.source_work > 0);
    let mut statistics = super::SharedExecutionStatistics::new(
        &crate::SolveConfig::default(),
        SourceSelection::Union,
    );
    statistics.record(&batch.statistics, None).unwrap();
    statistics.source_work = u64::MAX;
    let before = statistics.clone();
    let error = statistics.record(&batch.statistics, None).unwrap_err();
    assert!(matches!(error, crate::RunError::LazyStatisticsOverflow));
    assert_eq!(statistics, before);
}
