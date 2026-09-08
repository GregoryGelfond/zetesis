//! Independent finite interpretations for private membership preparation.

use zetesis_core::{AdmissionLimits, AtomPattern, Sign};

use super::*;
use crate::Control;

fn snapshot(
    program: &Program,
    catalog: &BTreeMap<Atom, u32>,
    snapshots: &[u32],
    stride: usize,
    worlds: usize,
    available: usize,
    work: &mut Work<'_>,
) -> Result<Snapshot, Stop> {
    Workspace::new(program, worlds, available, work)?
        .snapshot(catalog, snapshots, stride, available, work)
}

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

fn assert_conjunctions(
    program: &Program,
    catalog: &BTreeMap<Atom, u32>,
    input: &[u32],
    snapshot: &mut Snapshot,
    work: &mut Work<'_>,
) {
    let (atoms, mut join) = snapshot.parts();
    join.reset(&program.templates()[0], work).unwrap();
    let mut selected = Vec::new();
    for (depth, name) in ["a", "b", "c"].into_iter().enumerate() {
        if !atoms.iter().any(|atom| atom.predicate().name() == name) {
            break;
        }
        selected.push(catalog[&atom(name)]);
        let expected = input
            .iter()
            .any(|world| selected.iter().all(|id| world & (1 << id) != 0));
        let actual = join.extend(depth, 0, work).unwrap();
        assert_eq!(actual, expected, "worlds={input:?}, depth={depth}");
        if !actual {
            break;
        }
    }
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
            snapshot(&program, &catalog, &input, 1, 3, usize::MAX, &mut work).unwrap();
        assert_conjunctions(&program, &catalog, &input, &mut snapshot, &mut work);
    }
}

#[test]
fn reused_join_storage_ignores_previous_truth() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    let mut workspace = Workspace::new(&program, 3, usize::MAX, &mut work).unwrap();
    // Truth is allowed to shrink here too: reuse must depend solely on the
    // supplied round, not on the monotonicity of normal closure execution.
    for assignment in (0u32..512).rev() {
        let input = [assignment & 7, (assignment >> 3) & 7, (assignment >> 6) & 7];
        workspace
            .frames
            .fill(if assignment % 2 == 0 { 0 } else { u32::MAX });
        workspace.starts.fill(Some(usize::MAX));
        let mut snapshot = workspace
            .snapshot(&catalog, &input, 1, usize::MAX, &mut work)
            .unwrap();
        assert_conjunctions(&program, &catalog, &input, &mut snapshot, &mut work);
        workspace = snapshot.into_workspace();
    }
}

#[test]
fn successive_snapshots_retain_join_allocations() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    let mut workspace = Workspace::new(&program, 65, usize::MAX, &mut work).unwrap();
    let frames = (workspace.frames.as_ptr(), workspace.frames.capacity());
    let starts = (workspace.starts.as_ptr(), workspace.starts.capacity());
    for truth in [0, 7, 1, 0, 6, 7] {
        let snapshot = workspace
            .snapshot(&catalog, &[truth; 65], 1, usize::MAX, &mut work)
            .unwrap();
        workspace = snapshot.into_workspace();
        assert_eq!(
            (workspace.frames.as_ptr(), workspace.frames.capacity()),
            frames
        );
        assert_eq!(
            (workspace.starts.as_ptr(), workspace.starts.capacity()),
            starts
        );
    }
}

#[test]
fn reused_join_storage_skips_initialization_work() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut first = Work::source(&control, u64::MAX);
    let original = snapshot(&program, &catalog, &[7; 33], 1, 33, usize::MAX, &mut first).unwrap();
    let mut repeated = Work::source(&control, u64::MAX);
    original
        .into_workspace()
        .snapshot(&catalog, &[7; 33], 1, usize::MAX, &mut repeated)
        .unwrap();
    // Three positive rows require four two-word frames, three indices and one
    // template-shape visit. Membership is freshly rebuilt in both invocations.
    assert_eq!(first.statistics.work - repeated.statistics.work, 8 + 3 + 1);
    assert_eq!(first.mask_words - repeated.mask_words, 8);
    assert_eq!(first.mask_bytes, repeated.mask_bytes);
}

#[test]
fn reused_join_storage_remains_in_the_live_budget() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    let workspace = Workspace::new(&program, 3, usize::MAX, &mut work).unwrap();
    let retained = workspace.bytes();
    let exact = retained + 3 * size_of::<u32>();
    let snapshot = workspace
        .snapshot(&catalog, &[7; 3], 1, exact, &mut work)
        .unwrap();
    assert_eq!(snapshot.bytes(), exact);
    let mut next = Work::source(&control, u64::MAX);
    let refused = snapshot
        .into_workspace()
        .snapshot(&catalog, &[7; 3], 1, exact - 1, &mut next);
    assert!(matches!(refused, Err(Stop::Allocation)));
    assert_eq!(next.mask_bytes, retained);
}

#[test]
fn pairwise_overlap_does_not_establish_a_common_world() {
    let program = program();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    // a={0,2}, b={1,2}, c={0,1}: each pair intersects, but all three do not.
    let mut snapshot = snapshot(
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
    let old = snapshot(&program, &catalog, &[4, 1], 1, 2, usize::MAX, &mut old_work).unwrap();
    let mut new_work = Work::source(&control, u64::MAX);
    let fresh = snapshot(&program, &catalog, &[4, 5], 1, 2, usize::MAX, &mut new_work).unwrap();
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
    let mut snapshot = snapshot(&program, &catalog, &[1, 2], 1, 2, usize::MAX, &mut work).unwrap();
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
        let mut snapshot = snapshot(
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
            snapshot(
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
        snapshot(&program, &catalog, &[0], 1, 1, usize::MAX, &mut work),
        Err(Stop::InvalidProgram)
    ));
}

#[test]
fn mask_preflight_refuses_before_payload_allocation() {
    let program = program();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    assert!(matches!(
        snapshot(&program, &catalog(), &[7], 1, 1, 0, &mut work),
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
        snapshot(&program, &catalog(), &[7], 1, 1, usize::MAX, &mut work),
        Err(Stop::Cancelled)
    ));
    assert_eq!(work.mask_bytes, 0);
}
