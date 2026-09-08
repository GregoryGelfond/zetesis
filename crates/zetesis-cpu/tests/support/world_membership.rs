//! Independent finite interpretations for private membership preparation.

use zetesis_core::{AdmissionLimits, AtomPattern, Sign};

use super::*;
use crate::Control;

fn predicate(name: &str) -> Predicate {
    Predicate::new(name, 0).unwrap()
}

fn atom(name: &str) -> Atom {
    Atom::new(predicate(name), vec![]).unwrap()
}

fn template(names: &[&str]) -> Template {
    Template::new(
        None,
        names
            .iter()
            .map(|name| AtomPattern::new(predicate(name), vec![]).unwrap())
            .collect(),
        vec![],
        vec![],
        vec![],
    )
}

fn program() -> Program {
    Program::new(vec![template(&["a", "b", "c"])], AdmissionLimits::default()).unwrap()
}

fn catalog() -> BTreeMap<Atom, u32> {
    // Canonical source order differs deliberately from demanded ID order.
    [(atom("a"), 2), (atom("b"), 0), (atom("c"), 1)]
        .into_iter()
        .collect()
}

#[test]
fn masks_equal_independent_world_conjunctions() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    // Every assignment of three atoms to three worlds, including empty rows.
    for assignment in 0u32..512 {
        let input = [assignment & 7, (assignment >> 3) & 7, (assignment >> 6) & 7];
        let mut work = Work::source(&control, u64::MAX);
        let mut snapshot =
            Snapshot::new(&program, &catalog, &input, 1, 3, usize::MAX, &mut work).unwrap();
        let (atoms, mut join) = snapshot.parts();
        join.reset(&program.templates()[0], &mut work).unwrap();
        let mut selected = Vec::new();
        for (depth, name) in ["a", "b", "c"].into_iter().enumerate() {
            if !atoms.iter().any(|atom| atom.predicate().name() == name) {
                break;
            }
            selected.push(catalog[&atom(name)]);
            let expected = input
                .iter()
                .any(|world| selected.iter().all(|id| world & (1 << id) != 0));
            let actual = join.extend(depth, 0, &mut work).unwrap();
            assert_eq!(actual, expected, "assignment={assignment}, depth={depth}");
            if !actual {
                break;
            }
        }
    }
}

#[test]
fn pairwise_overlap_does_not_establish_a_common_world() {
    let program = program();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    // a={0,2}, b={1,2}, c={0,1}: each pair intersects, but all three do not.
    let mut snapshot = Snapshot::new(
        &program,
        &catalog(),
        &[6, 3, 5],
        1,
        3,
        usize::MAX,
        &mut work,
    )
    .unwrap();
    let (_, mut join) = snapshot.parts();
    join.reset(&program.templates()[0], &mut work).unwrap();
    assert!(join.extend(0, 0, &mut work).unwrap());
    assert!(join.extend(1, 0, &mut work).unwrap());
    assert!(!join.extend(2, 0, &mut work).unwrap());
    assert_eq!(work.pruned_prefixes, 1);
}

#[test]
fn catalog_identity_does_not_make_an_old_mask_fresh() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut old_work = Work::source(&control, u64::MAX);
    let old = Snapshot::new(&program, &catalog, &[4, 1], 1, 2, usize::MAX, &mut old_work).unwrap();
    let mut new_work = Work::source(&control, u64::MAX);
    let fresh =
        Snapshot::new(&program, &catalog, &[4, 5], 1, 2, usize::MAX, &mut new_work).unwrap();
    assert_eq!(old.atoms, fresh.atoms);
    assert_eq!(old.membership, [1, 2]);
    assert_eq!(fresh.membership, [3, 2]);
    // Negative control: reusing the old matrix would erase world 1's new a.
    assert_ne!(
        old.membership[0] & old.membership[1],
        fresh.membership[0] & fresh.membership[1]
    );
}

#[test]
fn predicate_signs_have_distinct_membership_rows() {
    let positive = predicate("p");
    let negative = Predicate::with_sign("p", 0, Sign::Negative).unwrap();
    let catalog = [
        (Atom::new(positive.clone(), vec![]).unwrap(), 1),
        (Atom::new(negative.clone(), vec![]).unwrap(), 0),
    ]
    .into_iter()
    .collect();
    let rule = Template::new(
        None,
        vec![
            AtomPattern::new(negative, vec![]).unwrap(),
            AtomPattern::new(positive, vec![]).unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    );
    let program = Program::new(vec![rule], AdmissionLimits::default()).unwrap();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    let mut snapshot =
        Snapshot::new(&program, &catalog, &[1, 2], 1, 2, usize::MAX, &mut work).unwrap();
    let (_, mut join) = snapshot.parts();
    join.reset(&program.templates()[0], &mut work).unwrap();
    assert!(join.extend(0, 0, &mut work).unwrap());
    assert!(!join.extend(1, 0, &mut work).unwrap());
}

#[test]
fn unused_world_tail_bits_are_never_members() {
    let program = program();
    let control = Control::default();
    for count in [31usize, 32, 33, 63, 64, 65] {
        let mut work = Work::source(&control, u64::MAX);
        let mut snapshot = Snapshot::new(
            &program,
            &catalog(),
            &vec![7; count],
            1,
            count,
            usize::MAX,
            &mut work,
        )
        .unwrap();
        let (_, mut join) = snapshot.parts();
        join.reset(&program.templates()[0], &mut work).unwrap();
        for bit in 0..join.words * 32 {
            assert_eq!(join.frames[bit / 32] & (1 << (bit % 32)) != 0, bit < count);
        }
    }
}

#[test]
fn malformed_snapshot_dimensions_are_refused() {
    let program = program();
    let control = Control::default();
    for (input, stride, worlds) in [
        (&[][..], 1, 0),
        (&[0][..], 0, 1),
        (&[0][..], 1, 2),
        (&[0][..], usize::MAX, 2),
    ] {
        let mut work = Work::source(&control, u64::MAX);
        assert!(matches!(
            Snapshot::new(
                &program,
                &catalog(),
                input,
                stride,
                worlds,
                usize::MAX,
                &mut work
            ),
            Err(Stop::InvalidProgram)
        ));
    }
}

#[test]
fn catalog_ids_must_fit_the_snapshot_stride() {
    let program = program();
    let catalog = [(atom("a"), 32)].into_iter().collect();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    assert!(matches!(
        Snapshot::new(&program, &catalog, &[0], 1, 1, usize::MAX, &mut work),
        Err(Stop::InvalidProgram)
    ));
}

#[test]
fn mask_preflight_refuses_before_payload_allocation() {
    let program = program();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    assert!(matches!(
        Snapshot::new(&program, &catalog(), &[7], 1, 1, 0, &mut work),
        Err(Stop::Allocation)
    ));
    assert_eq!(work.mask_bytes, 0);
}

#[test]
fn membership_preparation_observes_cancellation() {
    let program = program();
    let control = Control::default();
    control.cancel();
    let mut work = Work::source(&control, u64::MAX);
    assert!(matches!(
        Snapshot::new(&program, &catalog(), &[7], 1, 1, usize::MAX, &mut work),
        Err(Stop::Cancelled)
    ));
    assert_eq!(work.mask_bytes, 0);
}
