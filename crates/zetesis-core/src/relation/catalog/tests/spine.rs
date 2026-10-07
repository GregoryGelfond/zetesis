//! Published-path reuse preserves each row transaction and its old extent.

use super::*;
use crate::ordered_index::{Index, Link, position};

fn visit(index: &Index, link: Link, seen: &mut [bool], ordered: &mut Vec<usize>) -> i32 {
    let Some(link) = link else { return 0 };
    let id = position(link);
    assert!(!seen[id], "each published row has one tree occurrence");
    seen[id] = true;
    let node = index.nodes[id];
    let left = visit(index, node.children[0], seen, ordered);
    ordered.push(id);
    let right = visit(index, node.children[1], seen, ordered);
    assert_eq!(i32::from(node.balance), right - left);
    assert!((right - left).abs() <= 1);
    left.max(right) + 1
}

fn validate(fixture: &Fixture) {
    let catalog = &fixture.rows;
    assert_eq!(catalog.rows.nodes.len(), catalog.len());
    let mut seen = vec![false; catalog.len()];
    let mut ordered = Vec::new();
    visit(&catalog.rows, catalog.rows.root, &mut seen, &mut ordered);
    assert!(seen.into_iter().all(|seen| seen));
    let atoms = fixture.source();
    for pair in ordered.windows(2) {
        assert!(
            atoms
                .at(pair[0])
                .unwrap()
                .compare_ref_with(atoms.at(pair[1]).unwrap(), || Ok::<_, ()>(()))
                .unwrap()
                .is_lt()
        );
    }
    assert_eq!(ordered.last().copied(), catalog.last_row);
    if let Some(spine) = catalog.spine {
        assert!(spine.matches(&catalog.rows, catalog.rows.root, catalog.last_row.unwrap()));
        let mut cursor = catalog.rows.root;
        for step in &catalog.rows.path {
            assert_eq!(cursor.map(position), Some(step.id));
            let published = catalog.rows.nodes[step.id];
            assert_eq!(step.node.children, published.children);
            assert_eq!(step.node.balance, published.balance);
            assert!(step.right);
            assert!(!step.changed);
            cursor = published.children[1];
        }
        assert!(
            cursor.is_none(),
            "the certificate covers the whole right spine"
        );
    }
    dictionary::assert_translation(fixture);
}

fn prefix(count: i32) -> Fixture {
    let mut fixture = owner();
    for value in 0..count {
        fixture.insert(&atom(0, value), Limits::default()).unwrap();
    }
    fixture
}

fn columns(fixture: &Fixture) -> Vec<Vec<u32>> {
    fixture
        .view()
        .columns()
        .map(|column| column.iter().collect())
        .collect()
}

fn session_insert(
    fixture: &mut Fixture,
    input: &Atom,
    limits: Limits,
) -> Result<Insertion, CatalogFailure> {
    let canonical = fixture
        .authority
        .entry_atom_with(input, atom_limits(), || Ok::<_, ()>(()))
        .unwrap()
        .insert_ref_with(atom_limits(), || Ok::<_, ()>(()))
        .unwrap();
    let mut append = fixture.rows.appender(limits)?;
    append
        .insert(canonical, limits)
        .map(|appended| appended.insertion)
}

#[test]
fn increasing_rows_reuse_the_published_path() {
    let mut warm = prefix(31);
    let mut cold = prefix(31);
    validate(&warm);
    let depth = warm.rows.rows.path.len();
    let allocation = warm.rows.rows.path.as_ptr();
    assert!(depth > 2);
    cold.rows.spine = None;
    let input = atom(0, 31);
    let warmed = warm.insert(&input, Limits::default()).unwrap();
    let rebuilt = cold.insert(&input, Limits::default()).unwrap();
    // The cold route reads and copies h published nodes. The warm route checks
    // one certificate; balancing, final writes and suffix refresh are identical.
    assert_eq!(
        rebuilt.storage.construction_work,
        warmed.storage.construction_work + 2 * depth as u128 - 1
    );
    assert_eq!(warm.rows.rows.path.as_ptr(), allocation);
    assert_eq!(warm.retained_bytes(), cold.retained_bytes());
    assert_eq!(columns(&warm), columns(&cold));
    validate(&warm);
    validate(&cold);
}

#[test]
fn rotations_preserve_published_spines() {
    let mut warm = owner();
    let mut cold = owner();
    let mut root_rotations = 0;
    let mut retained_depth_rotations = 0;
    for value in 0..127 {
        let root = warm.rows.rows.root;
        let depth = warm.rows.rows.path.len();
        cold.rows.spine = None;
        assert_eq!(
            warm.insert(&atom(0, value), Limits::default()).unwrap().row,
            cold.insert(&atom(0, value), Limits::default()).unwrap().row
        );
        validate(&warm);
        assert!(warm.rows.spine.is_some());
        if value > 0 && root != warm.rows.rows.root {
            root_rotations += 1;
        } else if value > 0 && depth == warm.rows.rows.path.len() {
            retained_depth_rotations += 1;
        }
    }
    assert!(root_rotations > 0);
    assert!(retained_depth_rotations > 0);
    assert_eq!(columns(&warm), columns(&cold));
}

#[test]
fn interior_rows_revoke_the_retained_path() {
    let mut scalar = owner();
    let mut session = owner();
    for value in [0, 8, 16, 24, 32, 40, 48, 56, 64, 3, 19, 9, 40, 72, 72, 80] {
        let input = atom(0, value);
        let interior = scalar.rows.last_row.is_some_and(|id| {
            AtomRef::from(&input)
                .compare_ref_with(scalar.source().at(id).unwrap(), || Ok::<_, ()>(()))
                .unwrap()
                .is_lt()
        });
        let expected = scalar.insert(&input, Limits::default()).unwrap();
        let actual = session_insert(&mut session, &input, Limits::default()).unwrap();
        assert_eq!(
            (actual.row, actual.inserted),
            (expected.row, expected.inserted)
        );
        if expected.inserted && interior {
            assert!(scalar.rows.spine.is_none());
            assert!(session.rows.spine.is_none());
        }
        validate(&scalar);
        validate(&session);
        assert_eq!(columns(&scalar), columns(&session));
    }
}

#[test]
fn every_warm_work_refusal_preserves_the_prefix() {
    let mut complete = prefix(30);
    let input = atom(0, 30);
    let required = session_insert(&mut complete, &input, Limits::default())
        .unwrap()
        .storage
        .construction_work;
    let expected = columns(&complete);
    for max_work in 0..u64::try_from(required).unwrap() {
        let mut fixture = prefix(30);
        let previous = columns(&fixture);
        let failure = session_insert(
            &mut fixture,
            &input,
            Limits {
                max_work,
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            failure.error,
            Failure::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert_eq!(fixture.rows.len(), 30);
        assert_eq!(columns(&fixture), previous);
        assert_eq!(failure.retained_bytes, fixture.retained_bytes());
        assert!(failure.peak_construction_bytes >= failure.retained_bytes);
        validate(&fixture);
        assert_eq!(
            session_insert(&mut fixture, &input, Limits::default())
                .unwrap()
                .row,
            30
        );
        assert_eq!(columns(&fixture), expected);
        validate(&fixture);
    }
}

#[test]
fn warm_insertion_admits_its_actual_peak() {
    let input = atom(0, 30);
    let mut complete = prefix(30);
    let exact = complete
        .insert(&input, Limits::default())
        .unwrap()
        .storage
        .peak_construction_bytes;
    let mut accepted = prefix(30);
    accepted
        .insert(
            &input,
            Limits {
                max_bytes: exact,
                ..Limits::default()
            },
        )
        .unwrap();
    let mut refused = prefix(30);
    let failure = refused
        .insert(
            &input,
            Limits {
                max_bytes: exact - 1,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Limit {
            resource: Resource::Bytes,
            ..
        }
    ));
    assert_eq!(refused.rows.len(), 30);
    assert_eq!(failure.retained_bytes, refused.retained_bytes());
    validate(&refused);
    refused.insert(&input, Limits::default()).unwrap();
    assert_eq!(columns(&refused), columns(&accepted));
    validate(&refused);
}

#[test]
fn read_only_access_preserves_spine_authority() {
    let mut fixture = prefix(31);
    let allocation = fixture.rows.rows.path.as_ptr();
    let depth = fixture.rows.rows.path.len();
    fixture.lookup(&atom(0, 12), Limits::default()).unwrap();
    fixture.prepare_ordered(Limits::default()).unwrap();
    assert_eq!(ids(&fixture), (0..31).collect::<Vec<_>>());
    assert!(
        !fixture
            .insert(&atom(0, 30), Limits::default())
            .unwrap()
            .inserted
    );
    assert!(fixture.rows.spine.is_some());
    assert_eq!(fixture.rows.rows.path.as_ptr(), allocation);
    assert_eq!(fixture.rows.rows.path.len(), depth);
    validate(&fixture);
}

#[test]
fn foreign_rows_preserve_the_published_spine() {
    let mut fixture = prefix(31);
    let foreign = prefix(32);
    let failure = fixture
        .rows
        .insert(foreign.authority.get(31).unwrap(), Limits::default())
        .unwrap_err();
    assert!(matches!(
        failure.error,
        Failure::Read(ReadError::ForeignCatalog)
    ));
    validate(&fixture);
    assert_eq!(fixture.rows.len(), 31);
    assert!(fixture.rows.spine.is_some());
}

#[test]
fn clear_revokes_spine_authority() {
    let mut fixture = prefix(31);
    fixture.clear(Limits::default()).unwrap();
    assert!(fixture.rows.spine.is_none());
    fixture.insert(&atom(0, 5), Limits::default()).unwrap();
    assert_eq!(fixture.rows.len(), 1);
    validate(&fixture);
}
