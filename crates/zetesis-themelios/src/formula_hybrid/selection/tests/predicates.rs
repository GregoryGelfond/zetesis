use super::*;
use crate::formula_binding::Binding;
use crate::formula_hybrid::{Candidate, body};
use crate::formula_support::Context;
use crate::formula_support::testing::budget;

fn prepared_owner() -> HybridFormula {
    owner_of(include_str!(
        "../../../../tests/fixtures/hybrid-constraints/prepared-predicates.lp"
    ))
}

#[test]
fn predicate_preparation_is_reused_without_new_charges() {
    let owner = prepared_owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    prepared.prepare_predicates(0, &mut counters).unwrap();
    let work = counters.accounting.work;
    let bytes = workspace_bytes(prepared);
    prepared.prepare_predicates(0, &mut counters).unwrap();
    assert_eq!(counters.accounting.work, work);
    assert_eq!(workspace_bytes(prepared), bytes);
    assert!(prepared.predicates[0].is_some());
}

#[test]
fn refused_predicate_buffer_preserves_the_retry_ledger() {
    let owner = prepared_owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    // Publish just the sparse outer slots; a failed occurrence view must not
    // enter either a slot or the retained support ledger.
    prepared.predicates = RulePredicates::slots(
        &prepared.source.rules,
        &mut prepared.completed,
        &prepared.limits,
        &mut counters,
    )
    .unwrap();
    let retained = workspace_bytes(prepared);
    let prior_limit = prepared.limits.max_support_bytes;
    prepared.limits.max_support_bytes = usize::try_from(retained).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            prepared.prepare_predicates(0, &mut counters),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert!(prepared.predicates[0].is_none());
        assert_eq!(workspace_bytes(prepared), retained);
    }
    prepared.limits.max_support_bytes = prior_limit;
    prepared.prepare_predicates(0, &mut counters).unwrap();
    assert!(workspace_bytes(prepared) > retained);
}

#[test]
fn refused_predicate_work_keeps_the_slot_empty() {
    let owner = prepared_owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    prepared.predicates = RulePredicates::slots(
        &prepared.source.rules,
        &mut prepared.completed,
        &prepared.limits,
        &mut counters,
    )
    .unwrap();
    let retained = workspace_bytes(prepared);
    let prior_limit = prepared.limits.max_work;
    prepared.limits.max_work = counters.accounting.work + 2;
    assert!(matches!(
        prepared.prepare_predicates(0, &mut counters),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert!(prepared.predicates[0].is_none());
    assert_eq!(workspace_bytes(prepared), retained);
    prepared.limits.max_work = prior_limit;
    prepared.prepare_predicates(0, &mut counters).unwrap();
}

#[test]
fn foreign_body_cannot_borrow_prepared_occurrences() {
    let owner = prepared_owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    prepared.prepare_predicates(0, &mut counters).unwrap();
    let rule = &prepared.source.rules[0];
    let predicates = prepared.predicates[0].as_ref().unwrap();
    // Re-admission of the identical fixture gives an authentic but separate
    // source body; occurrence authority cannot cross its owning source.
    let foreign = prepared_owner();
    let foreign_checker = foreign.checker(ConstraintCheckLimits::default()).unwrap();
    let foreign_body = &foreign_checker.prepared.as_ref().unwrap().source.rules[0].body;
    assert_eq!(foreign_body.len(), rule.body.len());
    assert!(!std::ptr::eq(foreign_body.as_slice(), rule.body.as_slice()));
    for occurrence in 0..rule.body.len() {
        assert!(predicates.at(&rule.body, occurrence).is_some());
        assert!(predicates.at(foreign_body, occurrence).is_none());
    }
    assert!(predicates.at(&rule.body, rule.body.len()).is_none());
}

fn region(count: usize, mut code: usize) -> Region {
    let mut region = Region::all_open(count);
    for atom in 0..count {
        match code % 3 {
            1 => {
                assert!(region.hold(atom));
            }
            2 => {
                assert!(region.cut(atom));
            }
            _ => {}
        }
        code /= 3;
    }
    region
}

/// Exactly one binding from the ordinary prepared join, before any region
/// filter. This keeps the comparison independent of the new predicate gate.
fn with_binding(
    check: impl FnOnce(
        &super::super::super::PreparedConstraints<'_>,
        &Binding<'_>,
        &crate::formula_support::Computation<'_, '_>,
        &HybridFormula,
    ),
) {
    let owner = prepared_owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    prepared.prepare_predicates(0, &mut counters).unwrap();
    prepared
        .prepare_rule(0, &mut budget(), &mut counters)
        .unwrap();
    let rule = &prepared.source.rules[0];
    let queries = prepared
        .completed
        .queries(
            crate::JoinStrategy::Indexed,
            &prepared.limits,
            &counters,
            rule.location,
        )
        .unwrap();
    let mut computation = queries.computation(rule.location).unwrap();
    let mut rows = prepared.plans[0]
        .as_ref()
        .unwrap()
        .rows(
            &queries,
            None,
            &computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
        )
        .unwrap();
    let row = rows
        .next_row(
            &mut computation,
            &prepared.limits,
            &mut budget(),
            &mut counters,
            rule.location,
        )
        .unwrap()
        .unwrap();
    assert!(row.passes);
    check(prepared, &row.values, &computation, &owner);
}

#[test]
fn prepared_body_preserves_all_region_results() {
    with_binding(|prepared, binding, computation, owner| {
        let rule = &prepared.source.rules[0];
        let index = prepared.index.unwrap().lookup();
        let predicates = prepared.predicates[0].as_ref().unwrap();
        let count = owner.atom_catalog().atoms().len();
        assert_eq!(count, 3);
        for code in 0..3usize.pow(u32::try_from(count).unwrap()) {
            let region = region(count, code);
            assert_eq!(
                body(
                    &rule.body,
                    binding,
                    None,
                    Candidate::Region(owner.core_theory(), &region),
                    Some(index),
                    Some(predicates),
                    Context::new(
                        computation,
                        &prepared.limits,
                        &mut Counters::default(),
                        rule.location
                    )
                )
                .unwrap(),
                body(
                    &rule.body,
                    binding,
                    None,
                    Candidate::Region(owner.core_theory(), &region),
                    Some(index),
                    None,
                    Context::new(
                        computation,
                        &prepared.limits,
                        &mut Counters::default(),
                        rule.location
                    )
                )
                .unwrap()
            );
        }
    });
}

#[test]
fn prepared_gate_preserves_all_region_results() {
    with_binding(|prepared, _, _, owner| {
        let rule = &prepared.source.rules[0];
        let index = prepared.index.unwrap().lookup();
        let count = owner.atom_catalog().atoms().len();
        assert_eq!(count, 3);
        for code in 0..3usize.pow(u32::try_from(count).unwrap()) {
            let region = region(count, code);
            let selection = prepared.selection(0, &region);
            let ordinary = Selection {
                rows: selection.rows,
                index,
                predicates: None,
                region: &region,
            };
            assert_eq!(
                selection
                    .possible(
                        rule,
                        &prepared.completed,
                        &prepared.limits,
                        &mut Counters::default()
                    )
                    .unwrap(),
                ordinary
                    .possible(
                        rule,
                        &prepared.completed,
                        &prepared.limits,
                        &mut Counters::default()
                    )
                    .unwrap()
            );
        }
    });
}

#[test]
fn prepared_consequences_preserve_every_small_cube() {
    use crate::{ConstraintConsequence, ConstraintRegionPass};
    let owner = prepared_owner();
    let atoms = owner.atom_catalog().atoms();
    assert_eq!(atoms.len(), 3);
    let position = |name: &str, sign: zetesis_core::Sign| {
        atoms
            .iter()
            .position(|atom| {
                let predicate = atom.predicate();
                predicate.name() == name && predicate.sign() == sign && predicate.arity() == 1
            })
            .unwrap()
    };
    let p = position("p", zetesis_core::Sign::Positive);
    let negative = position("p", zetesis_core::Sign::Negative);
    let q = position("q", zetesis_core::Sign::Positive);
    for code in 0..27 {
        let cube = region(3, code);
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let outcome = checker
            .consequence_region(
                owner.core_theory(),
                &cube,
                &zetesis_cpu::Cancellation::default(),
                ConstraintRegionPass::First,
            )
            .unwrap();
        let prepared = checker.prepared.as_ref().unwrap();
        assert!(prepared.predicates[0].is_some());
        let models: Vec<_> = (0usize..8)
            .filter(|mask| {
                (0..3).all(|atom| {
                    cube.decision(atom)
                        .is_none_or(|held| held == (mask & (1 << atom) != 0))
                }) && !(mask & (1 << p) != 0 && mask & (1 << negative) == 0 && mask & (1 << q) == 0)
            })
            .collect();
        match outcome {
            ConstraintConsequence::Refuted { .. } => assert!(models.is_empty()),
            ConstraintConsequence::Hold { atom, .. } => {
                assert!(cube.decision(atom).is_none());
                assert!(models.iter().all(|mask| mask & (1 << atom) != 0));
            }
            ConstraintConsequence::Cut { atom, .. } => {
                assert!(cube.decision(atom).is_none());
                assert!(models.iter().all(|mask| mask & (1 << atom) == 0));
            }
            ConstraintConsequence::NoConsequence => assert!(!models.is_empty()),
        }
    }
}

#[test]
fn prepared_body_refusals_remain_errors() {
    with_binding(|prepared, binding, computation, owner| {
        let rule = &prepared.source.rules[0];
        let index = prepared.index.unwrap().lookup();
        let predicates = prepared.predicates[0].as_ref().unwrap();
        let cube = region(3, 13); // All held: the second literal is false.
        let mut counters = Counters::default();
        let expected = body(
            &rule.body,
            binding,
            None,
            Candidate::Region(owner.core_theory(), &cube),
            Some(index),
            Some(predicates),
            Context::new(computation, &prepared.limits, &mut counters, rule.location),
        )
        .unwrap();
        assert!(!expected);
        for ceiling in 0..counters.accounting.work {
            let limits = FormulaLimits {
                max_work: ceiling,
                ..prepared.limits
            };
            let result = body(
                &rule.body,
                binding,
                None,
                Candidate::Region(owner.core_theory(), &cube),
                Some(index),
                Some(predicates),
                Context::new(
                    computation,
                    &limits,
                    &mut Counters::default(),
                    rule.location,
                ),
            );
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
        }
    });
}

#[test]
fn final_model_does_not_use_region_predicate_windows() {
    with_binding(|prepared, binding, computation, _| {
        let rule = &prepared.source.rules[0];
        let empty = zetesis_core::Model::new([]).unwrap();
        assert!(
            !body(
                &rule.body,
                binding,
                None,
                Candidate::Model(&empty),
                prepared.index.map(zetesis_core::CatalogIndex::lookup),
                prepared.predicates[0].as_ref(),
                Context::new(
                    computation,
                    &prepared.limits,
                    &mut Counters::default(),
                    rule.location
                )
            )
            .unwrap()
        );
    });
}
