//! A successful retry publishes the complete documented sorted-run structure.

use zetesis_core::atom_interner::{AtomInterner, Limits as AtomLimits};
use zetesis_core::catalog::AtomRef;
use zetesis_core::relation::{
    Canonical, Catalog as Rows, CatalogFailure, Failure, Insertion, Limits, Lookup, Preparation,
    Resource, Runs, Storage,
};
use zetesis_core::{Atom, Predicate, Value};

// Each fixture has one explicit canonical authority; the measured operations
// below cover only its metadata relation and ordered-run preparation.
struct Catalog {
    authority: AtomInterner,
    rows: Rows,
}

fn atom_limits() -> AtomLimits {
    AtomLimits::for_atoms(64, 1024 * 1024)
}

impl Catalog {
    fn new(predicate: &Predicate, limits: Limits) -> Result<Self, CatalogFailure> {
        let mut authority = AtomInterner::default();
        let declared = authority
            .declare_predicate_with(predicate, atom_limits(), || Ok::<_, ()>(()))
            .unwrap();
        let rows = Rows::new(authority.read(), declared, limits)?;
        Ok(Self { authority, rows })
    }

    fn insert(&mut self, atom: &Atom, limits: Limits) -> Result<Insertion, CatalogFailure> {
        let atom = self
            .authority
            .entry_atom_with(atom, atom_limits(), || Ok::<_, ()>(()))
            .unwrap()
            .insert_ref_with(atom_limits(), || Ok::<_, ()>(()))
            .unwrap();
        self.rows.insert(atom, limits)
    }

    fn atoms(&self) -> Vec<AtomRef<'_>> {
        self.rows
            .atoms(self.authority.read())
            .unwrap()
            .iter()
            .collect()
    }

    fn lookup(&self, atom: &Atom, limits: Limits) -> Result<Lookup, CatalogFailure> {
        self.rows.lookup(self.authority.read(), atom.into(), limits)
    }

    fn canonical(&self, limits: Limits) -> Result<Canonical, CatalogFailure> {
        self.rows.canonical(self.authority.read(), limits)
    }

    fn prepare_ordered(&mut self, limits: Limits) -> Result<Preparation<'_>, CatalogFailure> {
        self.rows.prepare_ordered(self.authority.read(), limits)
    }

    fn ordered(&self) -> Option<Runs<'_>> {
        self.rows.ordered()
    }
}

fn atom(value: i32) -> Atom {
    Atom::new(
        Predicate::new("row", 1).unwrap(),
        vec![Value::Number(value)],
    )
    .unwrap()
}

/// Two prepared two-row runs, followed by one append. The next preparation
/// must compact the two old runs before publishing its new one-row tail.
fn before_third_preparation() -> (Catalog, Storage) {
    let mut catalog = Catalog::new(&Predicate::new("row", 1).unwrap(), Limits::default()).unwrap();
    for values in [[4, 2], [3, 1]] {
        for value in values {
            catalog.insert(&atom(value), Limits::default()).unwrap();
        }
        catalog.prepare_ordered(Limits::default()).unwrap();
    }
    let runs = catalog.ordered().unwrap();
    assert_eq!(runs.levels().iter().map(Vec::len).collect::<Vec<_>>(), [2]);
    assert_eq!(runs.tail().len(), 2);
    let insertion = catalog.insert(&atom(0), Limits::default()).unwrap();
    assert_eq!(insertion.row, 4);
    (catalog, insertion.storage)
}

fn assert_prepared_contract(catalog: &Catalog, refused_limit: Limits) {
    // Preparation and its refusal may rearrange only the ordered row-ID
    // view. Stable insertion identities and canonical order remain exact.
    assert_eq!(
        catalog.atoms(),
        [atom(4), atom(2), atom(3), atom(1), atom(0)]
    );
    for (row, value) in [4, 2, 3, 1, 0].into_iter().enumerate() {
        assert_eq!(
            catalog.lookup(&atom(value), Limits::default()).unwrap().row,
            Some(row)
        );
    }
    assert_eq!(
        catalog.canonical(Limits::default()).unwrap().ids,
        [4, 3, 1, 2, 0]
    );
    let runs = catalog.ordered().unwrap();
    assert_eq!(runs.len(), 5);
    assert_eq!(runs.tail(), [4]);
    // Runs::levels publicly promises that each level is less than half the
    // preceding level. This is a representation contract, not a timing claim.
    let lengths: Vec<_> = runs.levels().iter().map(Vec::len).collect();
    assert!(
        lengths.windows(2).all(|pair| pair[1] * 2 < pair[0]),
        "successful retry published non-geometric levels {lengths:?} after {refused_limit:?}",
    );
}

#[test]
fn work_refusal_retry_restores_the_public_run_contract() {
    let (mut reference, _) = before_third_preparation();
    let required = reference
        .prepare_ordered(Limits::default())
        .unwrap()
        .storage
        .construction_work;
    assert!(
        required > 1 && required < 128,
        "bounded five-row fixture: {required}"
    );
    for max_work in 0..u64::try_from(required).unwrap() {
        let (mut catalog, _) = before_third_preparation();
        let limits = Limits {
            max_work,
            ..Limits::default()
        };
        let Err(failure) = catalog.prepare_ordered(limits) else {
            panic!("insufficient work must refuse: {limits:?}");
        };
        assert!(matches!(
            failure.error,
            Failure::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(failure.work <= u128::from(max_work));
        assert!(catalog.ordered().is_none());
        catalog.prepare_ordered(Limits::default()).unwrap();
        assert_prepared_contract(&catalog, limits);
    }
}

#[test]
fn byte_refusal_retry_restores_the_public_run_contract() {
    let (mut reference, initial) = before_third_preparation();
    let required = reference
        .prepare_ordered(Limits::default())
        .unwrap()
        .storage
        .peak_construction_bytes;
    assert!(required > initial.retained_bytes);
    assert!(
        required - initial.retained_bytes < 4096,
        "bounded five-row fixture"
    );
    for max_bytes in initial.retained_bytes..required {
        let (mut catalog, _) = before_third_preparation();
        let limits = Limits {
            max_bytes,
            ..Limits::default()
        };
        let Err(failure) = catalog.prepare_ordered(limits) else {
            panic!("insufficient bytes must refuse: {limits:?}");
        };
        assert!(matches!(
            failure.error,
            Failure::Limit {
                resource: Resource::Bytes,
                ..
            }
        ));
        assert!(catalog.ordered().is_none());
        catalog.prepare_ordered(Limits::default()).unwrap();
        assert_prepared_contract(&catalog, limits);
    }
}

/// The next preparation must merge 3+2, then 8+5; a work-prefix sweep
/// therefore includes refusals after one merge has already committed.
fn before_two_merges() -> Catalog {
    let mut catalog = Catalog::new(&Predicate::new("row", 1).unwrap(), Limits::default()).unwrap();
    for values in [&[13, 12, 11, 10, 9, 8, 7, 6][..], &[5, 4, 3], &[2, 1]] {
        for &value in values {
            catalog.insert(&atom(value), Limits::default()).unwrap();
        }
        catalog.prepare_ordered(Limits::default()).unwrap();
    }
    let runs = catalog.ordered().unwrap();
    assert_eq!(
        runs.levels().iter().map(Vec::len).collect::<Vec<_>>(),
        [8, 3]
    );
    assert_eq!(runs.tail().len(), 2);
    catalog.insert(&atom(0), Limits::default()).unwrap();
    catalog
}

#[test]
fn repeated_refusals_preserve_committed_merge_progress() {
    let mut reference = before_two_merges();
    let required = reference
        .prepare_ordered(Limits::default())
        .unwrap()
        .storage
        .construction_work;
    assert!(
        required > 1 && required < 256,
        "bounded fourteen-row fixture: {required}"
    );
    for max_work in 0..u64::try_from(required).unwrap() {
        let mut catalog = before_two_merges();
        let limits = Limits {
            max_work,
            ..Limits::default()
        };
        for _ in 0..2 {
            match catalog.prepare_ordered(limits) {
                Ok(_) => break,
                Err(failure) => {
                    assert!(matches!(
                        failure.error,
                        Failure::Limit {
                            resource: Resource::Work,
                            ..
                        }
                    ));
                    assert!(catalog.ordered().is_none());
                }
            }
        }
        catalog.prepare_ordered(Limits::default()).unwrap();
        let runs = catalog.ordered().unwrap();
        assert_eq!(runs.len(), 14);
        assert_eq!(runs.tail(), [13]);
        let lengths: Vec<_> = runs.levels().iter().map(Vec::len).collect();
        assert!(
            lengths.windows(2).all(|pair| pair[1] * 2 < pair[0]),
            "retry after {limits:?} published {lengths:?}",
        );
        assert_eq!(
            catalog.canonical(Limits::default()).unwrap().ids,
            (0..14).rev().collect::<Vec<_>>(),
        );
        for (row, value) in (0..14).rev().enumerate() {
            assert_eq!(
                catalog.lookup(&atom(value), Limits::default()).unwrap().row,
                Some(row)
            );
        }
    }
}
