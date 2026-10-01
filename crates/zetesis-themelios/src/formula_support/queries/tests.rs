//! Distinguishing workspace ownership, live masks and refused work prefixes.

mod finite_domains;

use zetesis_core::catalog::TermRef;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};

use super::*;
use crate::GroundingPhase;
use crate::grounding_observer::Profile;
use crate::test_support::Observer;
use crate::test_support::location;

fn catalog() -> super::super::SupportCatalog {
    let mut catalog = super::super::SupportCatalog::default();
    for value in 0..70 {
        let atom = Atom::new(
            Predicate::new("r", 2).unwrap(),
            vec![Value::Number(value % 7), Value::Number(value)],
        )
        .unwrap();
        catalog = catalog
            .insert(
                &atom,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
    }
    catalog
}

fn pattern(alias: bool) -> AtomPattern {
    AtomPattern::new(
        Predicate::new("r", 2).unwrap(),
        vec![Term::Variable(0), Term::Variable(usize::from(!alias))],
    )
    .unwrap()
}

#[test]
fn a_selection_survives_new_cache_entries() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let flat = pattern(false);
    let alias = pattern(true);
    let first = support
        .select(
            PositivePattern::Flat((&flat).into()),
            [Some(Value::Number(1)), None].as_slice().into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    let second = support
        .select(
            PositivePattern::Flat((&alias).into()),
            [None::<TermRef<'_>>].as_slice().into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(support.tables.get().unwrap().indices.borrow().len(), 2);
    assert_eq!(
        first.selection.rows().collect::<Vec<_>>(),
        (1..70).step_by(7).collect::<Vec<_>>()
    );
    assert_eq!(
        second.selection.rows().collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    drop(second);
    let restored = support
        .select(
            PositivePattern::Flat((&flat).into()),
            [None::<TermRef<'_>>, None].as_slice().into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.selection.rows().collect::<Vec<_>>(),
        (0..70).collect::<Vec<_>>()
    );
    assert_eq!(
        first.selection.rows().collect::<Vec<_>>(),
        (1..70).step_by(7).collect::<Vec<_>>()
    );
}

#[test]
fn every_simultaneous_mask_consumes_live_capacity() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    drop(
        support
            .select(
                PositivePattern::Flat((&pattern).into()),
                [None::<TermRef<'_>>, None].as_slice().into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut counters = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    let baseline = support.live.get();
    let first = profile
        .phase(GroundingPhase::RuleInstantiation, None, || {
            support.select(
                PositivePattern::Flat((&pattern).into()),
                [None::<TermRef<'_>>, None].as_slice().into(),
                &limits,
                &mut counters,
                location(),
            )
        })
        .unwrap()
        .unwrap();
    assert_eq!(support.live.get(), baseline + first.bytes);
    let peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    let bounded = FormulaLimits {
        max_support_bytes: peak,
        ..limits
    };
    let failed = support.select(
        PositivePattern::Flat((&pattern).into()),
        [None::<TermRef<'_>>, None].as_slice().into(),
        &bounded,
        &mut counters,
        location(),
    );
    assert!(
        matches!(failed, Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. }) if observed > limit && limit == peak as u128)
    );
    assert_eq!(support.live.get(), baseline + first.bytes);
    assert_eq!(first.selection.rows().count(), 70);
    drop(first);
    assert_eq!(support.live.get(), baseline);
    assert!(
        support
            .select(
                PositivePattern::Flat((&pattern).into()),
                [None::<TermRef<'_>>, None].as_slice().into(),
                &bounded,
                &mut counters,
                location()
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn failed_selection_retains_its_charged_work_prefix() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    let values = [Some(Value::Number(1)), None];
    drop(
        support
            .select(
                PositivePattern::Flat((&pattern).into()),
                values.as_slice().into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let before = counters.accounting.work;
    drop(
        support
            .select(
                PositivePattern::Flat((&pattern).into()),
                values.as_slice().into(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap(),
    );
    let complete = counters.accounting.work - before;
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut checked = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    checked.accounting.work = counters.accounting.work;
    let bounded = FormulaLimits {
        max_work: checked.accounting.work + complete - 1,
        ..limits
    };
    let live = support.live.get();
    let failed = profile.phase(GroundingPhase::RuleInstantiation, None, || {
        support.select(
            PositivePattern::Flat((&pattern).into()),
            values.as_slice().into(),
            &bounded,
            &mut checked,
            location(),
        )
    });
    assert!(
        matches!(failed, Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. }) if observed > limit)
    );
    assert!(observer.0.get().table_query_work.unwrap() > 0);
    assert!(checked.accounting.work > counters.accounting.work);
    assert!(checked.accounting.work <= bounded.max_work);
    assert_eq!(support.live.get(), live);
}

#[test]
fn refused_capacity_is_not_a_retained_peak() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let observer = Observer::default();
    let profile = Profile::new(Some(&observer));
    let mut checked = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    let bounded = FormulaLimits {
        max_support_bytes: support.live.get(),
        ..limits
    };
    let pattern = pattern(false);
    let failed = profile.phase(GroundingPhase::RuleInstantiation, None, || {
        support.select(
            PositivePattern::Flat((&pattern).into()),
            [None::<TermRef<'_>>, None].as_slice().into(),
            &bounded,
            &mut checked,
            location(),
        )
    });
    assert!(matches!(
        failed,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(
        observer.0.get().support_peak_bytes,
        Some(u64::try_from(support.live.get()).unwrap())
    );
}

#[test]
fn workspace_size_overflow_is_a_typed_failure() {
    let mut relations = Relations::default();
    relations.bytes = usize::MAX;
    let limits = FormulaLimits {
        max_support_bytes: usize::MAX,
        ..FormulaLimits::default()
    };
    assert!(matches!(
        Support::completed(
            &relations,
            JoinStrategy::Table,
            &limits,
            &Counters::default(),
            location()
        ),
        Err(FormulaFailure::SupportTable {
            error: table::Failure {
                cause: Cause::Overflow,
                ..
            },
            ..
        })
    ));
}

#[test]
fn canonical_frames_drive_both_query_strategies() {
    use std::convert::Infallible;
    use zetesis_core::atom_interner::{AtomInterner, Limits as InternLimits};

    let success = || Ok::<(), Infallible>(());
    let mut owner = AtomInterner::new();
    let atom = Atom::new(
        Predicate::new("binding", 1).unwrap(),
        vec![Value::Number(1)],
    )
    .unwrap();
    let intern_limits = InternLimits {
        max_atoms: 1,
        max_bytes: 1024 * 1024,
    };
    let position = owner
        .entry_atom_with(&atom, intern_limits, success)
        .unwrap()
        .insert_with(intern_limits, success)
        .unwrap();
    let read = owner.read();
    let key = read
        .term_key(owner.get(position).unwrap().values().at(0).unwrap())
        .unwrap();
    let mut assignment = read.assignment();
    assignment.resize_with(2, usize::MAX, success).unwrap();
    assignment.set_with(0, &key, success).unwrap();

    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    let binding = assignment.as_slice().bind_with(read, success).unwrap();
    let expected = (1..70).step_by(7).collect::<Vec<_>>();
    assert_eq!(
        support
            .probe(
                (&pattern).into(),
                binding,
                &limits,
                &mut counters,
                location()
            )
            .unwrap(),
        Some(expected.as_slice())
    );
    let selection = support
        .select(
            PositivePattern::Flat((&pattern).into()),
            binding,
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    // Only the query's selection metadata survives the call. Reusing its input
    // ID frame cannot change the rows already selected from this snapshot.
    assignment.clear_with(0, success).unwrap();
    assert_eq!(selection.selection.rows().collect::<Vec<_>>(), expected);
}

#[test]
fn out_of_scope_slots_are_not_unrestricted() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let pattern = pattern(false);
    let values: [Option<TermRef<'_>>; 1] = [None];
    let binding = values.as_slice().into();
    assert!(matches!(
        support.probe(
            (&pattern).into(),
            binding,
            &limits,
            &mut counters,
            location()
        ),
        Err(FormulaFailure::UnsafeVariable { variable: 1, .. })
    ));
    assert!(matches!(
        support.select(
            PositivePattern::Flat((&pattern).into()),
            binding,
            &limits,
            &mut counters,
            location()
        ),
        Err(FormulaFailure::UnsafeVariable { variable: 1, .. })
    ));
}

#[test]
fn absent_table_relations_still_admit_lookup_work() {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let missing = AtomPattern::new(Predicate::new("missing", 0).unwrap(), Vec::new()).unwrap();
    let bounded = FormulaLimits {
        max_work: 0,
        ..limits
    };
    let mut attempt = Counters::default();
    let values: [Option<Value>; 0] = [];
    assert!(matches!(
        support.select(
            PositivePattern::Flat((&missing).into()),
            values.as_slice().into(),
            &bounded,
            &mut attempt,
            location()
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            limit: 0,
            ..
        })
    ));
    assert_eq!(attempt.accounting.work, 0);
}

/// These patterns have their own canonical vocabulary, distinct from the
/// relation authority. Equal ID numbers therefore cannot decide a match.
fn canonical_pattern(pattern: &AtomPattern) -> zetesis_core::TemplateCatalog {
    let mut owner = zetesis_core::TemplateCatalogBuilder::new(usize::MAX).unwrap();
    owner.append([], [pattern.into()], []).unwrap();
    owner.finish().unwrap()
}

#[test]
fn canonical_table_patterns_keep_repeated_variables() {
    let catalog = catalog();
    let owner = canonical_pattern(&pattern(true));
    let pattern = owner.at(0).unwrap().patterns().at(0).unwrap();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Table,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let selected = support
        .select(
            PositivePattern::Flat(pattern),
            [None::<TermRef<'_>>].as_slice().into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    let mut next = 0;
    let mut rows = Vec::new();
    while let Some(row) = selected
        .next(next, &limits, &mut counters, location())
        .unwrap()
    {
        rows.push(row);
        next = row + 1;
    }
    // r(N % 7, N) has equal arguments exactly for N in 0..7.
    assert_eq!(rows, (0..7).collect::<Vec<_>>());
}

#[test]
fn canonical_probes_preserve_typed_constants() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let predicate = Predicate::new("typed", 1).unwrap();
    let mut catalog = super::super::SupportCatalog::default();
    for value in [Value::String("same".into()), Value::Symbol("same".into())] {
        catalog = catalog
            .insert(
                &Atom::new(predicate.clone(), vec![value]).unwrap(),
                &limits,
                &mut counters,
                location(),
            )
            .unwrap();
    }
    let owner = canonical_pattern(
        &AtomPattern::new(
            predicate,
            vec![Term::Constant(Value::Symbol("same".into()))],
        )
        .unwrap(),
    );
    let pattern = owner.at(0).unwrap().patterns().at(0).unwrap();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let rows = support
        .probe(
            pattern,
            [None::<TermRef<'_>>; 0].as_slice().into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(rows.len(), 1);
    let selected = support.row(pattern.predicate(), rows[0]).unwrap();
    assert_eq!(
        selected.value(0).unwrap().descriptor(),
        zetesis_core::ValueNodeRef::Symbol("same")
    );
}
