use super::*;

#[test]
fn borrowed_intersection_keeps_pair_level_work() {
    let mut calls = Vec::new();
    {
        let mut work = Work::new(
            Limits::default(),
            |amount| {
                calls.push(amount);
                Ok::<_, ()>(())
            },
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            intersects(&[1, 2, 4, 8], &[0, 2, 4, 8], &mut work),
            Ok(true)
        );
        assert_eq!(work.statistics(0).work, 2);
    }
    assert_eq!(calls, [0, 0, 1, 0, 1]);
}

#[test]
fn borrowed_intersection_keeps_local_refusal_prefix() {
    for permitted in 0..3 {
        let limits = Limits {
            max_work: permitted,
            ..Limits::default()
        };
        let mut work = Work::new(limits, |_| Ok::<_, ()>(()), 0, 0).unwrap();
        let result = intersects(&[1, 2, 4], &[0, 0, 4], &mut work);
        assert_eq!(
            result,
            Err(MeteredCause::Table(Cause::Limit {
                resource: Resource::Work,
                observed: u128::from(permitted + 1),
                limit: u128::from(permitted),
            }))
        );
        assert_eq!(work.statistics(0).work, permitted);
    }
}

#[test]
fn borrowed_intersection_keeps_external_refusal_value() {
    let mut admitted = 0;
    let mut work = Work::new(
        Limits::default(),
        |amount| {
            if amount == 1 && admitted == 1 {
                return Err(("pair", admitted));
            }
            admitted += amount;
            Ok(())
        },
        0,
        0,
    )
    .unwrap();
    assert_eq!(
        intersects(&[1, 2, 4], &[0, 0, 4], &mut work),
        Err(MeteredCause::Stopped(("pair", 1)))
    );
    assert_eq!(work.statistics(0).work, 1);
}

#[test]
fn table_keeps_its_original_record_layout() {
    use std::mem::{align_of, offset_of, size_of};

    struct Original<'a> {
        relation: &'a Relation<'a>,
        scope: Vec<usize>,
        variables: Vec<Range<usize>>,
        values: Vec<TermRef<'a>>,
        supports: Vec<u32>,
        coherent: Vec<u32>,
        statistics: Statistics,
    }
    assert_eq!(size_of::<Table<'_, '_>>(), size_of::<Original<'_>>());
    assert_eq!(align_of::<Table<'_, '_>>(), align_of::<Original<'_>>());
    macro_rules! offset {
        ($field:ident) => {
            assert_eq!(
                offset_of!(Table<'_, '_>, $field),
                offset_of!(Original<'_>, $field)
            );
        };
    }
    offset!(relation);
    offset!(scope);
    offset!(variables);
    offset!(values);
    offset!(supports);
    offset!(coherent);
    offset!(statistics);
}

#[test]
fn projection_keeps_its_original_record_layout() {
    use std::mem::{align_of, offset_of, size_of};

    struct Original<'a> {
        table: &'a Table<'a, 'a>,
        rows: Vec<u32>,
        supported: Vec<bool>,
        statistics: Statistics,
    }
    assert_eq!(
        size_of::<Projection<'_, '_, '_>>(),
        size_of::<Original<'_>>()
    );
    assert_eq!(
        align_of::<Projection<'_, '_, '_>>(),
        align_of::<Original<'_>>()
    );
    macro_rules! offset {
        ($field:ident) => {
            assert_eq!(
                offset_of!(Projection<'_, '_, '_>, $field),
                offset_of!(Original<'_>, $field)
            );
        };
    }
    offset!(table);
    offset!(rows);
    offset!(supported);
    offset!(statistics);
}

#[test]
fn projections_borrow_the_same_unchanged_table() {
    let predicate = Predicate::new("table", 1).unwrap();
    let values: Vec<_> = (0..65).map(Value::Number).collect();
    let rows: Vec<_> = values.iter().cloned().map(|value| vec![value]).collect();
    let source = atoms(&predicate, &rows);
    let relation = Relation::from_atoms(&predicate, &source, RelationLimits::default()).unwrap();
    let table =
        Table::prepare(&relation, &[0], Limits::default(), &Cancellation::default()).unwrap();
    assert!(std::ptr::eq(table.relation, &raw const relation));
    let supports = (table.supports.as_ptr(), table.supports.capacity());
    let coherent = (table.coherent.as_ptr(), table.coherent.capacity());
    let first = table
        .project(&[&values], Limits::default(), &Cancellation::default())
        .unwrap();
    let second = table
        .project(&[&values], Limits::default(), &Cancellation::default())
        .unwrap();
    assert!(std::ptr::eq(first.table, &raw const table));
    assert!(std::ptr::eq(second.table, &raw const table));
    assert_eq!(
        (table.supports.as_ptr(), table.supports.capacity()),
        supports
    );
    assert_eq!(
        (table.coherent.as_ptr(), table.coherent.capacity()),
        coherent
    );
    assert_ne!(first.rows.as_ptr(), second.rows.as_ptr());
    assert_eq!(&*first.rows, &[u32::MAX, u32::MAX, 1]);
    assert_eq!(&*second.rows, &*first.rows);
    assert!((0..65).all(|row| first.contains(row) && second.contains(row)));
}
