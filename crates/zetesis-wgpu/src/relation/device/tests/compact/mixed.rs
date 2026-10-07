//! Append-built columns can have different physical widths in one upload.

use super::*;
use zetesis_core::atom_interner::{AtomInterner, Limits as AtomLimits};
use zetesis_core::relation::{Catalog, Column};

fn appended() -> (AtomInterner, Catalog) {
    let mut owner = AtomInterner::default();
    let atom_limits = AtomLimits::for_atoms(1_024, 4 * 1024 * 1024);
    let predicate = Predicate::new("mixed", 3).unwrap();
    let declared = owner
        .declare_predicate_with(&predicate, atom_limits, || Ok::<_, ()>(()))
        .unwrap();
    let mut catalog = Catalog::new(owner.read(), declared, Limits::default()).unwrap();
    {
        let mut append = catalog.appender(Limits::default()).unwrap();
        for value in 0..=256 {
            let atom = Atom::new(
                predicate.clone(),
                vec![Value::Number(0), Value::Number(value), Value::Number(1)],
            )
            .unwrap();
            let canonical = owner
                .entry_atom_with(&atom, atom_limits, || Ok::<_, ()>(()))
                .unwrap()
                .insert_ref_with(atom_limits, || Ok::<_, ()>(()))
                .unwrap();
            assert!(
                append
                    .insert(canonical, Limits::default())
                    .unwrap()
                    .insertion
                    .inserted
            );
        }
    }
    (owner, catalog)
}

#[test]
fn mixed_width_upload_preserves_appended_rows() {
    let (owner, catalog) = appended();
    let relation = catalog.view(owner.read()).unwrap();
    assert_eq!(
        relation.columns().map(Column::bits).collect::<Vec<_>>(),
        [8, 16, 8]
    );
    let bytes = packing::column_bytes(&relation, &wgpu::Limits::default()).unwrap();
    assert_eq!(bytes, 1_060);
    let mut target = vec![0xff; usize::try_from(bytes).unwrap()];
    pack_columns(
        &relation,
        wgpu::WriteOnly::from_mut(target.as_mut_slice()),
        &mut || Ok(()),
    )
    .unwrap();
    let words: Vec<_> = target
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    // The narrow column following the promoted one starts after its full
    // halfword payload, including the last odd row's padding.
    assert_eq!(&words[..6], [6, 8, 71, 16, 200, 8]);
    for (column, values) in relation.columns().enumerate() {
        let first = words[column * 2] as usize;
        let bits = words[column * 2 + 1];
        let lanes = (32 / bits) as usize;
        for (row, id) in values.iter().enumerate() {
            let decoded = (words[first + row / lanes] >> ((row % lanes) * bits as usize))
                & (u32::MAX >> (32 - bits));
            assert_eq!(decoded, id, "column {column}, row {row}");
        }
        let used = relation.row_count() % lanes;
        assert_ne!(used, 0);
        let tail = words[first + relation.row_count() / lanes];
        assert_eq!(tail >> (used * bits as usize), 0);
    }
}

pub(super) fn physical(executor: &mut GpuRelationExecutor) {
    let (owner, catalog) = appended();
    let relation = catalog.view(owner.read()).unwrap();
    assert_eq!(
        relation.columns().map(Column::bits).collect::<Vec<_>>(),
        [8, 16, 8]
    );
    let zero = Value::Number(0);
    let one = Value::Number(1);
    let middle = Value::Number(128);
    let last = Value::Number(256);
    let queries = [
        relation
            .query(
                &[(0, (&zero).into()), (1, (&zero).into()), (2, (&one).into())],
                Limits::default(),
            )
            .unwrap(),
        relation
            .query(&[(1, (&middle).into())], Limits::default())
            .unwrap(),
        relation
            .query(&[(1, (&last).into())], Limits::default())
            .unwrap(),
        relation
            .query(&[(0, (&one).into())], Limits::default())
            .unwrap(),
        relation
            .query(&[(2, (&zero).into())], Limits::default())
            .unwrap(),
        relation.query(&[], Limits::default()).unwrap(),
    ];
    let all = relation.all(Limits::default()).unwrap();
    let mut prepared = executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    let actual = prepared
        .filter(
            &queries,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    for (index, query) in queries.iter().enumerate() {
        let expected = relation.select(query, &all, Limits::default()).unwrap();
        assert_eq!(
            actual
                .selection(index, Limits::default())
                .unwrap()
                .positions(),
            expected.positions()
        );
    }
    assert_eq!(
        actual.selection(0, Limits::default()).unwrap().positions(),
        [0]
    );
    assert_eq!(
        actual.selection(1, Limits::default()).unwrap().positions(),
        [128]
    );
    assert_eq!(
        actual.selection(2, Limits::default()).unwrap().positions(),
        [256]
    );
}
