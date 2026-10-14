//! Eager lowering preserves anchored fact occurrences and refusal boundaries.

use zetesis_core::{GroundProgram, StaticError, StaticFailure, StaticLimits};
use zetesis_test_support::programs::{atom, fact_tables, nullary};

#[test]
fn fact_tables_preserve_ordered_ground_rules() {
    let [explicit, table] = fact_tables::interleaved();
    assert_eq!(table.templates().len(), 2);
    assert_eq!(table.facts().len(), 4);
    let expected = GroundProgram::compile(&explicit, StaticLimits::default()).unwrap();
    let actual = GroundProgram::compile(&table, StaticLimits::default()).unwrap();
    assert!(actual.atoms().iter().eq(expected.atoms().iter()));
    assert_eq!(actual.rules(), expected.rules());
    assert_eq!(actual.gate_atom_ids(), expected.gate_atom_ids());
    let heads = [
        Some(atom("seed", vec![fact_tables::structured()])),
        Some(atom("joined", vec![fact_tables::structured()])),
        Some(nullary("ready")),
        Some(atom("seed", vec![fact_tables::structured()])),
        None,
        Some(nullary("tail")),
    ];
    assert_eq!(actual.rules().len(), heads.len());
    for (rule, head) in actual.rules().iter().zip(heads) {
        assert_eq!(
            rule.head(),
            head.as_ref().map(|atom| actual.atom_id(atom).unwrap())
        );
    }
}

#[test]
fn nullary_table_facts_each_count_one_substitution() {
    for program in fact_tables::nullary_duplicates() {
        assert!(program.domain().is_empty());
        let limits = StaticLimits {
            max_atoms: 1,
            max_substitutions: 3,
            max_ground_rules: 3,
        };
        let graph = GroundProgram::compile(&program, limits).unwrap();
        assert_eq!(graph.rules().len(), 3);
        let error = GroundProgram::compile(
            &program,
            StaticLimits {
                max_substitutions: 2,
                ..limits
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            StaticError::LimitExceeded {
                resource: "substitutions",
                actual: 3,
                limit: 2,
            }
        );
    }
}

#[test]
fn duplicate_table_facts_each_count_one_ground_rule() {
    for program in fact_tables::nullary_duplicates() {
        let error = GroundProgram::compile(
            &program,
            StaticLimits {
                max_ground_rules: 2,
                ..StaticLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            StaticError::LimitExceeded {
                resource: "ground rules",
                actual: 3,
                limit: 2,
            }
        );
    }
}

#[test]
fn table_facts_preserve_every_control_boundary() {
    let [explicit, table] = fact_tables::interleaved();
    let mut expected_calls = 0;
    let expected = GroundProgram::compile_with(&explicit, StaticLimits::default(), || {
        expected_calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    let mut actual_calls = 0;
    let actual = GroundProgram::compile_with(&table, StaticLimits::default(), || {
        actual_calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    assert_eq!(actual_calls, expected_calls);
    assert_eq!(actual.rules(), expected.rules());
    for cutoff in 0..actual_calls {
        let mut accepted = 0;
        let error = GroundProgram::compile_with(&table, StaticLimits::default(), || {
            if accepted == cutoff {
                return Err(cutoff);
            }
            accepted += 1;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error, StaticFailure::Stopped(cutoff));
        assert_eq!(accepted, cutoff);
    }
    let retried = GroundProgram::compile(&table, StaticLimits::default()).unwrap();
    assert_eq!(retried.rules(), expected.rules());
}
