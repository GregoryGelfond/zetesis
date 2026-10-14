use super::*;
use std::cell::Cell;

use crate::formula::Compiled;
use crate::formula_ground::{ground, ground_hybrid};
use crate::formula_support::testing;
use crate::{GroundingObserver, GroundingOutcome, GroundingPhase, GroundingWork, ProgramSite};

const DIAGONAL: &str = include_str!("../../../tests/fixtures/expression-keys/diagonal.lp");
const ALIAS: &str = include_str!("../../../tests/fixtures/expression-keys/alias.lp");
const MIXED: &str = include_str!("../../../tests/fixtures/expression-keys/mixed.lp");
const ZERO: &str = include_str!("../../../tests/fixtures/expression-keys/zero.lp");
const OVERFLOW: &str = include_str!("../../../tests/fixtures/expression-keys/overflow.lp");
const ORDERING: &str = include_str!("../../../tests/fixtures/expression-keys/ordering.lp");
const GENERATED: &str = include_str!("../../../tests/fixtures/expression-keys/generated.lp");
const UNDEFINED: &str = include_str!("../../../tests/fixtures/expression-keys/undefined.lp");

thread_local! { static DISABLED: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn disabled() -> bool {
    DISABLED.get()
}
struct Restore(bool);
impl Drop for Restore {
    fn drop(&mut self) {
        DISABLED.set(self.0);
    }
}
fn controlled<T>(disabled: bool, run: impl FnOnce() -> T) -> T {
    let _restore = Restore(DISABLED.replace(disabled));
    run()
}

#[derive(Default)]
struct Receipts {
    emission: Cell<GroundingWork>,
    other: Cell<GroundingWork>,
}
impl GroundingObserver for Receipts {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        let destination = if phase == GroundingPhase::RuleInstantiation {
            &self.emission
        } else {
            &self.other
        };
        destination.set(destination.get().checked_sum(work));
    }
}

fn assert_same_program(left: &Compiled, right: &Compiled) {
    assert_eq!(
        crate::test_support::compiled_atoms(left),
        crate::test_support::compiled_atoms(right)
    );
    assert_eq!(left.warnings, right.warnings);
    assert_eq!(left.theory.nodes(), right.theory.nodes());
    assert_eq!(left.theory.operands(), right.theory.operands());
    assert_eq!(left.theory.roots(), right.theory.roots());
    assert_eq!(left.origins, right.origins);
}

#[test]
fn expression_postings_preserve_the_grounded_theory() {
    let reference_receipts = Receipts::default();
    let reference = controlled(true, || {
        ground(testing::prepare(DIAGONAL), Some(&reference_receipts), None)
    })
    .unwrap();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(DIAGONAL), Some(&receipts), None).unwrap();
    assert_same_program(&reference, &selected);
    let work = receipts.emission.get();
    assert_eq!(work.numeric_index_builds, Some(1));
    assert_eq!(work.numeric_index_declines, Some(0));
    assert_eq!(work.expression_key_probes, Some(9));
    assert_eq!(work.expression_key_rows, Some(19));
    assert!(work.join_rows.unwrap() < reference_receipts.emission.get().join_rows.unwrap());
    assert_eq!(receipts.other.get().expression_key_probes, Some(0));
    assert_eq!(receipts.other.get().numeric_index_builds, Some(0));
}

#[test]
fn failed_certificates_preserve_general_evaluation() {
    for source in [MIXED, ZERO, OVERFLOW] {
        let reference = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
        let receipts = Receipts::default();
        let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap();
        assert_same_program(&reference, &selected);
        assert_eq!(
            receipts.emission.get().numeric_index_declines,
            Some(1),
            "{source}"
        );
        assert_eq!(
            receipts.emission.get().expression_key_probes,
            Some(0),
            "{source}"
        );
    }
}

#[test]
fn relational_ordering_supplies_no_integer_certificate() {
    let reference = controlled(true, || ground(testing::prepare(ORDERING), None, None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(ORDERING), Some(&receipts), None).unwrap();
    assert_same_program(&reference, &selected);
    let atoms = crate::test_support::compiled_atoms(&selected);
    let expected = zetesis_core::Atom::new(
        zetesis_core::Predicate::new("r", 1).unwrap(),
        vec![zetesis_core::Value::Symbol("a".into())],
    )
    .unwrap();
    assert!(atoms.contains(&expected));
    assert_eq!(receipts.emission.get().numeric_index_builds, Some(0));
    assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
}

#[test]
fn generated_equalities_keep_their_original_route() {
    let reference = controlled(true, || ground(testing::prepare(GENERATED), None, None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(GENERATED), Some(&receipts), None).unwrap();
    assert_same_program(&reference, &selected);
    assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
}

#[test]
fn source_admission_keeps_undefined_families() {
    let reference =
        controlled(true, || ground(testing::prepare(UNDEFINED), None, None)).unwrap_err();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(UNDEFINED), Some(&receipts), None).unwrap_err();
    match (reference, selected) {
        (
            FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                error: before,
                location: before_site,
            }),
            FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                error: after,
                location: after_site,
            }),
        ) => {
            assert_eq!(before, after);
            assert_eq!(before_site, after_site);
        }
        other => panic!("expected matching undefined-family failures: {other:?}"),
    }
    assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
}

#[test]
fn hybrid_capture_keeps_its_original_rows() {
    let reference = controlled(true, || ground_hybrid(testing::prepare(DIAGONAL), None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground_hybrid(testing::prepare(DIAGONAL), Some(&receipts)).unwrap();
    assert!(reference.1.is_some());
    assert!(selected.1.is_some());
    assert_same_program(&reference.0, &selected.0);
    assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
    assert_eq!(receipts.emission.get().numeric_index_builds, Some(0));
}

#[test]
fn repeated_columns_remain_residual_equalities() {
    let reference = controlled(true, || ground(testing::prepare(ALIAS), None, None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(ALIAS), Some(&receipts), None).unwrap();
    assert_same_program(&reference, &selected);
    assert_eq!(receipts.emission.get().numeric_index_builds, Some(1));
    assert_eq!(receipts.emission.get().expression_key_probes, Some(1));
    assert_eq!(receipts.emission.get().expression_key_rows, Some(2));
}

#[test]
fn false_equalities_preserve_independent_fatal_exclusion() {
    for source in [
        include_str!("../../../tests/fixtures/expression-keys/excluded-fatal.lp"),
        include_str!("../../../tests/fixtures/expression-keys/negative-tuple.lp"),
    ] {
        let reference = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
        let receipts = Receipts::default();
        let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap();
        assert_same_program(&reference, &selected);
        assert!(
            receipts.emission.get().expression_key_probes.unwrap() > 0,
            "{source}"
        );
        assert!(
            receipts.emission.get().expression_key_rows.unwrap() > 0,
            "{source}"
        );
    }
}

#[test]
fn selected_rows_keep_independent_fatal_errors() {
    for source in [
        include_str!("../../../tests/fixtures/expression-keys/retained-fatal.lp"),
        include_str!("../../../tests/fixtures/expression-keys/negative-tuple-fatal.lp"),
    ] {
        let reference =
            controlled(true, || ground(testing::prepare(source), None, None)).unwrap_err();
        let receipts = Receipts::default();
        let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap_err();
        match (reference, selected) {
            (
                FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                    error: before,
                    location: before_site,
                }),
                FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                    error: after,
                    location: after_site,
                }),
            ) => {
                assert_eq!(before, themelios_program::term::EvalError::Overflow);
                assert_eq!(before, after);
                assert_eq!(before_site, after_site);
            }
            other => panic!("expected matching scalar failures: {other:?}"),
        }
        assert!(
            receipts.emission.get().expression_key_probes.unwrap() > 0,
            "{source}"
        );
        assert!(
            receipts.emission.get().expression_key_rows.unwrap() > 0,
            "{source}"
        );
    }
}

#[test]
fn arithmetic_heads_keep_the_original_traversal() {
    let source = include_str!("../../../tests/fixtures/expression-keys/head-arithmetic.lp");
    let reference = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
    let receipts = Receipts::default();
    let selected = ground(testing::prepare(source), Some(&receipts), None).unwrap();
    assert_same_program(&reference, &selected);
    assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
    let source = include_str!("../../../tests/fixtures/expression-keys/head-overflow.lp");
    for disabled in [true, false] {
        let receipts = Receipts::default();
        let result = controlled(disabled, || {
            ground(testing::prepare(source), Some(&receipts), None)
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Overflow,
                    ..
                }
            ))
        ));
        assert_eq!(receipts.emission.get().expression_key_probes, Some(0));
    }
}

mod ranges;
mod rounds;
