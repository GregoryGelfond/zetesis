//! The inverse translates canonical identity without changing local coordinates.

use super::*;
use crate::catalog::{DerivedTerms, Limits as TermLimits};
use crate::relation::{DictionaryIndex, QueryFailure, Work, dictionary::Probe};
use crate::{AdmissionLimits, AtomPattern, Program, Template, Term};

fn lookup(relation: &Relation<'_>, value: TermRef<'_>) -> (Option<u32>, u128) {
    let attempt = relation.query_attempt(&[(0, value)], Limits::default());
    let query = attempt.result.unwrap();
    let equality = query.equalities().first().map(|value| value.value_id());
    assert_eq!(query.is_possible(), equality.is_some());
    (equality, attempt.work)
}

pub(super) fn inverse_bytes(fixture: &Fixture) -> usize {
    let DictionaryIndex::Append(index) = &fixture.rows.layout.index else {
        panic!("append fixture");
    };
    index.identities.bytes()
}

/// Includes authority-only rows retained after a failed relation insertion.
/// The oracle reconstructs dictionary denotation from its published source cells.
pub(super) fn assert_translation(fixture: &Fixture) {
    let view = fixture.view();
    let atoms = fixture.source();
    for atom in (0..fixture.authority.len()).map(|row| fixture.authority.get(row).unwrap()) {
        for value in atom.values() {
            let expected = fixture
                .rows
                .layout
                .dictionary
                .iter()
                .position(|cell| {
                    atoms
                        .at(cell.row)
                        .unwrap()
                        .values()
                        .at(cell.column)
                        .unwrap()
                        == value
                })
                .map(|id| u32::try_from(id).unwrap());
            assert_eq!(lookup(&view, value).0, expected);
        }
    }
}

#[test]
fn repeated_new_terms_share_one_equality_coordinate() {
    let predicate = Predicate::new("quad", 4).unwrap();
    let mut fixture = Fixture::new(&predicate, Limits::default()).unwrap();
    // Canonical allocation order is deliberately different from dictionary order.
    let ingress = Atom::new(predicate.clone(), [1, 9, 3, 1].map(Value::Number).to_vec()).unwrap();
    fixture
        .authority
        .entry_atom_with(&ingress, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    for values in [[9, 9, 1, 9], [1, 3, 3, 9]] {
        fixture
            .insert(
                &Atom::new(predicate.clone(), values.map(Value::Number).to_vec()).unwrap(),
                Limits::default(),
            )
            .unwrap();
    }
    let columns: Vec<Vec<u32>> = fixture
        .view()
        .columns()
        .map(|column| column.iter().collect())
        .collect();
    assert_eq!(columns, [vec![0, 1], vec![0, 2], vec![1, 2], vec![0, 0]]);
    assert_eq!(fixture.rows.layout.dictionary.len(), 3);
    assert_translation(&fixture);
}

#[test]
fn canonical_misses_do_not_scan_common_payload_prefixes() {
    let mut fixture = owner();
    let long = "shared".repeat(2048);
    let present = Atom::new(
        Predicate::new("pair", 2).unwrap(),
        vec![Value::String(format!("{long}a")), Value::Number(0)],
    )
    .unwrap();
    let missing = Atom::new(
        Predicate::new("pair", 2).unwrap(),
        vec![Value::String(format!("{long}b")), Value::Number(1)],
    )
    .unwrap();
    fixture.insert(&present, Limits::default()).unwrap();
    fixture
        .authority
        .entry_atom_with(&missing, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let canonical = fixture.authority.get(1).unwrap();
    let view = fixture.view();
    let text = lookup(&view, canonical.values().at(0).unwrap());
    let number = lookup(&view, canonical.values().at(1).unwrap());
    assert_eq!(text, number);
    assert_eq!(text.0, None);
    let foreign = lookup(&view, (&missing.values()[0]).into());
    assert_eq!(foreign.0, None);
    assert!(foreign.1 > text.1 + 1000);
}

#[test]
fn foreign_ids_do_not_determine_dictionary_membership() {
    let mut local = owner();
    local.insert(&atom(9, 3), Limits::default()).unwrap();
    let mut foreign = owner();
    foreign.insert(&atom(40, 9), Limits::default()).unwrap();
    let local_term = local.authority.get(0).unwrap().values().at(0).unwrap();
    let foreign_atom = foreign.authority.get(0).unwrap();
    let different = foreign_atom.values().at(0).unwrap();
    let equal = foreign_atom.values().at(1).unwrap();
    assert_eq!(
        local.authority.read().selected_term(local_term).unwrap(),
        foreign.authority.read().selected_term(different).unwrap()
    );
    assert_ne!(
        local.authority.read().selected_term(local_term).unwrap(),
        foreign.authority.read().selected_term(equal).unwrap()
    );
    let view = local.view();
    assert_eq!(lookup(&view, different).0, None);
    assert_eq!(lookup(&view, equal).0, Some(0));
}

#[test]
fn independent_tuple_writers_share_term_translation() {
    let pattern = AtomPattern::new(
        Predicate::new("pair", 2).unwrap(),
        vec![
            Term::Constant(Value::Number(9)),
            Term::Constant(Value::Number(3)),
        ],
    )
    .unwrap();
    let program = Program::new(
        vec![Template::new(Some(pattern), vec![], vec![], vec![], vec![])],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut left = AtomInterner::for_program(&program, 1 << 20).unwrap();
    let mut right = AtomInterner::for_program(&program, 1 << 20).unwrap();
    let declared = left
        .declare_predicate_with(&Predicate::new("pair", 2).unwrap(), atom_limits(), || {
            Ok::<_, ()>(())
        })
        .unwrap();
    let mut rows = Catalog::new(left.read(), declared, Limits::default()).unwrap();
    let ingress = atom(9, 3);
    left.entry_atom_with(&ingress, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    right
        .entry_atom_with(&ingress, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let local = left.get(0).unwrap();
    let other = right.get(0).unwrap();
    rows.insert(local, Limits::default()).unwrap();
    let view = rows.view(left.read()).unwrap();
    assert_eq!(
        lookup(&view, local.values().at(0).unwrap()),
        lookup(&view, other.values().at(0).unwrap())
    );
    let failure = rows.insert(other, Limits::default()).unwrap_err();
    assert_eq!(failure.error, Failure::Read(ReadError::ForeignCatalog));
}

#[test]
fn newer_query_terms_keep_semantic_fallback() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    fixture
        .authority
        .commit_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let (prefix, mut append) = fixture.authority.split();
    let ingress = atom(40, 3);
    let newer = append
        .entry_atom_with(&ingress, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_ref_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let term = newer.values().at(0).unwrap();
    let DictionaryIndex::Append(index) = &fixture.rows.layout.index else {
        panic!("append fixture")
    };
    assert!(matches!(
        index
            .identities
            .probe_with(prefix.read(), term, &mut || Ok::<_, ()>(()))
            .unwrap(),
        Probe::Unavailable(ReadError::OutsidePrefix)
    ));
    let view = fixture.rows.view(prefix.read()).unwrap();
    assert_eq!(lookup(&view, term).0, None);
}

#[test]
fn derived_aliases_keep_semantic_fallback() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    let value = fixture.authority.get(0).unwrap().values().at(0).unwrap();
    let mut arena =
        DerivedTerms::new_with(&[fixture.authority.read()], 1 << 20, || Ok::<_, ()>(())).unwrap();
    let key = arena.borrow_with(value, || Ok::<_, ()>(())).unwrap();
    let absent = arena
        .scalar_with(
            crate::ValueNodeRef::Number(40),
            TermLimits::default(),
            || Ok::<_, ()>(()),
        )
        .unwrap();
    let derived = arena.read().term(&key).unwrap();
    let DictionaryIndex::Append(index) = &fixture.rows.layout.index else {
        panic!("append fixture")
    };
    assert!(matches!(
        index
            .identities
            .probe_with(fixture.authority.read(), derived, &mut || Ok::<_, ()>(()))
            .unwrap(),
        Probe::Unavailable(ReadError::ForeignCatalog)
    ));
    let view = fixture.view();
    assert_eq!(lookup(&view, derived).0, Some(0));
    assert_eq!(lookup(&view, arena.read().term(&absent).unwrap()).0, None);
}

#[test]
fn clear_invalidates_prior_canonical_translations() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    let capacity = inverse_bytes(&fixture);
    fixture.clear(Limits::default()).unwrap();
    assert_eq!(inverse_bytes(&fixture), capacity);
    assert_translation(&fixture);
    fixture.insert(&atom(3, 9), Limits::default()).unwrap();
    assert_translation(&fixture);
    assert_eq!(
        lookup(
            &fixture.view(),
            fixture.authority.get(0).unwrap().values().at(0).unwrap()
        )
        .0,
        Some(1)
    );
}

#[test]
fn clear_admits_retained_hash_capacity_before_reset() {
    let mut fixture = owner();
    for value in 0..64 {
        fixture
            .insert(&atom(value, value), Limits::default())
            .unwrap();
    }
    let first = fixture.clear(Limits::default()).unwrap();
    let capacity = inverse_bytes(&fixture) / size_of::<(crate::catalog::storage::TermId, u32)>();
    assert!(first.construction_work >= capacity as u128);
    fixture.insert(&atom(9, 9), Limits::default()).unwrap();
    let failure = fixture
        .clear(Limits {
            max_work: u64::try_from(first.construction_work - 1).unwrap(),
            ..Limits::default()
        })
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Limit {
            resource: Resource::Work,
            ..
        }
    ));
    assert_translation(&fixture);
    assert_eq!(fixture.atoms(), [atom(9, 9)]);
    let second = fixture
        .clear(Limits {
            max_work: u64::try_from(first.construction_work).unwrap(),
            ..Limits::default()
        })
        .unwrap();
    assert_eq!(second.construction_work, first.construction_work);
    assert_translation(&fixture);
}

#[test]
fn hash_growth_refusal_reports_actual_replacement_capacity() {
    let mut reference = owner();
    reference.insert(&atom(9, 3), Limits::default()).unwrap();
    let base = reference.retained_bytes();
    let previous = inverse_bytes(&reference);
    let mut work = Work::new(Limits::default(), base as u128).unwrap();
    let DictionaryIndex::Append(index) = &mut reference.rows.layout.index else {
        panic!("append fixture")
    };
    // Reserve through the real transaction capability, without publishing keys.
    // This request exercises HashMap's rounded replacement capacity.
    index.identities.reserve(129, &mut work).unwrap();
    let replacement = index.identities.bytes();
    assert!(replacement > 131 * size_of::<(crate::catalog::storage::TermId, u32)>());
    assert_eq!(work.peak, base + replacement);
    assert_eq!(work.live, base - previous + replacement);
    let peak = work.peak;
    let mut short = owner();
    short.insert(&atom(9, 3), Limits::default()).unwrap();
    let mut work = Work::new(
        Limits {
            max_bytes: peak - 1,
            ..Limits::default()
        },
        short.retained_bytes() as u128,
    )
    .unwrap();
    let DictionaryIndex::Append(index) = &mut short.rows.layout.index else {
        panic!("append fixture")
    };
    assert_eq!(
        index.identities.reserve(129, &mut work),
        Err(Failure::Limit {
            resource: Resource::Bytes,
            observed: peak as u128,
            limit: (peak - 1) as u128,
        })
    );
    assert_eq!(inverse_bytes(&short), replacement);
    assert_eq!(work.peak, peak);
    assert_eq!(work.live, short.retained_bytes());
    assert_translation(&short);
}

#[test]
fn canonical_query_refusal_publishes_no_partial_result() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    let canonical = fixture.authority.get(0).unwrap();
    let keys = [
        (0, canonical.values().at(0).unwrap()),
        (1, canonical.values().at(1).unwrap()),
    ];
    let view = fixture.view();
    let complete = view.query_attempt(&keys, Limits::default());
    let expected = complete.result.unwrap();
    for cut in 0..complete.work {
        let mut accepted = 0;
        let attempt = view.query_attempt_with(&keys, Limits::default(), || {
            if accepted == cut {
                return Err(cut);
            }
            accepted += 1;
            Ok(())
        });
        assert!(matches!(attempt.result, Err(QueryFailure::Stopped(actual)) if actual == cut));
        assert_eq!(attempt.work, cut);
        assert_eq!(accepted, cut);
        assert_eq!(
            view.query(&keys, Limits::default()).unwrap().equalities(),
            expected.equalities()
        );
    }
}

#[test]
fn canonical_query_unwind_preserves_the_dictionary() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    let term = fixture.authority.get(0).unwrap().values().at(0).unwrap();
    let view = fixture.view();
    let work = lookup(&view, term).1;
    for cut in 0..work {
        let mut accepted = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            view.query_attempt_with(&[(0, term)], Limits::default(), || {
                assert_ne!(accepted, cut, "injected query panic");
                accepted += 1;
                Ok::<_, ()>(())
            })
        }));
        assert!(panic.is_err());
        assert_eq!(accepted, cut);
        assert_translation(&fixture);
    }
}

#[test]
fn canonical_absence_does_not_hide_an_invalid_column() {
    let mut fixture = owner();
    fixture.insert(&atom(9, 3), Limits::default()).unwrap();
    let ingress = atom(40, 3);
    fixture
        .authority
        .entry_atom_with(&ingress, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let term = fixture.authority.get(1).unwrap().values().at(0).unwrap();
    let view = fixture.view();
    assert_eq!(
        view.query_attempt(&[(0, term), (2, term)], Limits::default())
            .result
            .err(),
        Some(Failure::Column)
    );
}
