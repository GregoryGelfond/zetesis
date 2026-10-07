use super::*;

#[test]
fn filtering_does_not_alias_at_the_byte_boundary() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        (0..=256)
            .chain([0])
            .map(|value| vec![Value::Number(value)])
            .collect(),
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert_eq!(relation.column(0).unwrap().bits(), 16);
    let value = Value::Number(256);
    let query = relation
        .query(&[(0, (&value).into())], Limits::default())
        .unwrap();
    let all = relation.all(Limits::default()).unwrap();
    let selected = relation.select(&query, &all, Limits::default()).unwrap();
    assert_eq!(selected.positions(), [256]);
}

#[test]
fn compact_masks_preserve_duplicate_occurrences() {
    let signature = predicate(1);
    let source = atoms(
        &signature,
        vec![
            vec![Value::Number(7)],
            vec![Value::Number(2)],
            vec![Value::Number(7)],
        ],
    );
    let relation = Relation::from_atoms(&signature, &source, Limits::default()).unwrap();
    assert_eq!(relation.column(0).unwrap().bits(), 8);
    let value = Value::Number(7);
    let query = relation
        .query(&[(0, (&value).into())], Limits::default())
        .unwrap();
    let all = relation.all(Limits::default()).unwrap();
    assert_eq!(
        relation
            .select_mask(&query, &all, Limits::default())
            .unwrap()
            .words(),
        [0b101]
    );
}
