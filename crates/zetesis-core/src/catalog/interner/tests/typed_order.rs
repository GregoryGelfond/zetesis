//! Typed order and membership are exact whichever way an atom arrives:
//! ordered requests at any point return the reference typed order of the
//! committed atoms, any form of an atom finds its one position, and an atom
//! arriving above every atom of its predicate is placed without a typed search.

use super::*;

/// Work charged to intern one wide atom per value, in the given order.
fn charged(values: impl Iterator<Item = i32>) -> u64 {
    let mut owner = AtomInterner::new();
    let mut steps = 0_u64;
    let limits = Limits {
        max_atoms: 1 << 20,
        max_bytes: 1 << 30,
    };
    for value in values {
        // Six arguments sharing their leading columns, as a rule's derived
        // heads do: every typed comparison reads each column.
        let wide = Atom::new(
            Predicate::new("w", 6).unwrap(),
            vec![
                Value::Number(1),
                Value::Number(2),
                Value::Number(3),
                Value::Number(4),
                Value::Number(value / 64),
                Value::Number(value % 64),
            ],
        )
        .unwrap();
        owner
            .entry_atom_with(&wide, limits, || {
                steps += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap()
            .insert_with(limits, || {
                steps += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
    }
    steps
}

#[test]
fn ascending_arrivals_skip_the_typed_search() {
    const COUNT: i32 = 1 << 12;
    // The same atoms; a scrambled order needs the typed search for each.
    let ascending = charged(0..COUNT);
    let scrambled = charged((0..COUNT).map(|index| index.wrapping_mul(7919) % COUNT));
    assert!(
        ascending * 2 <= scrambled,
        "ascending {ascending}, scrambled {scrambled}"
    );
}

#[test]
fn the_current_maximum_is_found_without_insertion() {
    let mut owner = AtomInterner::new();
    for value in 0..10 {
        insert(&mut owner, &atom(value));
    }
    assert_eq!(insert(&mut owner, &atom(9)), 9);
    assert_eq!(owner.len(), 10);
}

/// Atoms of two predicates over numbers, strings and symbols, so typed order
/// crosses predicates and value kinds.
fn mixed(code: u8) -> Atom {
    let value = match code % 3 {
        0 => Value::Number(i32::from(code / 3) - 20),
        1 => Value::String(format!("s{}", code / 3)),
        _ => Value::Symbol(format!("s{}", code / 3)),
    };
    if code.is_multiple_of(2) {
        Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()
    } else {
        Atom::new(
            Predicate::new("q", 2).unwrap(),
            vec![value, Value::Number(i32::from(code % 5))],
        )
        .unwrap()
    }
}

/// One step of an interleaving: intern an atom, commit, or ask for order.
#[derive(Clone, Debug)]
enum Step {
    Insert(u8),
    Commit,
    Order,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        6 => any::<u8>().prop_map(Step::Insert),
        1 => Just(Step::Commit),
        1 => Just(Step::Order),
    ]
}

proptest! {
    #[test]
    fn interleaved_ordered_requests_return_the_typed_order(steps in prop::collection::vec(step(), 0..120)) {
        let mut owner = AtomInterner::new();
        let mut discovered: Vec<Atom> = Vec::new();
        let mut committed = 0;
        for step in steps {
            match step {
                Step::Insert(code) => {
                    let atom = mixed(code);
                    let position = insert(&mut owner, &atom);
                    let expected = discovered.iter().position(|known| *known == atom).unwrap_or_else(|| {
                        discovered.push(atom.clone());
                        discovered.len() - 1
                    });
                    prop_assert_eq!(position, expected);
                }
                Step::Commit => {
                    owner.commit_with(limits(), || Ok::<(), Infallible>(())).unwrap();
                    committed = discovered.len();
                }
                Step::Order => {
                    let order = owner.ordered_ids_with(limits(), || Ok::<(), Infallible>(())).unwrap();
                    let mut expected: Vec<_> = discovered[..committed].to_vec();
                    expected.sort();
                    let actual: Vec<_> = order.iter().map(|&id| owner.get(id).unwrap()).collect();
                    prop_assert_eq!(actual, expected.iter().map(AtomRef::from).collect::<Vec<_>>());
                }
            }
        }
    }
}

#[test]
fn every_form_of_an_atom_finds_its_one_position() {
    let mut owner = AtomInterner::new();
    let atoms: Vec<_> = (0..30).map(mixed).collect();
    for atom in &atoms {
        insert(&mut owner, atom);
    }
    let mut foreign = AtomInterner::new();
    for atom in atoms.iter().rev() {
        insert(&mut foreign, atom);
    }
    for (position, atom) in atoms.iter().enumerate() {
        let found = |query: AtomRef<'_>| {
            owner
                .find_atom_with(query, limits(), || Ok::<(), Infallible>(()))
                .unwrap()
        };
        // Owned ingress, this owner's own canonical form, another owner's.
        assert_eq!(found(AtomRef::from(atom)), Some(position));
        assert_eq!(found(owner.get(position).unwrap()), Some(position));
        let elsewhere = foreign
            .find_atom_with(atom, limits(), || Ok::<(), Infallible>(()))
            .unwrap()
            .unwrap();
        assert_eq!(found(foreign.get(elsewhere).unwrap()), Some(position));
    }
}

#[test]
fn an_atom_with_an_unknown_argument_is_absent() {
    let owner = {
        let mut owner = AtomInterner::new();
        insert(&mut owner, &mixed(0));
        insert(&mut owner, &mixed(1));
        owner
    };
    let find = |atom: &Atom| {
        owner
            .find_atom_with(atom, limits(), || Ok::<(), Infallible>(()))
            .unwrap()
    };
    // A string and a symbol of the same text are distinct values.
    let string = Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::String("s0".to_owned())],
    )
    .unwrap();
    let symbol = Atom::new(
        Predicate::new("p", 1).unwrap(),
        vec![Value::Symbol("s0".to_owned())],
    )
    .unwrap();
    assert_eq!(find(&mixed(0)), Some(0));
    assert_eq!(find(&string), None);
    assert_eq!(find(&symbol), None);
    assert_eq!(find(&atom(12345)), None);
}
