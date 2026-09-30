//! Prepared relation queries retain exact occurrence identities across refusals.

use zetesis_core::catalog::TermRef;
use zetesis_core::relation::{Failure, Limits, Relation, Resource};
use zetesis_core::{Atom, Predicate, Value, ValueNodeRef};

fn refusal<T>(result: Result<T, Failure>) -> Failure {
    let Err(error) = result else {
        panic!("a tighter query ceiling must refuse before publishing output");
    };
    error
}

fn assert_occurrences(
    relation: &Relation<'_>,
    source: &[Atom],
    indices: &[usize],
    before: &[[TermRef<'_>; 2]],
) {
    for (position, &source_index) in indices.iter().enumerate() {
        let row = relation.row(position).unwrap();
        assert_eq!(row.position(), position);
        assert_eq!(row.source_index(), source_index);
        for (column, value) in source[source_index].values().iter().enumerate() {
            let representative = row.value(column).unwrap();
            assert_eq!(representative, TermRef::from(value));
            assert_eq!(representative, before[position][column]);
            // Borrowed text must still reference original source payload;
            // scalar descriptor views carry the typed value by copy.
            if let ValueNodeRef::String(text) | ValueNodeRef::Symbol(text) =
                representative.descriptor()
            {
                assert!(source.iter().flat_map(Atom::values).any(|original| {
                    match original {
                        Value::String(original) | Value::Symbol(original) => {
                            std::ptr::eq(text, original.as_str())
                        }
                        _ => false,
                    }
                }));
                let previous = before[position][column].descriptor();
                assert!(
                    matches!(previous, ValueNodeRef::String(original) | ValueNodeRef::Symbol(original) if std::ptr::eq(text, original))
                );
            }
        }
    }
}

#[test]
fn tighter_query_limits_preserve_prepared_occurrences() {
    let predicate = Predicate::new("pair", 2).unwrap();
    let source: Vec<_> = [
        vec![Value::Number(1), Value::String("1".into())],
        vec![Value::Number(2), Value::String("unused".into())],
        vec![Value::Number(1), Value::Symbol("1".into())],
    ]
    .into_iter()
    .map(|values| Atom::new(predicate.clone(), values).unwrap())
    .collect();
    let indices = [2, 0, 2];
    let relation =
        Relation::from_catalog(&predicate, &source, &indices, Limits::default()).unwrap();
    let before: [[TermRef<'_>; 2]; 3] = std::array::from_fn(|position| {
        std::array::from_fn(|column| relation.row(position).unwrap().value(column).unwrap())
    });
    let symbol = Value::Symbol("1".into());
    let string = Value::String("1".into());
    let query = relation
        .query(&[(1, TermRef::from(&symbol))], Limits::default())
        .unwrap();
    let input = relation.all(Limits::default()).unwrap();
    // There are three occurrences, two columns, and exactly three typed values:
    // Number(1), String("1"), Symbol("1"). The unselected source row contributes
    // neither a fourth occurrence nor its two dictionary values.
    let exact = Limits {
        max_rows: 3,
        max_columns: 2,
        max_values: 3,
        ..Limits::default()
    };
    for (resource, limits, observed, limit) in [
        (
            Resource::Rows,
            Limits {
                max_rows: 2,
                ..exact
            },
            3,
            2,
        ),
        (
            Resource::Columns,
            Limits {
                max_columns: 1,
                ..exact
            },
            2,
            1,
        ),
        (
            Resource::Values,
            Limits {
                max_values: 2,
                ..exact
            },
            3,
            2,
        ),
    ] {
        let expected = Failure::Limit {
            resource,
            observed,
            limit,
        };
        assert_eq!(
            refusal(relation.query(&[(1, TermRef::from(&symbol))], limits)),
            expected
        );
        assert_eq!(refusal(relation.select(&query, &input, limits)), expected);
        assert_eq!(
            refusal(relation.select_mask(&query, &input, limits)),
            expected
        );

        // Refusal publishes no replacement query/selection and does not rebase
        // existing owners or collapse two occurrences of original source row 2.
        assert!(relation.same_owner(query.relation()));
        assert!(relation.same_owner(input.relation()));
        assert_eq!(input.positions(), &[0, 1, 2]);
        assert_occurrences(&relation, &source, &indices, &before);

        let resumed = relation
            .query(&[(1, TermRef::from(&symbol))], exact)
            .unwrap();
        assert!(relation.same_owner(resumed.relation()));
        let selected = relation.select(&resumed, &input, exact).unwrap();
        assert!(relation.same_owner(selected.relation()));
        assert_eq!(selected.positions(), &[0, 2]);
        assert_eq!(selected.row(0).unwrap().source_index(), 2);
        assert_eq!(selected.row(1).unwrap().source_index(), 2);
        assert_eq!(
            relation
                .select_mask(&resumed, &input, exact)
                .unwrap()
                .words(),
            &[0b101]
        );

        let other = relation
            .query(&[(1, TermRef::from(&string))], exact)
            .unwrap();
        let selected = relation.select(&other, &input, exact).unwrap();
        assert_eq!(selected.positions(), &[1]);
        assert_eq!(selected.row(0).unwrap().source_index(), 0);
    }
}
