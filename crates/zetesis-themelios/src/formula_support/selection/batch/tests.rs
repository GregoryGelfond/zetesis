use super::*;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use zetesis_core::{Atom, Predicate, Value};

fn with_batch<T>(
    run: impl FnOnce(SourceBatch, &[SourceAtom], &mut Computation<'_, '_>, &mut Counters) -> T,
) -> T {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut sources = Vec::new();
        for number in 0..4 {
            let atom =
                Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(number)]).unwrap();
            sources.push(
                computation
                    .atom_ref((&atom).into(), &limits, counters, location())
                    .unwrap(),
            );
        }
        let mut batch = SourceSelection::batch(computation, &limits, counters, location()).unwrap();
        for index in [3, 2, 1, 0, 3, 1, 0] {
            batch
                .push(&sources[index], computation, &limits, counters, location())
                .unwrap();
        }
        run(batch, &sources, computation, counters)
    })
}

#[test]
fn reverse_and_duplicate_ids_publish_one_coordinate_each() {
    with_batch(|batch, sources, computation, counters| {
        let limits = FormulaLimits::default();
        let selected = batch
            .finish(
                (FormulaResource::Atoms, 4),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
        assert_eq!(selected.len(), 4);
        for (position, source) in sources.iter().enumerate() {
            assert_eq!(
                selected
                    .position(source, &limits, counters, location())
                    .unwrap(),
                Some(position)
            );
            assert_eq!(selected.positions()[position], source.position);
        }
    });
}

#[test]
fn batch_atom_limit_counts_distinct_identities() {
    with_batch(|batch, _, computation, counters| {
        let limits = FormulaLimits::default();
        assert!(matches!(
            batch.finish(
                (FormulaResource::Atoms, 3),
                computation,
                &limits,
                counters,
                location(),
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Atoms,
                observed: 4,
                limit: 3,
                ..
            })
        ));
    });
}

#[test]
fn every_stopped_finish_prefix_publishes_no_selection() {
    let complete_work = with_batch(|batch, _, computation, counters| {
        let before = counters.accounting.work;
        batch
            .finish(
                (FormulaResource::Atoms, 4),
                computation,
                &FormulaLimits::default(),
                counters,
                location(),
            )
            .unwrap();
        counters.accounting.work - before
    });
    assert!(
        complete_work > 7,
        "sorting and inverse publication are both exercised"
    );
    for allowed in 0..complete_work {
        with_batch(|batch, sources, computation, counters| {
            let before = counters.accounting.work;
            let limits = FormulaLimits {
                max_work: before + allowed,
                ..FormulaLimits::default()
            };
            let result = batch.finish(
                (FormulaResource::Atoms, 4),
                computation,
                &limits,
                counters,
                location(),
            );
            assert!(
                matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ),
                "allowed={allowed}"
            );
            assert_eq!(counters.accounting.work - before, allowed);
            // A stopped private permutation cannot damage the canonical owner.
            for source in sources {
                computation
                    .source_atom(source, &FormulaLimits::default(), counters, location())
                    .unwrap();
            }
        });
    }
}

#[test]
fn batch_rejects_foreign_source_identity() {
    with_batch(|mut batch, _, _, _| {
        Fixture::default().with(location(), |_, computation, counters| {
            let limits = FormulaLimits::default();
            let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(0)]).unwrap();
            let foreign = computation
                .atom_ref((&atom).into(), &limits, counters, location())
                .unwrap();
            assert!(matches!(
                batch.push(&foreign, computation, &limits, counters, location()),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
            assert!(matches!(
                batch.finish(
                    (FormulaResource::Atoms, 4),
                    computation,
                    &limits,
                    counters,
                    location()
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
        });
    });
}

#[test]
fn cancelled_finish_does_not_publish_a_selection() {
    with_batch(|batch, _, computation, counters| {
        let cancellation = zetesis_algebra::Cancellation::default();
        counters.attach_test_control(&cancellation);
        cancellation.cancel();
        let result = batch.finish(
            (FormulaResource::Atoms, 4),
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        );
        counters.detach_test_control();
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: zetesis_algebra::Stop::Cancelled,
                ..
            })
        ));
    });
}

#[test]
fn merging_keeps_existing_coordinates_and_appends_new_ones() {
    // The selection already holds p(2) and p(0); the batch names p(3), p(2),
    // p(1) and p(0) with repeats. Existing atoms keep their coordinates and
    // are visited once each; p(1) and p(3) follow in increasing source order.
    with_batch(|batch, sources, computation, counters| {
        let limits = FormulaLimits::default();
        let mut selection =
            SourceSelection::new(computation, &limits, counters, location()).unwrap();
        for index in [2, 0] {
            selection
                .insert(
                    &sources[index],
                    (FormulaResource::Atoms, 4),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
        }
        let mut visited = Vec::new();
        batch
            .merge_into(
                &mut selection,
                (FormulaResource::Atoms, 4),
                Context::new(&*computation, &limits, counters, location()),
                |local, _| {
                    visited.push(local);
                    Ok(())
                },
            )
            .unwrap();
        visited.sort_unstable();
        assert_eq!(visited, [0, 1, 2, 3]);
        let expected = [Some(1), Some(2), Some(0), Some(3)];
        for (source, expected) in sources.iter().zip(expected) {
            assert_eq!(
                selection
                    .position(source, &limits, counters, location())
                    .unwrap(),
                expected
            );
        }
    });
}

#[test]
fn merging_counts_distinct_new_identities_against_the_atom_limit() {
    with_batch(|batch, sources, computation, counters| {
        let limits = FormulaLimits::default();
        let mut selection =
            SourceSelection::new(computation, &limits, counters, location()).unwrap();
        selection
            .insert(
                &sources[0],
                (FormulaResource::Atoms, 3),
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
        assert!(matches!(
            batch.merge_into(
                &mut selection,
                (FormulaResource::Atoms, 3),
                Context::new(&*computation, &limits, counters, location()),
                |_, _| Ok(()),
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Atoms,
                observed: 4,
                limit: 3,
                ..
            })
        ));
        assert_eq!(selection.len(), 1);
    });
}
