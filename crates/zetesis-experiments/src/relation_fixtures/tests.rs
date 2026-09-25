use zetesis_core::relation::{Limits as RelationLimits, Relation};

use super::*;

fn fixture(family: Family, payload: Payload) -> Fixture {
    Fixture::new(family, payload, 16, 16, Limits::default()).unwrap()
}

#[test]
fn scalar_columns_match_the_independent_row_reference() {
    for family in [
        Family::Single,
        Family::Independent,
        Family::Correlated,
        Family::Skewed,
    ] {
        for payload in [Payload::Numeric, Payload::Tuple] {
            let fixture = fixture(family, payload);
            let relation = Relation::from_atoms(
                fixture.predicate(),
                fixture.atoms(),
                RelationLimits::default(),
            )
            .unwrap();
            let input = relation.all(RelationLimits::default()).unwrap();
            for (index, keys) in fixture.queries().iter().enumerate() {
                let keys: Vec<_> = keys
                    .iter()
                    .map(|(column, value)| (*column, value.into()))
                    .collect();
                let query = relation.query(&keys, RelationLimits::default()).unwrap();
                let selected = relation
                    .select(&query, &input, RelationLimits::default())
                    .unwrap();
                assert_eq!(
                    selected.positions(),
                    fixture.reference_positions(index, 16).unwrap()
                );
            }
        }
    }
}

#[test]
fn selected_rows_reconstruct_the_original_typed_values() {
    let fixture = fixture(Family::Independent, Payload::Tuple);
    let relation = Relation::from_atoms(
        fixture.predicate(),
        fixture.atoms(),
        RelationLimits::default(),
    )
    .unwrap();
    for query in 0..fixture.queries().len() {
        let positions = fixture.reference_positions(query, 16).unwrap();
        let selection = relation
            .selection(&positions, RelationLimits::default())
            .unwrap();
        for index in 0..positions.len() {
            let row = selection.row(index).unwrap();
            let source = &fixture.atoms()[row.source_index()];
            assert_eq!(
                row.predicate(),
                zetesis_core::catalog::PredicateRef::from(source.predicate())
            );
            for (column, value) in source.values().iter().enumerate() {
                assert_eq!(row.value(column), Some(value.into()));
            }
        }
    }
}

#[test]
fn identity_arguments_keep_source_atoms_unique() {
    let fixture = fixture(Family::Correlated, Payload::Numeric);
    let unique: std::collections::BTreeSet<_> = fixture.atoms().iter().collect();
    assert_eq!(unique.len(), fixture.atoms().len());
}

#[test]
fn empty_equalities_select_all_original_rows() {
    let fixture = fixture(Family::Skewed, Payload::Numeric);
    assert_eq!(
        fixture.reference_positions(0, 16).unwrap(),
        (0..16).collect::<Vec<_>>()
    );
}

#[test]
fn missing_values_have_no_source_matches() {
    let fixture = fixture(Family::Skewed, Payload::Tuple);
    assert!(fixture.reference_positions(1, 16).unwrap().is_empty());
}

#[test]
fn repeated_query_occurrences_are_retained() {
    let fixture = fixture(Family::Single, Payload::Numeric);
    assert_eq!(fixture.queries().len(), 16);
    assert_eq!(fixture.queries()[0], fixture.queries()[8]);
}

#[test]
fn zero_queries_remain_an_empty_population() {
    let fixture = Fixture::new(Family::Single, Payload::Numeric, 1, 0, Limits::default()).unwrap();
    assert!(fixture.queries().is_empty());
    assert_eq!(fixture.storage().equalities, 0);
}

#[test]
fn finite_dimensions_are_checked_before_construction() {
    for (rows, queries) in [(0, 1), (65_537, 1), (1, 257)] {
        assert!(matches!(
            Fixture::new(
                Family::Single,
                Payload::Numeric,
                rows,
                queries,
                Limits::default()
            ),
            Err(Error::Dimensions)
        ));
    }
}

#[test]
fn storage_excess_does_not_publish_a_fixture() {
    let fixture = fixture(Family::Single, Payload::Numeric);
    let maximum = fixture.storage().peak_construction_bytes;
    assert!(
        Fixture::new(
            Family::Single,
            Payload::Numeric,
            16,
            16,
            Limits { max_bytes: maximum }
        )
        .is_ok()
    );
    assert!(matches!(
        Fixture::new(
            Family::Single,
            Payload::Numeric,
            16,
            16,
            Limits {
                max_bytes: maximum - 1
            }
        ),
        Err(Error::Bytes)
    ));
}

#[test]
fn tuple_construction_accounts_temporary_storage() {
    let fixture = Fixture::new(Family::Single, Payload::Tuple, 1, 0, Limits::default()).unwrap();
    assert!(fixture.storage().peak_construction_bytes > fixture.storage().retained_bytes);
}

#[test]
fn a_truncated_reference_is_an_error() {
    let fixture = fixture(Family::Single, Payload::Numeric);
    assert!(matches!(
        fixture.reference_positions(0, 15),
        Err(Error::Positions)
    ));
}

#[test]
fn empty_results_need_no_position_allowance() {
    let fixture = fixture(Family::Single, Payload::Numeric);
    assert!(fixture.reference_positions(1, 0).unwrap().is_empty());
}

#[test]
fn foreign_query_indices_are_refused() {
    let fixture = fixture(Family::Single, Payload::Numeric);
    assert!(matches!(
        fixture.reference_positions(16, 16),
        Err(Error::Query)
    ));
}
