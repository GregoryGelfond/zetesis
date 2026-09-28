use super::*;
use crate::Backend;
use crate::relation_fixtures::{Family, Payload};

#[test]
fn lost_selected_rows_fail_the_independent_reference() {
    let configuration = Configuration {
        family: Family::Correlated,
        payload: Payload::Numeric,
        rows: 33,
        queries: 8,
        backend: Backend::Cpu,
        workers: 2,
        warmups: 0,
        repetitions: 1,
        max_bytes: 128 * 1024 * 1024,
    };
    let fixture = Fixture::new(
        configuration.family,
        configuration.payload,
        configuration.rows,
        configuration.queries,
        FixtureLimits::default(),
    )
    .unwrap();
    let relation =
        Relation::from_atoms(fixture.predicate(), fixture.atoms(), Limits::default()).unwrap();
    let (queries, bytes, _) = queries(&fixture, &relation, configuration.max_bytes).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let frame = Frame {
        fixture: &fixture,
        relation: &relation,
        queries: &queries,
        input: &input,
        configuration,
        shared_bytes: fixture.storage().retained_bytes
            + relation.storage().retained_bytes
            + bytes
            + input.retained_bytes(),
        column_bytes: 0,
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let mut batch = cpu_batch(&frame, Route::Scalar, &pool, &Cancellation::default()).unwrap();
    validate(&frame, &batch.masks, &Cancellation::default()).unwrap();
    batch.masks[0] &= !1;
    assert!(matches!(
        validate(&frame, &batch.masks, &Cancellation::default()),
        Err(Error::Parity)
    ));
}

#[test]
fn truncated_mask_populations_fail_validation() {
    let configuration = Configuration {
        family: Family::Single,
        payload: Payload::Tuple,
        rows: 1,
        queries: 1,
        backend: Backend::Cpu,
        workers: 1,
        warmups: 0,
        repetitions: 1,
        max_bytes: 128 * 1024 * 1024,
    };
    let fixture = Fixture::new(
        configuration.family,
        configuration.payload,
        1,
        1,
        FixtureLimits::default(),
    )
    .unwrap();
    let relation =
        Relation::from_atoms(fixture.predicate(), fixture.atoms(), Limits::default()).unwrap();
    let (queries, _, _) = queries(&fixture, &relation, configuration.max_bytes).unwrap();
    let input = relation.all(Limits::default()).unwrap();
    let frame = Frame {
        fixture: &fixture,
        relation: &relation,
        queries: &queries,
        input: &input,
        configuration,
        shared_bytes: 0,
        column_bytes: 0,
    };
    assert!(matches!(
        validate(&frame, &[], &Cancellation::default()),
        Err(Error::Parity)
    ));
}
