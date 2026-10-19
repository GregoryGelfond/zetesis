//! A recorded condition excludes its entire unsupported set in every answer,
//! independently of the ancestor decisions which exposed that set.

use super::{Explanation, Input, Limits, collect};
use crate::regions::founded::Scratch;
use crate::regions::{Work, flag_words};
use crate::test_support::{SinglePermits, theory, world};
use crate::{Narrower, Node, NodeView, Producers, RegionLimits, Theory};
use zetesis_cpu::{Cancellation, Stop};

struct Program {
    owner: Theory,
    index: Narrower,
    producers: Producers,
}

struct Completed {
    scratch: Scratch,
    sure: Vec<u64>,
    never: Vec<u64>,
}

impl Program {
    fn new(owner: Theory) -> Self {
        let index = Narrower::new(&owner);
        let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .producers
            .unwrap();
        Self {
            owner,
            index,
            producers,
        }
    }

    fn answers(&self) -> Vec<usize> {
        (0..1 << self.owner.atom_count())
            .filter(|&mask| {
                crate::check(
                    &self.owner,
                    &world(&self.owner, mask),
                    crate::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
                .accepted()
            })
            .collect()
    }

    fn complete(&self, decisions: &[(usize, bool)], extra_false: &[usize]) -> Completed {
        let mut sure = vec![0; flag_words(self.owner.atom_count())];
        let mut never = sure.clone();
        for &(atom, value) in decisions {
            let words = if value { &mut sure } else { &mut never };
            words[atom / 64] |= 1 << (atom % 64);
        }
        let mut node_never = vec![0; flag_words(self.owner.nodes().len())];
        let view = self.owner.view();
        for node in 0..view.len() {
            let false_here = match view.node(node).unwrap() {
                NodeView::Atom(atom) => bit(&never, atom),
                NodeView::False => true,
                NodeView::Implies(left, right) => matches!(
                    (view.node(left).unwrap(), view.node(right).unwrap()),
                    (NodeView::Atom(atom), NodeView::False) if bit(&sure, atom)
                ),
                _ => false,
            };
            if false_here || extra_false.contains(&node) {
                node_never[node / 64] |= 1 << (node % 64);
            }
        }
        let mut scratch = Scratch::default();
        scratch
            .complete(
                &self.owner,
                &self.index,
                &self.producers,
                &node_never,
                &never,
                &mut Work::new(u64::MAX),
            )
            .unwrap();
        Completed {
            scratch,
            sure,
            never,
        }
    }
}

fn bit(words: &[u64], atom: usize) -> bool {
    words[atom / 64] & (1 << (atom % 64)) != 0
}

fn limits() -> Limits {
    Limits {
        atoms: 32,
        literals: 16,
        visits: 4096,
        bytes: 65_536,
    }
}

fn explain(
    program: &Program,
    completed: &Completed,
    start: usize,
    limits: Limits,
    work: &mut Work<'_>,
) -> Result<Option<Explanation>, Stop> {
    collect(
        Input {
            theory: &program.owner,
            index: &program.index,
            producers: &program.producers,
            completed: &completed.scratch,
            atom_sure: &completed.sure,
            atom_never: &completed.never,
        },
        start,
        limits,
        work,
        &mut 0,
    )
}

fn matches(conditions: &[(usize, bool)], mask: usize) -> bool {
    conditions
        .iter()
        .all(|&(atom, value)| (mask & (1 << atom) != 0) == value)
}

fn assert_sound(program: &Program, completed: &Completed, record: &Explanation, answers: &[usize]) {
    assert!(!record.unsupported.is_empty());
    for &(atom, value) in &record.conditions {
        assert!(atom < program.owner.atom_count());
        assert!(bit(
            if value {
                &completed.sure
            } else {
                &completed.never
            },
            atom
        ));
    }
    for &atom in &record.unsupported {
        assert!(atom < program.owner.atom_count());
    }
    // Quantify over all original answers, not only descendants of the region
    // whose completed closure supplied the explanation hint.
    for &answer in answers {
        if matches(&record.conditions, answer) {
            assert!(
                record
                    .unsupported
                    .iter()
                    .all(|atom| answer & (1 << atom) == 0),
                "conditions={:?}, unsupported={:?}, answer={answer}",
                record.conditions,
                record.unsupported
            );
        }
    }
}

fn partial(mut code: usize, atoms: usize) -> Vec<(usize, bool)> {
    (0..atoms)
        .filter_map(|atom| {
            let state = code % 3;
            code /= 3;
            (state != 0).then_some((atom, state == 2))
        })
        .collect()
}

/// a <-> b, optional x, and every subset of eight possible support sources.
fn tiny_program(selected: usize) -> Program {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(2, 3),
        Node::or_pair([2, 4]),
        Node::implies(0, 1),
        Node::implies(1, 0),
        Node::implies(2, 0),
        Node::implies(4, 1),
        Node::and_pair([1, 2]),
        Node::implies(10, 0),
        Node::or_pair([1, 2]),
        Node::implies(12, 0),
        Node::or_pair([0, 2]),
        Node::implies(1, 14),
        Node::implies(0, 3),
        Node::or_pair([0, 16]),
        Node::implies(2, 17),
        Node::or_pair([0, 1]),
    ];
    let mut roots = vec![5, 6, 7];
    roots.extend(
        [8, 9, 11, 13, 15, 18, 0, 19]
            .into_iter()
            .enumerate()
            .filter_map(|(bit, root)| (selected & (1 << bit) != 0).then_some(root)),
    );
    Program::new(theory(3, nodes, roots))
}

#[test]
fn every_tiny_explanation_excludes_all_matching_original_answers() {
    let mut unconditional = 0;
    let mut conditional = 0;
    let mut open_starts = 0;
    for selected in 0..256 {
        let program = tiny_program(selected);
        let answers = program.answers();
        for code in 0..27 {
            let completed = program.complete(&partial(code, 3), &[]);
            for start in 0..2 {
                let Some(record) = explain(
                    &program,
                    &completed,
                    start,
                    limits(),
                    &mut Work::new(u64::MAX),
                )
                .unwrap() else {
                    continue;
                };
                assert!(!bit(&completed.never, start));
                open_starts += usize::from(!bit(&completed.sure, start));
                assert!(record.unsupported.contains(&start));
                assert_sound(&program, &completed, &record, &answers);
                if record.conditions.is_empty() {
                    unconditional += 1;
                } else {
                    conditional += 1;
                }
            }
        }
    }
    assert!(
        open_starts > 0,
        "the test must capture without a held output"
    );
    assert!(unconditional > 0, "the test must exercise unseeded cycles");
    assert!(
        conditional > 0,
        "the test must exercise explicit boundary conditions"
    );
}

/// a <-> b; a <- x; b <- not y; choices x,y,z.
fn boundary_program() -> Program {
    Program::new(theory(
        5,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::atom(4),
            Node::falsum(),
            Node::implies(2, 5),
            Node::implies(3, 5),
            Node::implies(4, 5),
            Node::or_pair([2, 6]),
            Node::or_pair([3, 7]),
            Node::or_pair([4, 8]),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(2, 0),
            Node::implies(7, 1),
        ],
        vec![9, 10, 11, 12, 13, 14, 15],
    ))
}

#[test]
fn boundary_conditions_do_not_retain_unrelated_ancestor_decisions() {
    let program = boundary_program();
    let answers = program.answers();
    for z in [false, true] {
        let completed = program.complete(&[(0, true), (2, false), (3, true), (4, z)], &[]);
        let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .unwrap();
        assert_eq!(record.conditions, [(2, false), (3, true)]);
        assert_eq!(record.unsupported, [0, 1]);
        assert_sound(&program, &completed, &record, &answers);
    }
}

#[test]
fn an_enabled_disjunctive_head_is_not_blocked_by_its_held_cohead() {
    let program = Program::new(theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::implies(0, 1),
            Node::implies(1, 0),
            Node::or_pair([0, 3]),
            Node::implies(2, 6),
        ],
        vec![2, 4, 5, 7],
    ));
    let answers = program.answers();
    assert!(
        answers.contains(&0b1100),
        "the cohead is an alternative answer"
    );
    assert!(
        answers.contains(&0b0111),
        "the complete producer also permits the cycle"
    );
    let completed = program.complete(&[(0, true), (3, true)], &[]);
    assert!(
        explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_none()
    );
}

#[test]
fn opaque_false_nodes_do_not_become_unconditional_explanations() {
    // a <-> b; a <- not not x; {x}. At x=false the opaque body really is
    // false, but a cached false node does not explain which decision made it so.
    let program = Program::new(theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::implies(4, 3),
            Node::implies(0, 1),
            Node::implies(1, 0),
            Node::implies(5, 0),
            Node::or_pair([2, 4]),
        ],
        vec![6, 7, 8, 9],
    ));
    let completed = program.complete(&[(0, true), (2, false)], &[5]);
    assert_ne!(
        completed.scratch.missing_in_word(0, completed.never[0]) & 1,
        0
    );
    assert!(
        explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_none()
    );
    assert!(
        program.answers().contains(&0b111),
        "an unconditional cycle cut would be false"
    );
}

#[test]
fn foreign_completed_geometry_supplies_no_explanation() {
    let original = boundary_program();
    let foreign = boundary_program();
    let completed = original.complete(&[(2, false), (3, true)], &[]);
    assert!(!original.owner.same_instance(&foreign.owner));
    assert!(
        explain(&foreign, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_none()
    );
    assert!(
        explain(&original, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_some()
    );
}

#[test]
fn every_local_explanation_cap_declines_without_partial_evidence() {
    let program = boundary_program();
    let completed = program.complete(&[(2, false), (3, true)], &[]);
    for limited in [
        Limits {
            atoms: 1,
            ..limits()
        },
        Limits {
            literals: 1,
            ..limits()
        },
        Limits {
            visits: 0,
            ..limits()
        },
        Limits {
            bytes: 0,
            ..limits()
        },
    ] {
        assert!(
            explain(&program, &completed, 0, limited, &mut Work::new(u64::MAX))
                .unwrap()
                .is_none()
        );
    }
    assert!(
        explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
            .unwrap()
            .is_some()
    );
}

#[test]
fn every_interrupted_explanation_prefix_returns_no_certificate() {
    let program = boundary_program();
    let completed = program.complete(&[(2, false), (3, true)], &[]);
    let mut measured = Work::new(u64::MAX);
    let expected = explain(&program, &completed, 0, limits(), &mut measured)
        .unwrap()
        .unwrap();
    assert!(measured.spent > 0);
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for prefix in 0..measured.spent {
            let mut quota = SinglePermits::new(prefix, failure);
            let mut work = Work::reserved(&mut quota);
            assert!(
                matches!(explain(&program, &completed, 0, limits(), &mut work),
                Err(actual) if actual == failure)
            );
            assert_eq!(work.spent, prefix);
            let retry = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
                .unwrap()
                .unwrap();
            assert_eq!(retry.conditions, expected.conditions);
            assert_eq!(retry.unsupported, expected.unsupported);
        }
    }
}

#[test]
fn failed_conjunct_witness_rolls_back_speculative_atoms() {
    // a <- (b or not not x) and y; b <- a; choices x,y.
    // The first conjunct tentatively adds b, then its opaque disjunct fails.
    // The second conjunct has the independent witness y=false. Keeping b
    // would weaken reuse by recording an unsupported atom that was not needed.
    let program = Program::new(theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::falsum(),
            Node::implies(2, 4),
            Node::implies(5, 4),
            Node::implies(3, 4),
            Node::or_pair([2, 5]),
            Node::or_pair([3, 7]),
            Node::or_pair([1, 6]),
            Node::and_pair([10, 3]),
            Node::implies(11, 0),
            Node::implies(0, 1),
        ],
        vec![8, 9, 12, 13],
    ));
    let conjuncts = program.index.chain_of[11].unwrap();
    let alternatives = program.index.chain_of[10].unwrap();
    assert_eq!(
        program.index.chains[crate::regions::chain_position(conjuncts)]
            .leaves(&program.index.extrema),
        [10, 3]
    );
    assert_eq!(
        program.index.chains[crate::regions::chain_position(alternatives)]
            .leaves(&program.index.extrema),
        [1, 6]
    );
    let completed = program.complete(&[(0, true), (2, false), (3, false)], &[6]);
    let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(3, false)]);
    assert_eq!(record.unsupported, [0]);
    assert_sound(&program, &completed, &record, &program.answers());
}

#[test]
fn disjunctive_bodies_require_every_false_operand() {
    // a <-> b; a <- x or y; choices x,y. Either external atom alone
    // restores a stable answer containing the cycle, so both cuts matter.
    let program = Program::new(theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::falsum(),
            Node::implies(2, 4),
            Node::implies(3, 4),
            Node::or_pair([2, 5]),
            Node::or_pair([3, 6]),
            Node::implies(0, 1),
            Node::implies(1, 0),
            Node::or_pair([2, 3]),
            Node::implies(11, 0),
        ],
        vec![7, 8, 9, 10, 12],
    ));
    let completed = program.complete(&[(0, true), (2, false), (3, false)], &[]);
    let record = explain(&program, &completed, 0, limits(), &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(2, false), (3, false)]);
    assert_eq!(record.unsupported, [0, 1]);
    let answers = program.answers();
    assert!(answers.contains(&0b0111));
    assert!(answers.contains(&0b1011));
    assert_sound(&program, &completed, &record, &answers);
}

#[test]
fn duplicate_atom_occurrences_do_not_spend_literal_capacity_twice() {
    // Distinct Atom nodes for b and x make these distinct flattened leaves.
    // Duplicate sources and head occurrences must still have one certificate
    // entry per semantic atom, including when the list limit is exact.
    let program = Program::new(theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 5),
            Node::or_pair([2, 6]),
            Node::or_pair([1, 3]),
            Node::or_pair([2, 4]),
            Node::or_pair([8, 9]),
            Node::implies(10, 0),
            Node::implies(0, 1),
            Node::implies(9, 0),
        ],
        vec![7, 11, 12, 13, 13],
    ));
    let completed = program.complete(&[(0, true), (2, false)], &[]);
    let exact = Limits {
        atoms: 2,
        literals: 1,
        ..limits()
    };
    let record = explain(&program, &completed, 0, exact, &mut Work::new(u64::MAX))
        .unwrap()
        .unwrap();
    assert_eq!(record.conditions, [(2, false)]);
    assert_eq!(record.unsupported, [0, 1]);
    assert_sound(&program, &completed, &record, &program.answers());
}
