//! Selected order, bounded route applicability and unpublished failure output.

use super::*;

const PERMIT: fn() -> Result<(), Infallible> = || Ok(());
const SELECTED: [usize; 9] = [8, 0, 7, 1, 6, 2, 5, 3, 4];
const ORDERED: [usize; 9] = [1, 3, 5, 7, 8, 6, 4, 2, 0];

fn fixture() -> AtomInterner {
    let mut owner = owner(&[9, 1, 8, 2]);
    owner.commit_with(limits(), PERMIT).unwrap();
    for number in [7, 3, 6, 4, 5] {
        insert(&mut owner, &atom(number));
    }
    // Exercise path growth, including its actual old/new overlap.
    owner.index.path = Vec::new();
    owner.restart_storage_peak();
    owner
}

fn work() -> usize {
    let mut owner = fixture();
    let mut selected = SELECTED;
    let mut work = 0;
    owner
        .appender()
        .order_selected_with(&mut selected, limits(), || {
            work += 1;
            PERMIT()
        })
        .unwrap();
    assert_eq!(selected, ORDERED);
    work
}

#[test]
fn selected_order_matches_typed_atom_order() {
    let mut owner = AtomInterner::new();
    let mut atoms = Vec::new();
    for number in (0..130).rev() {
        let value = match number % 3 {
            0 => Value::String(format!("value-{number}")),
            1 => Value::Symbol(format!("value-{number}")),
            _ => Value::from_nodes(
                vec![
                    ValueNode::Function {
                        name: "nested".into(),
                        sign: Sign::Negative,
                        arity: 1,
                    },
                    ValueNode::Number(number),
                ],
                ValueLimits::default(),
            )
            .unwrap(),
        };
        let atom = Atom::new(
            Predicate::with_sign(
                if number % 5 == 0 { "q" } else { "p" },
                2,
                if number % 2 == 0 {
                    Sign::Negative
                } else {
                    Sign::Positive
                },
            )
            .unwrap(),
            vec![value.clone(), value],
        )
        .unwrap();
        assert_eq!(insert(&mut owner, &atom), atoms.len());
        atoms.push(atom);
        if atoms.len() == 65 {
            owner.commit_with(limits(), PERMIT).unwrap();
        }
    }
    // This completed canonical row deliberately has no discovery position.
    owner
        .store
        .import_atom(&atom(999), TermLimits::default())
        .unwrap();
    let mut selected: Vec<_> = (0..130)
        .rev()
        .filter(|id| id % 2 == 0 || *id == 129)
        .collect();
    let mut expected = selected.clone();
    expected.sort_by(|left, right| atoms[*left].cmp(&atoms[*right]));
    let (committed, mut append) = owner.split();
    append
        .order_selected_with(&mut selected, limits(), PERMIT)
        .unwrap();
    assert_eq!(selected, expected);
    assert_eq!(committed.len(), 65);
    assert_eq!(committed.get(0).unwrap(), AtomRef::from(&atoms[0]));
}

#[test]
fn selected_order_rejects_malformed_positions() {
    for selected in [vec![1, 1], vec![0, 9], vec![9]] {
        let mut owner = fixture();
        let mut actual = selected.clone();
        assert!(matches!(
            owner
                .appender()
                .order_selected_with(&mut actual, limits(), PERMIT),
            Err(Failure::Catalog(crate::catalog::Error::Shape))
        ));
        assert_eq!(actual, selected);
        validate(&owner);
    }
}

#[test]
fn trivial_selections_need_no_scratch() {
    let mut owner = fixture();
    let current = owner.storage_bytes();
    let exact = Limits {
        max_bytes: current,
        ..limits()
    };
    for mut selected in [Vec::new(), vec![8]] {
        owner
            .appender()
            .order_selected_with(&mut selected, exact, PERMIT)
            .unwrap();
        assert_eq!(owner.storage_bytes(), current);
        assert_eq!(owner.storage_peak_bytes(), current);
    }
}

#[test]
fn dense_strategy_excludes_sparse_history() {
    let mut owner = AtomInterner::new();
    for count in 1..65 {
        insert(&mut owner, &atom(count));
        let append = owner.appender();
        assert!(append.selected_order_storage(1).is_none());
        assert!(append.selected_order_storage(append.len() + 1).is_none());
        if append.len() >= 4 {
            let half = append.len().div_ceil(2);
            assert!(append.selected_order_storage(half).is_some());
            assert!(append.selected_order_storage(half - 1).is_none());
        }
    }
}

#[test]
fn preflight_covers_requested_order_storage() {
    let mut owner = fixture();
    let retained = owner.storage_bytes();
    let mut append = owner.appender();
    let estimated = append.selected_order_storage(SELECTED.len()).unwrap();
    assert_eq!(append.storage_bytes(), retained);
    assert_eq!(append.storage_peak_bytes(), retained);
    let mut selected = SELECTED;
    append
        .order_selected_with(
            &mut selected,
            Limits {
                max_bytes: estimated,
                ..limits()
            },
            PERMIT,
        )
        .unwrap();
    assert_eq!(selected, ORDERED);
    assert!(append.storage_peak_bytes() <= estimated);
}

#[test]
fn selected_order_obeys_the_combined_storage_peak() {
    let mut complete = fixture();
    let mut selected = SELECTED;
    complete
        .appender()
        .order_selected_with(&mut selected, limits(), PERMIT)
        .unwrap();
    let peak = complete.storage_peak_bytes();
    for bound in [peak - 1, peak] {
        let mut owner = fixture();
        let mut selected = SELECTED;
        let result = owner.appender().order_selected_with(
            &mut selected,
            Limits {
                max_bytes: bound,
                ..limits()
            },
            PERMIT,
        );
        if bound == peak {
            result.unwrap();
            assert_eq!(selected, ORDERED);
        } else {
            assert!(matches!(result, Err(Failure::Bytes { required, limit })
                if required > limit && limit == bound));
            assert_eq!(selected, SELECTED);
        }
        validate(&owner);
    }
}

#[test]
fn every_order_cut_preserves_the_original_selection() {
    for cut in 0..work() {
        let mut owner = fixture();
        let mut selected = SELECTED;
        let mut visited = 0;
        let result = owner
            .appender()
            .order_selected_with(&mut selected, limits(), || {
                if visited == cut {
                    return Err(cut);
                }
                visited += 1;
                Ok(())
            });
        assert!(matches!(result, Err(Failure::Stopped(actual)) if actual == cut));
        assert_eq!(selected, SELECTED);
        validate(&owner);
        owner
            .appender()
            .order_selected_with(&mut selected, limits(), PERMIT)
            .unwrap();
        assert_eq!(selected, ORDERED);
    }
}

fn unwind_order(cut: usize) -> (AtomInterner, [usize; 9]) {
    let mut owner = fixture();
    let mut selected = SELECTED;
    let mut visited = 0;
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner
            .appender()
            .order_selected_with(&mut selected, limits(), || {
                assert_ne!(visited, cut, "injected order panic");
                visited += 1;
                PERMIT()
            });
    }));
    assert!(panic.is_err());
    (owner, selected)
}

#[test]
fn order_unwind_preserves_the_original_selection() {
    for cut in 0..work() {
        let (mut owner, mut selected) = unwind_order(cut);
        assert_eq!(selected, SELECTED);
        validate(&owner);
        owner
            .appender()
            .order_selected_with(&mut selected, limits(), PERMIT)
            .unwrap();
        assert_eq!(selected, ORDERED);
    }
}

#[test]
fn order_unwind_preserves_the_actual_peak_receipt() {
    for cut in 0..work() {
        let mut refused = fixture();
        let mut selected = SELECTED;
        let mut visited = 0;
        let _ = refused
            .appender()
            .order_selected_with(&mut selected, limits(), || {
                if visited == cut {
                    return Err(());
                }
                visited += 1;
                Ok(())
            });
        let (unwound, _) = unwind_order(cut);
        assert_eq!(
            unwound.storage_peak_bytes(),
            refused.storage_peak_bytes(),
            "cut {cut}"
        );
    }
}

#[test]
fn dense_order_uses_linear_metadata_work() {
    for count in [16, 64, 130] {
        let values: Vec<_> = (0..count).rev().collect();
        let mut owner = owner(&values);
        let mut selected: Vec<_> = (0..owner.len()).collect();
        let mut work = 0;
        owner
            .appender()
            .order_selected_with(&mut selected, limits(), || {
                work += 1;
                PERMIT()
            })
            .unwrap();
        // Directory/node/path visits, bit operations and output/publication
        // writes are linear; geometric path relocations fit this loose bound.
        assert!(work <= 16 * owner.len() + 32, "n={count}, work={work}");
        let expected: Vec<_> = (0..owner.len()).rev().collect();
        assert_eq!(selected, expected);
    }
}

fn text_order_work(width: usize) -> usize {
    let mut owner = AtomInterner::new();
    for suffix in (0..8).rev() {
        let atom = Atom::new(
            Predicate::new("text", 1).unwrap(),
            vec![Value::String(format!("{}{suffix}", "a".repeat(width)))],
        )
        .unwrap();
        insert(&mut owner, &atom);
    }
    let mut selected: Vec<_> = (0..owner.len()).collect();
    let mut work = 0;
    owner
        .appender()
        .order_selected_with(&mut selected, limits(), || {
            work += 1;
            PERMIT()
        })
        .unwrap();
    assert_eq!(selected, (0..8).rev().collect::<Vec<_>>());
    work
}

#[test]
fn selected_order_does_not_inspect_payload() {
    assert_eq!(text_order_work(1), text_order_work(8192));
}
