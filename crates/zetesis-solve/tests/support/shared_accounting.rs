//! Shared-CPU evidence and completed-check delivery.

use std::num::NonZeroUsize;
use zetesis_core::{
    AdmissionLimits, AtomPattern, Model, Predicate, Program, Seed, Template, Term, Value,
};
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
    assert!(matches!(error, crate::SolveError::LazyStatisticsOverflow));
    assert_eq!(statistics, before);
}

fn payload(model: &Model) -> &str {
    let Value::String(value) = &model.atoms().first().unwrap().values()[0] else {
        panic!("fixture contains one string argument");
    };
    value
}

#[test]
fn batch_results_transfer_payload_storage() {
    let head = AtomPattern::new(
        Predicate::new("message", 1).unwrap(),
        vec![Term::Constant(Value::String("retained payload".into()))],
    )
    .unwrap();
    let program = Program::new(
        vec![Template::new(Some(head), vec![], vec![], vec![], vec![])],
        AdmissionLimits::default(),
    )
    .unwrap();
    let pool = BatchOracle::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
    let batch = pool
        .check_shared(
            &program,
            &[Seed::new(&program, []).unwrap()],
            shared::Limits::default(),
            SourceSelection::Union,
            &Control::default(),
        )
        .unwrap();
    assert!(batch.checks[0].accepted());
    let original = payload(batch.checks[0].closure()).as_ptr();
    let mut statistics = super::SharedExecutionStatistics::new(
        &crate::SolveConfig::default(),
        SourceSelection::Union,
    );
    let mut results = super::batch_results(Ok(batch), &mut statistics).unwrap();
    assert_eq!(results.len(), 1);
    let model = results.pop().unwrap().unwrap().unwrap();
    assert_eq!(payload(&model), "retained payload");
    // This boundary must transfer the original nonempty String allocation;
    // value equality alone would also accept the former closure clone.
    assert_eq!(payload(&model).as_ptr(), original);
}
