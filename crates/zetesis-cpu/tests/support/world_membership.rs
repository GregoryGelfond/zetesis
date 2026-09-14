//! Independent finite interpretations for private membership preparation.

use zetesis_core::{AdmissionLimits, AtomPattern, Sign, Value};

use super::*;
use crate::Control;

struct Catalog {
    atoms: Vec<Atom>,
    positions: Vec<usize>,
}

impl Catalog {
    fn new(atoms: Vec<Atom>) -> Self {
        let mut positions: Vec<_> = (0..atoms.len()).collect();
        positions.sort_by_key(|&id| &atoms[id]);
        Self { atoms, positions }
    }

    fn id(&self, atom: &Atom) -> usize {
        self.atoms.iter().position(|actual| actual == atom).unwrap()
    }

    fn snapshot<'a>(
        &'a self,
        workspace: Workspace,
        input: &[u32],
        stride: usize,
        available: usize,
        work: &mut Work<'_>,
    ) -> Result<Snapshot<'a>, Stop> {
        workspace.record_retained(work);
        let rows = Rows::select(
            &self.atoms,
            self.positions.clone(),
            input,
            stride,
            workspace.worlds,
            available,
            work,
        )?;
        workspace.snapshot(rows, input, stride, available, work)
    }
}

fn snapshot<'a>(
    program: &Program,
    catalog: &'a Catalog,
    snapshots: &[u32],
    stride: usize,
    worlds: usize,
    available: usize,
    work: &mut Work<'_>,
) -> Result<Snapshot<'a>, Stop> {
    let workspace = Workspace::new(program, worlds, available, work)?;
    catalog.snapshot(workspace, snapshots, stride, available, work)
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

fn catalog() -> Catalog {
    // Canonical source order differs deliberately from demanded ID order.
    Catalog::new(vec![atom("b"), atom("c"), atom("a")])
}

fn assert_conjunctions(
    program: &Program,
    catalog: &Catalog,
    input: &[u32],
    snapshot: &mut Snapshot<'_>,
    work: &mut Work<'_>,
) {
    let (atoms, mut join) = snapshot.parts();
    join.reset(&program.templates()[0], work).unwrap();
    let mut selected = Vec::new();
    for (depth, name) in ["a", "b", "c"].into_iter().enumerate() {
        if !atoms.clone().any(|atom| atom.predicate().name() == name) {
            break;
        }
        selected.push(catalog.id(&atom(name)));
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
        let mut snapshot = catalog
            .snapshot(workspace, &input, 1, usize::MAX, &mut work)
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
        let snapshot = catalog
            .snapshot(workspace, &[truth; 65], 1, usize::MAX, &mut work)
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
    catalog
        .snapshot(
            original.into_workspace(),
            &[7; 33],
            1,
            usize::MAX,
            &mut repeated,
        )
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
    let indices = 3 * size_of::<usize>();
    let exact = retained + indices + 3 * size_of::<u32>();
    let snapshot = catalog
        .snapshot(workspace, &[7; 3], 1, exact, &mut work)
        .unwrap();
    assert_eq!(snapshot.bytes(), exact);
    let mut next = Work::source(&control, u64::MAX);
    let refused = catalog.snapshot(snapshot.into_workspace(), &[7; 3], 1, exact - 1, &mut next);
    assert!(matches!(refused, Err(Stop::Allocation)));
    assert_eq!(next.mask_bytes, retained + indices);
}

#[test]
fn pairwise_overlap_does_not_establish_a_common_world() {
    let program = program();
    let catalog = catalog();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    // a={0,2}, b={1,2}, c={0,1}: each pair intersects, but all three do not.
    let mut snapshot =
        snapshot(&program, &catalog, &[6, 3, 5], 1, 3, usize::MAX, &mut work).unwrap();
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
    assert!(old.rows.iter().eq(fresh.rows.iter()));
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
    let catalog = Catalog::new(vec![
        Atom::new(negative.clone(), vec![]).unwrap(),
        Atom::new(positive.clone(), vec![]).unwrap(),
    ]);
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
    let catalog = catalog();
    let control = Control::default();
    for count in [31usize, 32, 33, 63, 64, 65] {
        let mut work = Work::source(&control, u64::MAX);
        let mut snapshot = snapshot(
            &program,
            &catalog,
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
    let catalog = Catalog {
        atoms: (0..33).map(|id| atom(&format!("a{id}"))).collect(),
        positions: vec![32],
    };
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

#[test]
fn selected_rows_borrow_the_authoritative_typed_payload() {
    let predicate = Predicate::new("p", 1).unwrap();
    let catalog = Catalog::new(vec![
        Atom::new(predicate.clone(), vec![Value::String("payload".repeat(32))]).unwrap(),
        Atom::new(predicate.clone(), vec![Value::Number(1)]).unwrap(),
        Atom::new(predicate, vec![Value::Symbol("payload".repeat(32))]).unwrap(),
    ]);
    let program = program();
    let control = Control::default();
    let mut work = Work::source(&control, u64::MAX);
    let selected = snapshot(&program, &catalog, &[5], 1, 1, usize::MAX, &mut work).unwrap();
    let expected: Vec<_> = catalog
        .positions
        .iter()
        .copied()
        .filter(|id| *id != 1)
        .collect();
    assert_eq!(selected.rows.positions, expected);
    for actual in selected.rows.iter() {
        let original = &catalog.atoms[catalog.id(actual)];
        assert!(std::ptr::eq(actual, original));
        assert_eq!(actual.values().as_ptr(), original.values().as_ptr());
        match (&actual.values()[0], &original.values()[0]) {
            (Value::String(left), Value::String(right))
            | (Value::Symbol(left), Value::Symbol(right)) => {
                assert_eq!(left.as_ptr(), right.as_ptr());
            }
            other => panic!("unexpected selected values: {other:?}"),
        }
    }
}
