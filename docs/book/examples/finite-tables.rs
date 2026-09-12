//! Project complete typed rows and restore a previously wider domain.

// ANCHOR: example
use zetesis_core::{Atom, Predicate, Value, relation::Relation};
use zetesis_cpu::{
    Control,
    table::{Limits, Table},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pair = Predicate::new("pair", 2)?;
    let atoms = [(1, 2), (1, 3), (2, 4)]
        .into_iter()
        .map(|(left, right)| {
            Atom::new(
                pair.clone(),
                vec![Value::Number(left), Value::Number(right)],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let relation = Relation::from_atoms(&pair, &atoms, Default::default())?;
    let control = Control::default();
    let table = Table::prepare(&relation, &[0, 1], Limits::default(), &control)?;
    let departures = [Value::Number(1), Value::Number(2)];
    let arrivals = [Value::Number(3)];
    let restricted = table.project(&[&departures, &arrivals], Limits::default(), &control)?;
    assert_eq!(
        restricted.domain(0).unwrap().collect::<Vec<_>>(),
        vec![&Value::Number(1)]
    );
    assert!(restricted.contains(1));
    assert!(!restricted.contains(0));

    let arrivals = [Value::Number(2), Value::Number(3), Value::Number(4)];
    let restored = table.project(&[&departures, &arrivals], Limits::default(), &control)?;
    assert_eq!(
        restored.domain(0).unwrap().collect::<Vec<_>>(),
        departures.iter().collect::<Vec<_>>()
    );
    assert!((0..3).all(|row| restored.contains(row)));
    Ok(())
}
// ANCHOR_END: example
