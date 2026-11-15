//! Identifier selection locates the ordered point query's row at its own cost.

use super::*;
use crate::formula_support::queries::IndexedSelection;
use crate::formula_support::relations::KeyIdentifiers;

/// The two delta positions an occurrence can take around a pivot, and none.
const PIVOTS: [Option<usize>; 4] = [None, Some(0), Some(1), Some(2)];

fn binary(first: &Value, second: &Value) -> Atom {
    Atom::new(
        Predicate::new("p", 2).unwrap(),
        vec![first.clone(), second.clone()],
    )
    .unwrap()
}

/// Compare the indexed fallback with the ordered point query on every key
/// over `values` and one value no row holds, under every delta partition.
///
/// The rows give each value at least two rows in each column, so every key
/// over `values` meets several candidates: seven keys are present and
/// `(values[1], values[1])` and `(values[2], values[0])` are absent.
fn assert_agreement(values: &[Value; 3], unheld: &Value) {
    let [one, two, three] = values;
    let held = [
        (one, one),
        (one, two),
        (two, one),
        (two, three),
        (three, two),
        (three, three),
        (one, three),
    ];
    let atoms = held.map(|(first, second)| binary(first, second));
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let domain = [one, two, three, unheld];
        let mut identified = 0;
        for first in domain {
            for second in domain {
                let key = [Some(first.clone()), Some(second.clone())];
                let candidates = support
                    .probe_at(
                        rows,
                        (&pattern).into(),
                        key.as_slice().into(),
                        &limits,
                        counters,
                        location(),
                    )
                    .unwrap()
                    .unwrap()
                    .len();
                for pivot in PIVOTS {
                    let expected = support
                        .exact_at(
                            rows,
                            PositivePattern::Flat((&pattern).into()),
                            key.as_slice().into(),
                            (pivot, 1),
                            GroundingWork::new(&limits, counters, location()),
                        )
                        .unwrap()
                        .unwrap();
                    let (selection, routes) = routed(true, || {
                        support
                            .indexed_selection_at(
                                rows,
                                PositivePattern::Flat((&pattern).into()),
                                key.as_slice().into(),
                                (pivot, 1),
                                GroundingWork::new(&limits, counters, location()),
                            )
                            .unwrap()
                    });
                    let context = format!("({first:?}, {second:?}) at pivot {pivot:?}");
                    if candidates > 1 {
                        let IndexedSelection::Exact(range) = selection else {
                            panic!("several candidates must be refined: {context}");
                        };
                        assert_eq!(range, expected, "{context}");
                        assert_eq!(routes, [1, 0], "{context}");
                        identified += 1;
                    } else {
                        assert!(
                            matches!(selection, IndexedSelection::Posting(_)),
                            "{context}"
                        );
                        assert_eq!(routes, [0, 0], "{context}");
                    }
                }
            }
        }
        // Exactly the nine keys over the held values meet several candidates.
        assert_eq!(identified, 9 * PIVOTS.len());
    });
}

#[test]
fn identifier_selection_agrees_with_the_ordered_point_query() {
    assert_agreement(
        &[Value::Number(1), Value::Number(2), Value::Number(3)],
        &Value::Number(9),
    );
}

#[test]
fn identifier_selection_keeps_typed_identity() {
    // The three values share the spelling `7` and differ in type.
    let nested = Value::from_nodes(
        vec![
            ValueNode::Function {
                sign: Sign::Positive,
                name: "f".into(),
                arity: 1,
            },
            ValueNode::Number(7),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    assert_agreement(
        &[Value::Number(7), Value::String("7".into()), nested],
        &Value::String("f(7)".into()),
    );
}

/// The work of one indexed fallback beyond that of its probe alone.
fn refinement_work(
    atoms: impl IntoIterator<Item = Atom>,
    key: [i32; 2],
    expected: Range<usize>,
) -> u64 {
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, setup, location())
            .unwrap();
        let values = key.map(|value| Some(Value::Number(value)));
        let mut probe = Counters::default();
        support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &limits,
                &mut probe,
                location(),
            )
            .unwrap();
        let mut actual = Counters::default();
        let (selection, routes) = routed(true, || {
            support
                .indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    values.as_slice().into(),
                    (None, 0),
                    GroundingWork::new(&limits, &mut actual, location()),
                )
                .unwrap()
        });
        let IndexedSelection::Exact(range) = selection else {
            panic!("several candidates must be refined");
        };
        assert_eq!(range, expected);
        assert_eq!(routes, [1, 0]);
        actual.accounting.work - probe.accounting.work
    })
}

#[test]
fn a_located_key_charges_the_comparisons_up_to_its_row() {
    let grid = (0..8).flat_map(|first| (0..8).map(move |second| atom(&[first, second])));
    // The first column's posting holds rows 24..32. Rows 24..28 agree on the
    // first column and differ on the second, two comparisons each; row 28
    // agrees on both. Two partition comparisons and the selection follow.
    assert_eq!(refinement_work(grid, [3, 4], 28..29), 4 * 2 + 2 + 3);
}

#[test]
fn an_absent_key_charges_every_candidate_and_no_partition() {
    let grid = (0..4)
        .flat_map(|first| (0..4).map(move |second| [first, second]))
        .filter(|&row| row != [1, 2])
        .map(|row| atom(&row));
    // The first column's posting holds the three rows (1, 0), (1, 1) and
    // (1, 3): two comparisons each, then the selection of no row.
    assert_eq!(refinement_work(grid, [1, 2], 0..0), 3 * 2 + 1);
}

/// Rows `(corner, corner + j)` and `(corner + j, corner)` for `j < arm`: the
/// key `(corner, corner)` has a posting of `arm` rows in each column.
fn cross(corner: i32, arm: i32) -> impl Iterator<Item = Atom> {
    (0..arm)
        .map(move |offset| atom(&[corner, corner + offset]))
        .chain((1..arm).map(move |offset| atom(&[corner + offset, corner])))
}

/// The routes taken, and agreement with the ordered point query, for the
/// corner key of a cross whose arms hold `arm` rows.
fn corner_routes(arm: usize) -> [usize; 2] {
    let corner = 1000;
    let mut fixture =
        testing::Fixture::from_atoms(cross(corner, i32::try_from(arm).unwrap()), location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let values = [Some(Value::Number(corner)), Some(Value::Number(corner))];
        let posting = support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &limits,
                counters,
                location(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(posting.len(), arm);
        let (selection, routes) = routed(true, || {
            support
                .indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    values.as_slice().into(),
                    (None, 0),
                    GroundingWork::new(&limits, counters, location()),
                )
                .unwrap()
        });
        let IndexedSelection::Exact(range) = selection else {
            panic!("several candidates must be refined");
        };
        // The corner is the first row of its cross.
        assert_eq!(range, 0..1);
        routes
    })
}

#[test]
fn a_posting_at_the_candidate_bound_is_settled_by_identifiers() {
    assert_eq!(corner_routes(IDENTIFIED_CANDIDATES), [1, 0]);
}

#[test]
fn a_posting_past_the_candidate_bound_keeps_the_ordered_point_query() {
    assert_eq!(corner_routes(IDENTIFIED_CANDIDATES + 1), [0, 1]);
}

/// The routes taken for a complete key of `arity` columns whose first two
/// columns each hold two values and whose other columns hold one.
fn wide_routes(arity: usize) -> [usize; 2] {
    let predicate = Predicate::new("p", arity).unwrap();
    let row = |first: i32, second: i32| {
        let mut values = vec![Value::Number(0); arity];
        values[0] = Value::Number(first);
        values[1] = Value::Number(second);
        Atom::new(predicate.clone(), values).unwrap()
    };
    let atoms = [row(0, 0), row(0, 1), row(1, 0), row(1, 1)];
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern =
        AtomPattern::new(predicate.clone(), (0..arity).map(Term::Variable).collect()).unwrap();
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let mut values = vec![Some(Value::Number(0)); arity];
        values[0] = Some(Value::Number(1));
        let (selection, routes) = routed(true, || {
            support
                .indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    values.as_slice().into(),
                    (None, 0),
                    GroundingWork::new(&limits, counters, location()),
                )
                .unwrap()
        });
        let IndexedSelection::Exact(range) = selection else {
            panic!("several candidates must be refined");
        };
        assert_eq!(range, 2..3);
        routes
    })
}

#[test]
fn a_key_filling_the_identifier_record_is_settled_by_identifiers() {
    assert_eq!(wide_routes(KeyIdentifiers::COLUMNS), [1, 0]);
}

#[test]
fn a_key_wider_than_the_identifier_record_keeps_the_ordered_point_query() {
    assert_eq!(wide_routes(KeyIdentifiers::COLUMNS + 1), [0, 1]);
}

#[test]
fn identifier_selection_refusals_keep_the_work_receipt() {
    let grid = (0..4).flat_map(|first| (0..4).map(move |second| atom(&[first, second])));
    let mut fixture = testing::Fixture::from_atoms(grid, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let defaults = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &defaults, setup, location())
            .unwrap();
        let before_bytes = support.live_bytes();
        let execute = |maximum, counters: &mut Counters| {
            let limits = FormulaLimits {
                max_work: maximum,
                ..defaults
            };
            routed(true, || {
                support.indexed_selection_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    [Some(Value::Number(2)), Some(Value::Number(3))]
                        .as_slice()
                        .into(),
                    (None, 0),
                    GroundingWork::new(&limits, counters, location()),
                )
            })
        };
        let mut full = Counters::default();
        let (selection, routes) = execute(defaults.max_work, &mut full);
        assert!(matches!(selection, Ok(IndexedSelection::Exact(range)) if range == (11..12)));
        assert_eq!(routes, [1, 0]);
        let required = full.accounting.work;
        for maximum in 0..required {
            let mut counters = Counters::default();
            let (refused, routes) = execute(maximum, &mut counters);
            let failure = refused.err().expect("less work must refuse the selection");
            assert!(matches!(failure, FormulaFailure::Limit {
                resource: FormulaResource::Work, limit, observed, ..
            } if limit == u128::from(maximum) && observed > limit));
            assert!(counters.accounting.work <= maximum);
            assert_eq!(routes, [0, 0]);
            assert_eq!(support.live_bytes(), before_bytes);
        }
        let mut exact = Counters::default();
        let (selection, _) = execute(required, &mut exact);
        assert!(matches!(selection, Ok(IndexedSelection::Exact(range)) if range == (11..12)));
        assert_eq!(exact.accounting.work, required);
    });
}

#[test]
fn grounding_is_identical_through_either_exact_route() {
    for (source, routes) in [
        // Postings of two rows: identifiers settle each key.
        (
            "a(1).a(2).b(1,1).b(1,2).b(2,1).b(2,2).{p}.h:-a(X),b(X,X),p.",
            [true, false],
        ),
        // The key b(0,0) meets 65 candidates in each column.
        (
            "a(0).b(0,0..64).b(1..64,0).{p}.h:-a(X),b(X,X),p.",
            [false, true],
        ),
    ] {
        let compile = |enabled| {
            routed(enabled, || {
                crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
            })
        };
        let (actual, [identifiers, ordered]) = compile(true);
        let (reference, disabled) = compile(false);
        assert_eq!([identifiers > 0, ordered > 0], routes, "{source}");
        assert_eq!(disabled, [0, 0]);
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(
            actual.theory.operands(),
            reference.theory.operands(),
            "{source}"
        );
        assert_eq!(actual.theory.roots(), reference.theory.roots(), "{source}");
        assert_eq!(actual.origins, reference.origins, "{source}");
        assert_eq!(actual.warnings, reference.warnings, "{source}");
    }
}
