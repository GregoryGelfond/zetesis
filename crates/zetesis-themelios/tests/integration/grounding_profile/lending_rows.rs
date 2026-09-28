//! Complete source families and actual ownership observations for eager rows.

use std::collections::BTreeSet;

use zetesis_core::Atom;
use zetesis_cpu::Cancellation;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{AdmittedFormula, FormulaLimits, GroundingPhase};

use super::{Observer, compile};

fn family(admitted: &AdmittedFormula) -> BTreeSet<BTreeSet<Atom>> {
    let mut search = StableModels::new(
        admitted.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let models = search
        .by_ref()
        .map(|result| {
            result
                .unwrap()
                .atoms()
                .map(|index| {
                    admitted
                        .atoms()
                        .at(index)
                        .unwrap()
                        .to_atom(zetesis_core::ValueLimits::default())
                        .unwrap()
                })
                .collect()
        })
        .collect();
    assert!(search.exhausted());
    models
}

#[test]
fn relational_instantiation_borrows_its_complete_rows() {
    let observer = Observer::default();
    let source = "v(1..4).e(X,Y):-v(X),v(Y),X<Y.reach(X,Y):-e(X,Y).reach(X,Z):-reach(X,Y),e(Y,Z).";
    compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
    let records = observer.records.borrow();
    let rules: Vec<_> = records
        .iter()
        .filter(|record| record.phase == GroundingPhase::RuleInstantiation)
        .collect();
    // Four facts, two sets of six pairs and four increasing triples.
    assert_eq!(
        rules
            .iter()
            .map(|record| record.work.roots.unwrap())
            .sum::<u64>(),
        20
    );
    assert!(
        rules
            .iter()
            .all(|record| record.work.binding_snapshots == Some(0))
    );
}

#[test]
fn support_generation_borrows_relational_bindings() {
    let observer = Observer::default();
    compile(
        "v(1..4).e(X,Y):-v(X),v(Y),X<Y.reach(X,Y):-e(X,Y).reach(X,Z):-reach(X,Y),e(Y,Z).",
        &FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let records = observer.records.borrow();
    let support = records
        .iter()
        .find(|record| record.phase == GroundingPhase::SupportCompletion)
        .unwrap();
    assert!(support.work.join_rows.is_some_and(|rows| rows > 0));
    assert_eq!(support.work.binding_snapshots, Some(0));
}

#[test]
fn nested_support_joins_borrow_local_bindings() {
    for source in ["d(1;2).{p(X):d(X)}.", "d(1;2).p(X):d(X)."] {
        let observer = Observer::default();
        let admitted = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
        assert_eq!(admitted.atoms().len(), 4);
        let records = observer.records.borrow();
        let support = records
            .iter()
            .find(|record| record.phase == GroundingPhase::SupportCompletion)
            .unwrap();
        assert!(support.work.join_rows.is_some_and(|rows| rows > 0));
        assert_eq!(support.work.binding_snapshots, Some(0));
    }
}

#[test]
fn local_support_lending_preserves_outer_correlation() {
    let facts = "d(1;2).e(1,a).e(1,b).e(2,c).";
    for (source, ground) in [
        ("{p(K,X):e(K,X)}:-d(K).", "{p(1,a);p(1,b)}.{p(2,c)}."),
        ("p(K,X):e(K,X):-d(K).", "p(1,a);p(1,b).p(2,c)."),
    ] {
        let admitted =
            compile(&format!("{facts}{source}"), &FormulaLimits::default(), None).unwrap();
        let expected =
            compile(&format!("{facts}{ground}"), &FormulaLimits::default(), None).unwrap();
        assert_eq!(family(&admitted), family(&expected));
    }
}

#[test]
fn lending_preserves_signed_structural_join_families() {
    let facts = "e(f(1),f(2)).e(f(1),f(3)).e(f(2),f(3)).-q(f(3)).";
    let source = format!("{facts}{{mark}}.r(X,Z):-e(X,Y),e(Y,Z),-q(Z).selected(X):-r(X,Z),mark.");
    let admitted = compile(&source, &FormulaLimits::default(), None).unwrap();
    let expected = [
        format!("{facts}r(f(1),f(3))."),
        format!("{facts}r(f(1),f(3)).mark.selected(f(1))."),
    ]
    .into_iter()
    .flat_map(|source| family(&compile(&source, &FormulaLimits::default(), None).unwrap()))
    .collect();
    assert_eq!(family(&admitted), expected);
}

#[test]
fn generated_rule_rows_keep_their_owned_continuation() {
    let observer = Observer::default();
    let admitted = compile(
        "d(1;2).q(X,Y):-d(X),Y=1..X.",
        &FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    let expected = compile(
        "d(1;2).q(1,1).q(2,1).q(2,2).",
        &FormulaLimits::default(),
        None,
    )
    .unwrap();
    assert_eq!(family(&admitted), family(&expected));
    assert!(
        observer
            .records
            .borrow()
            .iter()
            .filter(|record| record.phase == GroundingPhase::RuleInstantiation)
            .any(|record| record.work.binding_snapshots.is_some_and(|count| count > 0))
    );
}

#[test]
fn nested_arithmetic_keeps_each_outer_binding() {
    let source = "d(1;2).e(1,1).e(2,1).e(2,2).q(1).q(2).p(X):-d(X),q(Y+0):e(X,Y).";
    let admitted = compile(source, &FormulaLimits::default(), None).unwrap();
    let expected = compile(
        "d(1;2).e(1,1).e(2,1).e(2,2).q(1).q(2).p(1).p(2).",
        &FormulaLimits::default(),
        None,
    )
    .unwrap();
    assert_eq!(family(&admitted), family(&expected));
}

#[test]
fn generated_support_rows_keep_their_owned_continuation() {
    let observer = Observer::default();
    compile(
        "d(1;2).q(X,Y):-d(X),Y=1..X.",
        &FormulaLimits::default(),
        Some(&observer),
    )
    .unwrap();
    assert!(
        observer
            .records
            .borrow()
            .iter()
            .filter(|record| record.phase == GroundingPhase::SupportCompletion)
            .any(|record| record.work.binding_snapshots.is_some_and(|count| count > 0))
    );
}

#[test]
fn selected_support_keeps_defined_false_witnesses() {
    // A selected-row consumer yields no head, but the defined false instance
    // still witnesses arithmetic admission for this source family.
    let admitted = compile("d(0;1).p(X):-d(X),1/X<0.", &FormulaLimits::default(), None).unwrap();
    let expected = compile("d(0;1).", &FormulaLimits::default(), None).unwrap();
    assert_eq!(family(&admitted), family(&expected));
}
