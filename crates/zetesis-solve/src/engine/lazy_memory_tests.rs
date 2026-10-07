//! Named lazy storage follows the route's host allowance.

use super::{Engine, lazy_source_limits};
use crate::execution_observation::Ignore;
use crate::phase_timing::Recorder;
use crate::{Grounder, Resources, SolveConfig, SourceBatching};
use std::num::NonZeroUsize;
use zetesis_core::{
    AdmissionLimits, AtomPattern, Model, Predicate, Program, SeedSelection, Template,
};
use zetesis_cpu::{Cancellation, Stop, source};

#[test]
fn lazy_memory_capacities_follow_the_host_share() {
    for memory in [0, 1, 1024, Resources::REFERENCE_MEMORY, u64::MAX] {
        let options = Resources::new(memory, NonZeroUsize::MIN).solve_config();
        for host_bytes in [options.max_batch_bytes, options.max_batch_bytes / 2] {
            let limits = lazy_source_limits(&options, host_bytes, options.max_source_work);
            let host = usize::try_from(host_bytes).unwrap();
            assert_eq!(limits.max_host_bytes, host);
            assert_eq!(limits.max_scan_bytes, host);
            assert_eq!(limits.max_instance_bytes, host / 32);
        }
    }
}

#[test]
fn explicit_host_allowances_keep_checked_extents() {
    let limits = lazy_source_limits(&SolveConfig::default(), u64::MAX, u64::MAX);
    assert_eq!(limits.max_host_bytes, isize::MAX as usize);
    assert_eq!(limits.max_scan_bytes, isize::MAX as usize);
    assert_eq!(limits.max_instance_bytes, isize::MAX as usize / 32);
}

#[test]
fn source_storage_policy_preserves_dispatch_bounds() {
    let options = SolveConfig::default();
    let limits = lazy_source_limits(&options, options.max_batch_bytes, 17);
    let dispatch = zetesis_cpu::lazy::Limits::default();
    assert_eq!(limits.max_source_work, 17);
    assert_eq!(limits.max_chunk_rules, dispatch.max_chunk_rules);
    assert_eq!(limits.max_chunk_words, dispatch.max_chunk_words);
    assert_eq!(limits.max_candidates, options.batch_size.get());
}

fn repeated_gate() -> Program {
    let gate = AtomPattern::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    Program::new(
        vec![Template::new(
            None,
            vec![],
            vec![gate.clone(), gate],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn instance_share_admits_its_exact_key_envelope() {
    let program = repeated_gate();
    let snapshot = Model::new([]).unwrap();
    // The source instance header plus two repeated nullary keys, each including
    // the one-byte predicate name. Payload is borrowed, not copied.
    let needed =
        size_of::<source::Instance<'_>>() + 2 * (size_of::<zetesis_core::AtomKey<'_>>() + 1);
    for bytes in [needed, needed - 1] {
        let host_bytes = u64::try_from(bytes * 32).unwrap();
        let limits = lazy_source_limits(&SolveConfig::default(), host_bytes, u64::MAX);
        let mut offered = 0;
        let result = source::scan(
            &program,
            &snapshot,
            source::ScanLimits {
                max_work: limits.max_source_work,
                max_instance_atoms: limits.max_chunk_words,
                max_instance_bytes: limits.max_instance_bytes,
                max_scan_bytes: limits.max_scan_bytes,
            },
            &Cancellation::default(),
            |instance| {
                assert_eq!(instance.gate_true().len(), 2);
                offered += 1;
                Ok::<(), Stop>(())
            },
        );
        if bytes == needed {
            assert_eq!(result.unwrap().bindings, 1);
            assert_eq!(offered, 1);
        } else {
            let failure = result.unwrap_err();
            assert!(matches!(
                failure.cause,
                source::ScanCause::Source(Stop::Allocation)
            ));
            assert_eq!(failure.statistics.bindings, 0);
            assert_eq!(offered, 0);
        }
    }
}

#[test]
fn small_memory_refuses_the_shared_host_envelope() {
    let program = repeated_gate();
    let config = SolveConfig {
        grounder: Grounder::Lazy,
        source_batching: SourceBatching::Union,
        ..Resources::new(1024 * 1024, NonZeroUsize::MIN).solve_config()
    };
    let phases = Recorder::new(false);
    let mut engine = Engine::new(&config, &program, &mut Ignore, &phases).unwrap();
    let seed = SeedSelection::new(&program, []).unwrap();
    let results = engine
        .check(
            &config,
            &program,
            &[seed],
            None,
            &Cancellation::default(),
            &phases,
        )
        .unwrap();
    assert_eq!(results, vec![Err(Stop::Allocation)]);
    let statistics = engine.shared_statistics().expect("shared CPU route");
    assert_eq!(statistics.submitted_candidates, 1);
    assert_eq!(statistics.completed_candidates, 0);
    assert_eq!(statistics.source_instances, 0);
}
