//! Restricted passes can differ; their saturated regions retain global closure.

use super::*;

fn positive_rules(atoms: usize, rules: &[(usize, usize)]) -> Theory {
    let mut nodes: Vec<_> = (0..atoms).map(Node::atom).collect();
    let roots = rules
        .iter()
        .map(|&(body, head)| {
            let root = nodes.len();
            nodes.push(Node::implies(body, head));
            root
        })
        .collect();
    theory(atoms, nodes, roots)
}

fn bridged_cycles() -> Theory {
    positive_rules(5, &[(1, 0), (0, 1), (1, 4), (3, 2), (2, 3), (4, 2)])
}

fn disjunctive_boundary() -> Theory {
    theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::or_pair([0, 2]),
            Node::implies(3, 4),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::falsum(),
            Node::implies(3, 8),
            Node::or_pair([3, 9]),
        ],
        vec![5, 6, 7, 10],
    )
}

fn aliased_boundary() -> Theory {
    theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::atom(0),
            Node::implies(1, 0),
            Node::implies(4, 1),
            Node::implies(2, 0),
            Node::implies(3, 2),
            Node::falsum(),
            Node::implies(3, 9),
            Node::or_pair([3, 10]),
        ],
        vec![5, 6, 7, 8, 11],
    )
}

/// Every atom is a reference closure head, so the old global event engine has
/// no conceptual outside seeds and reads every producer body.
fn global_scratch(program: &Program) -> NarrowingScratch {
    let mut scratch = NarrowingScratch::default();
    scratch
        .founded
        .needed(
            &program.owner,
            &program.index,
            &program.producers,
            &mut Work::new(u64::MAX),
        )
        .unwrap();
    scratch
        .founded
        .prepare_global_reference(&program.owner, &program.index, &program.producers);
    scratch
}

#[test]
fn acyclic_bridges_delay_cuts_without_changing_the_fixed_point() {
    let program = Program::new(bridged_cycles());
    let mut restricted = Scratch::default();
    restricted
        .complete(
            &program.owner,
            &program.index,
            &program.producers,
            &[0],
            &[0],
            &mut Work::new(u64::MAX),
        )
        .unwrap();
    let mut global = global_scratch(&program);
    global
        .founded
        .complete(
            &program.owner,
            &program.index,
            &program.producers,
            &[0],
            &[0],
            &mut Work::new(u64::MAX),
        )
        .unwrap();
    assert_eq!(restricted.cyclic_atoms, [0b0_1111]);
    assert_eq!(
        restricted.atoms,
        [0b0_1100],
        "the bridge initially supports the downstream cycle"
    );
    assert_eq!(global.founded.atoms, [0]);
    let global_before = global.founded.passes;
    let mut left = Region::all_open(5);
    let mut right = left.clone();
    let mut left_scratch = NarrowingScratch::default();
    let left_result = program
        .close(&mut left, &mut program.index.knowledge(), &mut left_scratch)
        .0;
    let right_result = program
        .close(&mut right, &mut program.index.knowledge(), &mut global)
        .0;
    assert_eq!(left_result, right_result);
    assert_eq!(left, right);
    assert!((0..5).all(|atom| left.is_cut(atom)));
    assert!(left_scratch.founded.passes > global.founded.passes - global_before);
}

#[test]
fn every_partial_region_matches_global_saturation() {
    for owner in [
        bridged_cycles(),
        disjunctive_boundary(),
        aliased_boundary(),
        negative_seed(),
        projected_bodies(),
    ] {
        let program = Program::new(owner);
        let atoms = program.owner.atom_count();
        for encoded in 0..3_usize.pow(u32::try_from(atoms).unwrap()) {
            let mut left = Region::all_open(atoms);
            let mut decisions = encoded;
            for atom in 0..atoms {
                match decisions % 3 {
                    1 => {
                        left.cut(atom);
                    }
                    2 => {
                        left.hold(atom);
                    }
                    _ => {}
                }
                decisions /= 3;
            }
            let mut right = left.clone();
            let mut restricted = NarrowingScratch::default();
            let mut global = global_scratch(&program);
            let left_result = program
                .close(&mut left, &mut program.index.knowledge(), &mut restricted)
                .0;
            let right_result = program
                .close(&mut right, &mut program.index.knowledge(), &mut global)
                .0;
            assert_eq!(left_result, right_result, "partial region {encoded}");
            if matches!(left_result, Narrowing::Fixed { .. }) {
                assert_eq!(left, right, "partial region {encoded}");
            }
        }
    }
}

fn family(program: &Program, global: bool) -> BTreeSet<usize> {
    let mut scratch = if global {
        global_scratch(program)
    } else {
        NarrowingScratch::default()
    };
    let mut pending = vec![(
        Region::all_open(program.owner.atom_count()),
        program.index.knowledge(),
    )];
    let mut candidates = BTreeSet::new();
    while let Some((mut region, mut known)) = pending.pop() {
        if global {
            invalidate(&mut known);
        }
        if program.close(&mut region, &mut known, &mut scratch).0 == Narrowing::Refuted {
            continue;
        }
        let open = region.open().next();
        if let Some(atom) = open {
            let (cut, held) = region.split(atom);
            pending.push((cut, known.clone()));
            pending.push((held, known));
        } else {
            let mask = (0..program.owner.atom_count())
                .filter(|&atom| region.is_held(atom))
                .fold(0, |mask, atom| mask | (1 << atom));
            assert!(candidates.insert(mask));
        }
    }
    candidates
}

#[test]
fn restricted_and_global_traversals_keep_the_full_family() {
    for owner in [
        bridged_cycles(),
        disjunctive_boundary(),
        aliased_boundary(),
        negative_seed(),
        projected_bodies(),
    ] {
        let program = Program::new(owner);
        let actual = family(&program, false);
        assert_eq!(actual, family(&program, true));
        let expected: BTreeSet<_> = (0..1 << program.owner.atom_count())
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        let answers: BTreeSet<_> = actual
            .into_iter()
            .filter(|&mask| accepted(&program.owner, mask))
            .collect();
        assert_eq!(answers, expected);
    }
}

#[test]
fn component_preparation_requires_the_exact_owner() {
    let first = Program::new(positive_rules(3, &[(0, 1), (1, 0)]));
    let second = Program::new(positive_rules(3, &[(1, 2), (2, 1)]));
    let mut scratch = Scratch::default();
    for (program, expected) in [(&first, 0b011), (&second, 0b110), (&first, 0b011)] {
        scratch
            .prepare_bodies(
                &program.owner,
                &program.index,
                &program.producers,
                &mut Work::new(u64::MAX),
            )
            .unwrap();
        assert_eq!(scratch.cyclic_atoms, [expected]);
        assert!(
            scratch
                .owner
                .as_ref()
                .unwrap()
                .same_instance(&program.owner)
        );
        assert!(
            !scratch
                .states
                .iter()
                .any(|state| matches!(state, State::Visiting(_)))
        );
    }
}

#[test]
fn stopped_component_prefixes_publish_no_authority() {
    let program = Program::new(bridged_cycles());
    let mut complete = Work::new(u64::MAX);
    let mut expected = Scratch::default();
    expected
        .prepare_bodies(
            &program.owner,
            &program.index,
            &program.producers,
            &mut complete,
        )
        .unwrap();
    let mut interrupted_visits = false;
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..complete.spent {
            let mut scratch = Scratch::default();
            let mut quota = RefuseAfter {
                remaining: ceiling,
                failure,
            };
            let mut work = Work::reserved(&mut quota);
            assert_eq!(
                scratch.prepare_bodies(
                    &program.owner,
                    &program.index,
                    &program.producers,
                    &mut work
                ),
                Err(failure)
            );
            assert_eq!(work.spent, ceiling);
            assert!(scratch.owner.is_none());
            assert!(scratch.may_affect_body(0));
            interrupted_visits |= scratch
                .states
                .iter()
                .any(|state| matches!(state, State::Visiting(_)));
            scratch
                .prepare_bodies(
                    &program.owner,
                    &program.index,
                    &program.producers,
                    &mut Work::new(u64::MAX),
                )
                .unwrap();
            assert_eq!(scratch.cyclic_atoms, expected.cyclic_atoms);
            assert_eq!(scratch.body_nodes, expected.body_nodes);
            assert!(
                !scratch
                    .states
                    .iter()
                    .any(|state| matches!(state, State::Visiting(_)))
            );
        }
    }
    assert!(
        interrupted_visits,
        "refusal coverage reaches live DFS cursors"
    );
}

#[test]
fn boundary_seeds_skip_unrelated_atom_occurrences() {
    fn warm(extra: usize) -> u64 {
        let mut nodes = vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(2, 0),
        ];
        nodes.extend((0..extra).map(|_| Node::atom(2)));
        let program = Program::new(theory(3, nodes, vec![2, 3, 4, 5]));
        let mut scratch = Scratch::default();
        scratch.prepare_projected(&program.owner, &program.index, &program.producers);
        let mut work = Work::new(u64::MAX);
        scratch
            .complete(
                &program.owner,
                &program.index,
                &program.producers,
                &vec![0; flag_words(program.owner.nodes().len())],
                &[0],
                &mut work,
            )
            .unwrap();
        assert_eq!(scratch.cyclic_atoms, [0b011]);
        assert_eq!(scratch.atoms, [0b011]);
        assert_eq!(scratch.body_nodes.len(), 3);
        work.spent
    }
    assert_eq!(warm(0), warm(200));
}
