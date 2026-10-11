use super::*;
use crate::formula_ground::{Purpose, ground};
use crate::formula_support::{testing, testing::Fixture};
use crate::test_support::location;
use crate::{ExpansionLimits, FormulaLimits};
use zetesis_core::Value;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AggregateErrorKind, Interpretation, Limits, Narrower, Narrowing, NarrowingScratch, NodeView,
    OriginalSubject, Region, RegionLimits, models, models_reduct,
};

fn with_family<T>(
    limits: &FormulaLimits,
    run: impl FnOnce(&mut Builder<'_, '_, '_>, &[ExtremumElement], &Binding) -> T,
) -> T {
    Fixture::default().with(location(), |_, computation, counters| {
        let values = testing::binding(
            &[
                Some(Value::Number(1)),
                Some(Value::Symbol("a".into())),
                Some(Value::Supremum),
            ],
            computation,
            counters,
            location(),
        );
        let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), 0);
        let mut builder = Builder::empty(
            computation,
            limits,
            &mut budget,
            counters,
            Purpose::Theory,
            None,
            location(),
        )
        .unwrap();
        builder.initialize(location()).unwrap();
        let condition = builder.node(NodeView::Atom(0), location()).unwrap();
        let elements = [0, 1].map(|slot| ExtremumElement {
            value: builder
                .term(&values.key(slot, location()).unwrap(), location())
                .unwrap(),
            condition,
        });
        run(&mut builder, &elements, &values)
    })
}

#[test]
fn zero_states_selects_scalar_before_preparation() {
    for states in [0, 1_024] {
        let mut limits = FormulaLimits::default();
        limits.aggregate.max_states = states;
        with_family(&limits, |builder, elements, values| {
            let before = builder.counters.accounting.work;
            assert_eq!(
                builder
                    .shares_extremum_family(elements.len(), values.len(), location())
                    .unwrap(),
                states != 0,
            );
            assert_eq!(builder.counters.accounting.work, before);
            let mut roots = CoordinateMap::new(
                builder.computation,
                builder.limits,
                &mut builder.counters,
                location(),
            )
            .unwrap();
            builder
                .extremum_assignment_roots(
                    elements,
                    AggregateExtremum::Min,
                    values,
                    &mut roots,
                    location(),
                )
                .unwrap();
        });
    }
}

#[test]
fn shared_preparation_cannot_reuse_spent_formula_work() {
    let mut compiler_refusals = 0;
    for remaining in 0..160 {
        with_family(&FormulaLimits::default(), |builder, elements, values| {
            builder.counters.accounting.work = builder.limits.max_work - remaining;
            let result = builder.append_extremum_assignment_family(
                elements,
                AggregateExtremum::Min,
                values,
                location(),
            );
            if let Err(FormulaFailure::Aggregate { error, .. }) = result {
                assert_eq!(error.kind(), AggregateErrorKind::WorkLimit);
                compiler_refusals += 1;
            }
            assert!(builder.counters.accounting.work <= builder.limits.max_work);
        });
    }
    assert!(
        compiler_refusals > 0,
        "exercise refusal inside the family compiler"
    );
}

#[test]
fn assignment_rank_quotients_preserve_arbitrary_frozen_worlds() {
    for source in [
        "{a;b}. n(N) :- N=#min{1,x:a;1,y:b;z,z:a;\"z\",s:b}.",
        "{a;b}. n(N) :- N=#max{1,x:a;1,y:b;z,z:a;\"z\",s:b}.",
    ] {
        let shared = ground(testing::prepare(source), None, None).unwrap();
        let mut scalar = testing::prepare(source);
        scalar.limits.aggregate.max_states = 0;
        let scalar = ground(scalar, None, None).unwrap();
        assert_eq!(shared.atoms.atoms(), scalar.atoms.atoms());
        // Check the execution representation as well as semantic agreement.
        assert!(!shared.theory.parts().extremum_cuts().is_empty());
        assert!(scalar.theory.parts().extremum_cuts().is_empty());
        let wide_disjunctions = |theory: &zetesis_ferraris::Theory| {
            (0..theory.nodes().len())
                .filter(|&index| matches!(theory.view().node(index).unwrap(), NodeView::Or(row) if row.len() > 2))
                .count()
        };
        assert_eq!(wide_disjunctions(&shared.theory), 0, "{source}");
        assert!(wide_disjunctions(&scalar.theory) > 0, "{source}");
        assert!(shared.theory.atom_count() <= 7);
        let count = 1usize << shared.theory.atom_count();
        let limits = Limits::default();
        let cancellation = Cancellation::default();
        for outer in 0..count {
            let world = |theory: &zetesis_ferraris::Theory, bits: usize| {
                Interpretation::new(
                    theory,
                    (0..theory.atom_count()).filter(|i| bits & (1 << i) != 0),
                )
                .unwrap()
            };
            let left = world(&shared.theory, outer);
            let right = world(&scalar.theory, outer);
            assert_eq!(
                models(&shared.theory, &left, limits, &cancellation).unwrap(),
                models(&scalar.theory, &right, limits, &cancellation).unwrap(),
            );
            for inner in 0..count {
                assert_eq!(
                    models_reduct(
                        &shared.theory,
                        &left,
                        &world(&shared.theory, inner),
                        limits,
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &scalar.theory,
                        &right,
                        &world(&scalar.theory, inner),
                        limits,
                        &cancellation
                    )
                    .unwrap(),
                    "{source} M={outer} J={inner}",
                );
            }
        }
    }
}

#[test]
fn shared_extrema_preserve_nonempty_unit_consequences() {
    for (kind, empty) in [("min", "sup"), ("max", "inf")] {
        let source = format!("{{a;b}}. :- b. :- N = #{kind} {{1:a;2:b;3:a}}, N = #{empty}.");
        let admitted = ground(testing::prepare(&source), None, None).unwrap();
        let theory = &admitted.theory;
        assert_eq!(theory.atom_count(), 2);
        // Native cuts must recover the unit; no flat Boolean witness may
        // restore the deduction on their behalf.
        assert!(!theory.parts().extremum_cuts().is_empty());
        assert!((0..theory.nodes().len()).all(|index| {
            !matches!(theory.view().node(index).unwrap(), NodeView::Or(row) if row.len() > 2)
        }));
        let narrower = Narrower::new(theory);
        let mut region = Region::all_open(theory.atom_count());
        let (outcome, _) = narrower
            .narrow_known(
                OriginalSubject::new(theory, None),
                &mut region,
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_ne!(outcome, Narrowing::Refuted);
        for (index, atom) in admitted.atoms.atoms().iter().enumerate() {
            let expected = match atom.predicate().name() {
                "a" => true,
                "b" => false,
                other => panic!("unexpected atom {other}"),
            };
            assert_eq!(region.decision(index), Some(expected), "{source}");
        }
    }
}

#[test]
fn scalar_source_extrema_use_native_cuts() {
    for kind in ["min", "max"] {
        for comparison in ["=", "!=", "<", "<=", ">", ">="] {
            let source = format!("{{a;b}}. q :- #{kind} {{1,x:a;z,y:b}} {comparison} 2.");
            let mut prepared = testing::prepare(&source);
            // A guard is not an assignment-value enumeration. Its native
            // representation must not acquire that unrelated population limit.
            prepared.limits.max_assignment_values = 0;
            let admitted = ground(prepared, None, None).unwrap();
            assert!(
                !admitted.theory.parts().extremum_cuts().is_empty(),
                "{source} must exercise native cuts"
            );
            let mut scalar = testing::prepare(&source);
            scalar.limits.aggregate.max_states = 0;
            let scalar = ground(scalar, None, None).unwrap();
            assert!(scalar.theory.parts().extremum_cuts().is_empty());
            assert_eq!(admitted.atoms.atoms(), scalar.atoms.atoms());
            let worlds = 1usize << admitted.theory.atom_count();
            let world = |theory: &zetesis_ferraris::Theory, bits: usize| {
                Interpretation::new(
                    theory,
                    (0..theory.atom_count()).filter(|i| bits & (1 << i) != 0),
                )
                .unwrap()
            };
            for outer in 0..worlds {
                let a = world(&admitted.theory, outer);
                let b = world(&scalar.theory, outer);
                for inner in 0..worlds {
                    assert_eq!(
                        models_reduct(
                            &admitted.theory,
                            &a,
                            &world(&admitted.theory, inner),
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap(),
                        models_reduct(
                            &scalar.theory,
                            &b,
                            &world(&scalar.theory, inner),
                            Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap(),
                        "{source} M={outer} J={inner}"
                    );
                }
            }
        }
    }
}

fn assert_source_ranks(source: &str, expected: &[u64], rank_count: u64) {
    let admitted = ground(testing::prepare(source), None, None).unwrap();
    let mut rows = 0;
    for index in 0..admitted.theory.nodes().len() {
        if let NodeView::ExtremumCut(cut) = admitted.theory.view().node(index).unwrap() {
            rows += 1;
            assert_eq!(cut.row.ranks, expected, "{source}");
            assert_eq!(cut.row.rank_count, rank_count);
            assert_eq!(cut.row.conditions.len(), expected.len());
        }
    }
    assert!(rows > 0, "the fixture must retain ordinary native cuts");
}

#[test]
fn coalesced_mixed_extrema_keep_storage_order_and_asp_ranks() {
    // Contribution tuple order uses typed storage identity: String precedes
    // Symbol. ASP extrema order reverses those classes, so the checked reuse
    // probe must reject this order. Distinct tuples still retain equal ranks.
    for kind in ["min", "max"] {
        let source = format!(
            "{{a;b}}. q :- #{kind} {{\"a\",x:a;z,z:b;1,b:a;#sup,s:a;1,a:b;#inf,i:b}} != z."
        );
        assert_source_ranks(&source, &[0, 1, 1, 3, 2, 4], 5);
    }
}

#[test]
fn coalesced_numeric_extrema_arrive_in_asp_order() {
    // Numeric storage and ASP order agree. The source's completed tuple sort
    // supplies the order which native preparation can check and reuse.
    for kind in ["min", "max"] {
        let source = format!("{{a;b}}. q :- #{kind} {{3,x:a;1,y:a;2,z:b;1,b:b}} != 2.");
        assert_source_ranks(&source, &[1, 1, 2, 3], 5);
    }
}
