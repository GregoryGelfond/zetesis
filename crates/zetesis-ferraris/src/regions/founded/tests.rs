//! Candidate cuts preserve every independently checked stable completion.

use super::*;
use crate::test_support::{theory, world};
use crate::{
    FrozenSubject, Interpretation, Limits, Narrowing, NarrowingAttempt, NarrowingQuota,
    NarrowingScratch, Node, OriginalSubject, Region, RegionLimits,
};
use zetesis_cpu::Cancellation;

mod projected_heads;
mod projected_parents;
mod reuse;
mod seed_fusion;

impl Scratch {
    fn use_original_projection(&mut self, owner: &Theory, index: &Narrower, producers: &Producers) {
        let initial = initialization::prepare(
            owner,
            index,
            &self.body_nodes,
            &self.cyclic_atoms,
            &mut Work::new(u64::MAX),
        )
        .unwrap();
        self.projected = Some(Projection {
            parents: index.parents.clone(),
            heads: projected_heads::original_rows(owner, producers),
            seed_start: initial.len(),
            initial,
        });
    }

    /// Complete both cone preparation and the deferred incidence projection.
    fn prepare_projected(&mut self, owner: &Theory, index: &Narrower, producers: &Producers) {
        for _ in 0..2 {
            self.prepare_bodies(owner, index, producers, &mut Work::new(u64::MAX))
                .unwrap();
        }
        assert!(self.projected.is_some());
    }

    /// Test-only reference: use the existing engine over every head and body,
    /// with no conceptual noncyclic seeds and no production algorithm switch.
    fn prepare_global_reference(
        &mut self,
        owner: &Theory,
        index: &Narrower,
        producers: &Producers,
    ) {
        self.owner = None;
        self.cyclic_atoms = vec![0; flag_words(owner.atom_count())];
        for atom in 0..owner.atom_count() {
            self.cyclic_atoms[atom / 64] |= 1 << (atom % 64);
        }
        self.prepare_cone(owner, index, producers, false, &mut Work::new(u64::MAX))
            .unwrap();
        self.use_original_projection(owner, index, producers);
        self.owner = Some(owner.clone());
    }
}

fn producers(owner: &Theory) -> Producers {
    crate::producers(owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap()
}

fn narrow(owner: &Theory, region: &mut Region) -> Narrowing {
    let index = Narrower::new(owner);
    index
        .narrow_known(
            OriginalSubject::new(owner, Some(&producers(owner))),
            region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default(),
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .0
}

fn cycle() -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3],
    )
}

fn accepted(owner: &Theory, mask: usize) -> bool {
    crate::check(
        owner,
        &world(owner, mask),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .accepted()
}

#[test]
fn unseeded_cycles_are_cut_before_a_candidate() {
    let owner = cycle();
    let mut region = Region::all_open(2);
    assert_eq!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(0) && region.is_cut(1));
    assert!(accepted(&owner, 0));
    assert!(!accepted(&owner, 3));
}

#[test]
fn held_atoms_do_not_seed_their_own_cycles() {
    let owner = cycle();
    let mut region = Region::all_open(2);
    region.hold(0);
    assert_eq!(narrow(&owner, &mut region), Narrowing::Refuted);
}

#[test]
fn disjunctive_facts_seed_all_possible_heads() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([0, 1]),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![2, 3, 4],
    );
    // Excluding the other held head from each disjunctive producer would
    // incorrectly turn this stable positive disjunctive cycle into no support.
    assert!(accepted(&owner, 3));
    let mut region = Region::all_open(2);
    region.hold(0);
    assert!(matches!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { .. }
    ));
    assert!(region.is_held(0) && region.is_held(1));
}

#[test]
fn atomic_choice_permission_seeds_positive_cycles() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::implies(0, 1),
            Node::implies(1, 0),
        ],
        vec![4, 5, 6],
    );
    assert!(accepted(&owner, 0) && accepted(&owner, 3));
    let mut region = Region::all_open(2);
    assert_eq!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { changed: false }
    );
    assert!(region.is_open(0) && region.is_open(1));
}

#[test]
fn conditional_choices_need_founded_bodies() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(1, 2),
            Node::or_pair([1, 3]),
            Node::implies(0, 4),
            Node::implies(1, 0),
        ],
        vec![5, 6],
    );
    assert!(accepted(&owner, 0));
    assert!(!accepted(&owner, 3));
    let mut region = Region::all_open(2);
    assert_eq!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(0) && region.is_cut(1));
}

#[test]
fn unresolved_negations_supply_possible_support() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::implies(1, 2),
            Node::implies(3, 1),
            Node::implies(4, 0),
        ],
        vec![5, 6],
    );
    assert!(accepted(&owner, 1) && accepted(&owner, 2));
    let mut region = Region::all_open(2);
    assert_eq!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { changed: false }
    );
    assert!(region.is_open(0) && region.is_open(1));
}

#[test]
fn disjunctive_bodies_keep_external_seeds() {
    let owner = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([0, 1]),
            Node::implies(2, 0),
        ],
        vec![1, 3],
    );
    assert!(accepted(&owner, 3));
    let mut region = Region::all_open(2);
    assert!(matches!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { .. }
    ));
    assert!(region.is_held(0) && region.is_held(1));
}

fn compatible(region: &Region, candidate: &Interpretation) -> bool {
    (0..candidate.theory().atom_count()).all(|atom| {
        region
            .decision(atom)
            .is_none_or(|value| value == candidate.contains(atom))
    })
}

fn partial_region(mut code: usize, atoms: usize) -> Region {
    let mut region = Region::all_open(atoms);
    for atom in 0..atoms {
        match code % 3 {
            1 => {
                region.cut(atom);
            }
            2 => {
                region.hold(atom);
            }
            _ => {}
        }
        code /= 3;
    }
    region
}

#[test]
fn every_tiny_cut_preserves_stable_completions() {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(0, 1),
        Node::implies(1, 0),
        Node::or_pair([0, 1]),
        Node::implies(0, 3),
        Node::implies(1, 3),
        Node::or_pair([0, 7]),
        Node::and_pair([0, 1]),
        Node::or_pair([0, 2]),
        Node::implies(10, 2),
        Node::implies(11, 1),
        Node::implies(7, 3),
        Node::implies(14, 0),
        Node::implies(1, 9),
        Node::implies(4, 2),
        Node::implies(8, 0),
        Node::and_pair([10, 1]),
        Node::implies(19, 2),
        Node::or_pair([6, 0]),
        Node::implies(0, 3),
        Node::implies(4, 3),
        Node::implies(23, 3),
        Node::implies(24, 2),
    ];
    let roots = [
        0, 1, 2, 3, 4, 5, 6, 7, 9, 12, 13, 15, 16, 17, 18, 20, 21, 22, 24, 25,
    ];
    let mut retained = 0;
    let mut scratch = NarrowingScratch::default();
    for (position, &left) in roots.iter().enumerate() {
        for &right in &roots[position..] {
            let owner = theory(3, nodes.clone(), vec![left, right]);
            let answers: Vec<_> = (0..8)
                .filter(|&mask| accepted(&owner, mask))
                .map(|mask| world(&owner, mask))
                .collect();
            let index = Narrower::new(&owner);
            let producers = producers(&owner);
            for code in 0..27 {
                let mut region = partial_region(code, 3);
                let expected: Vec<_> = answers.iter().filter(|m| compatible(&region, m)).collect();
                let (result, _) = index
                    .narrow_known(
                        OriginalSubject::new(&owner, Some(&producers)),
                        &mut region,
                        &mut index.knowledge(),
                        &mut scratch,
                        RegionLimits::default(),
                        &Cancellation::default(),
                    )
                    .unwrap();
                match result {
                    Narrowing::Refuted => {
                        assert!(expected.is_empty(), "roots={left},{right}; region={code}");
                    }
                    Narrowing::Fixed { .. } => {
                        for answer in expected {
                            assert!(
                                compatible(&region, answer),
                                "roots={left},{right}; region={code}"
                            );
                            retained += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(retained > 100);
}

#[test]
fn frozen_queries_do_not_compute_possible_support() {
    let owner = cycle();
    let index = Narrower::new(&owner);
    let truth = vec![true; owner.nodes().len()];
    let mut region = Region::all_open(2);
    let mut scratch = NarrowingScratch::default();
    let (result, _) = index
        .narrow_frozen_known(
            FrozenSubject::new(&owner, &truth),
            &mut region,
            &mut index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result, Narrowing::Fixed { changed: false });
    assert_eq!(region.open().count(), 2);
    assert_eq!(scratch.founded.retained_bytes(), 0);
}

#[test]
fn incomplete_producer_inventory_does_not_cut_cycles() {
    let owner = cycle();
    let index = Narrower::new(&owner);
    let mut region = Region::all_open(2);
    let mut scratch = NarrowingScratch::default();
    let (result, _) = index
        .narrow_known(
            OriginalSubject::new(&owner, None),
            &mut region,
            &mut index.knowledge(),
            &mut scratch,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(result, Narrowing::Fixed { changed: false });
    assert_eq!(region.open().count(), 2);
    assert_eq!(scratch.founded.retained_bytes(), 0);
}

struct Allowance(u64);
impl NarrowingQuota for Allowance {
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        if self.0 == 0 {
            return Err(Stop::WorkLimit);
        }
        let granted = wanted.min(self.0);
        self.0 -= granted;
        Ok(granted)
    }
    fn refund(&mut self, unused: u64) {
        self.0 += unused;
    }
}

fn limited(owner: &Theory, scratch: &mut NarrowingScratch, work: u64) -> NarrowingAttempt {
    limited_region(
        owner,
        scratch,
        &mut Region::all_open(owner.atom_count()),
        work,
    )
}

fn limited_region(
    owner: &Theory,
    scratch: &mut NarrowingScratch,
    region: &mut Region,
    work: u64,
) -> NarrowingAttempt {
    let index = Narrower::new(owner);
    index.narrow_known_reserved(
        OriginalSubject::new(owner, Some(&producers(owner))),
        region,
        &mut index.knowledge(),
        scratch,
        &Cancellation::default(),
        &mut Allowance(work),
    )
}

#[test]
fn interrupted_foundation_passes_leave_no_stale_support() {
    let owner = cycle();
    let mut complete_region = Region::all_open(owner.atom_count());
    let complete = limited_region(
        &owner,
        &mut NarrowingScratch::default(),
        &mut complete_region,
        u64::MAX,
    );
    assert_eq!(complete.result.unwrap(), Narrowing::Fixed { changed: true });
    assert!(complete_region.is_cut(0) && complete_region.is_cut(1));
    let mut retained_prefixes = 0;
    for ceiling in 0..complete.statistics.work {
        let mut scratch = NarrowingScratch::default();
        let stopped = limited(&owner, &mut scratch, ceiling);
        assert_eq!(stopped.result.unwrap_err(), Stop::WorkLimit);
        assert_eq!(stopped.statistics.work, ceiling);
        let retained = scratch.explanations.len();
        assert_eq!(
            u64::try_from(retained).unwrap(),
            stopped.statistics.support_explanations.certified
        );
        assert!(retained <= 1);
        if retained == 1 {
            assert_eq!(scratch.explanations.outputs(0).collect::<Vec<_>>(), [0, 1]);
            retained_prefixes += 1;
        }
        let mut retry_region = Region::all_open(owner.atom_count());
        let reused = limited_region(&owner, &mut scratch, &mut retry_region, u64::MAX);
        assert_eq!(reused.result, complete.result);
        assert_eq!(retry_region, complete_region);
        if retained == 0 {
            assert_eq!(reused.statistics, complete.statistics);
        } else {
            // The failed prefix may have finished a complete explanation
            // before stopping in later cut publication or propagation. That
            // row is valid for fresh knowledge; a partial closure is not.
            let counts = reused.statistics.support_explanations;
            assert_eq!(counts.attempts, 0);
            assert_eq!(counts.certified, 0);
            assert_eq!(counts.matched, 1);
            assert_eq!(counts.cut, 2);
            assert_eq!(counts.forced, 0);
            assert_eq!(counts.refuted, 0);
            assert_eq!(reused.statistics.held, complete.statistics.held);
            assert_eq!(reused.statistics.cut, complete.statistics.cut);
            assert_eq!(
                reused.statistics.propagations,
                complete.statistics.propagations
            );
        }
    }
    assert!(
        retained_prefixes > 0,
        "late stops must exercise complete-row reuse"
    );
}

#[test]
fn support_work_attribution_retains_each_refused_prefix() {
    let owner = cycle();
    let complete = limited(&owner, &mut NarrowingScratch::default(), u64::MAX);
    assert!(complete.statistics.support_work > 0);
    assert!(complete.statistics.founded_work > 0);
    let mut previous = crate::NarrowingStatistics::default();
    for ceiling in 0..=complete.statistics.work {
        let attempt = limited(&owner, &mut NarrowingScratch::default(), ceiling);
        let counts = attempt.statistics;
        assert_eq!(counts.work, ceiling);
        assert!(counts.support_work + counts.founded_work <= counts.work);
        assert!(counts.support_work >= previous.support_work);
        assert!(counts.founded_work >= previous.founded_work);
        if ceiling < complete.statistics.work {
            assert_eq!(attempt.result, Err(Stop::WorkLimit));
        } else {
            assert_eq!(attempt.result, complete.result);
            assert_eq!(counts, complete.statistics);
        }
        previous = counts;
    }
}

fn acyclic_disjunction() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::and_pair([0, 0]),
            Node::or_pair([1, 2]),
            Node::implies(6, 7),
        ],
        vec![5, 8, 8],
    )
}

#[test]
fn acyclic_gate_reuses_only_its_completed_result() {
    let owner = acyclic_disjunction();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut work = Work::new(u64::MAX);
    assert!(
        !Scratch::default()
            .needed(&owner, &index, &producers, &mut work)
            .unwrap()
    );
    assert!(work.spent > 0);
    // A different worker gets the completed original classification without
    // preparing a second graph or growing per-region closure scratch.
    let mut untouched = Scratch::default();
    let mut none = Work::new(0);
    assert!(
        !untouched
            .needed(&owner, &index, &producers, &mut none)
            .unwrap()
    );
    assert_eq!(none.spent, 0);
    assert_eq!(untouched.retained_bytes(), 0);
}

#[test]
fn positive_cycles_retain_foundedness_propagation() {
    let owner = cycle();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    assert!(
        Scratch::default()
            .needed(&owner, &index, &producers, &mut Work::new(u64::MAX),)
            .unwrap()
    );
    let mut region = Region::all_open(2);
    assert_eq!(
        narrow(&owner, &mut region),
        Narrowing::Fixed { changed: true }
    );
    assert!(region.is_cut(0) && region.is_cut(1));
}

#[test]
fn interrupted_cycle_tests_publish_no_classification() {
    for owner in [cycle(), acyclic_disjunction()] {
        let index = Narrower::new(&owner);
        let mut spent = Work::new(u64::MAX);
        let expected = Scratch::default()
            .needed(&owner, &index, &producers(&owner), &mut spent)
            .unwrap();
        for ceiling in 0..spent.spent {
            let producers = producers(&owner);
            let mut scratch = Scratch::default();
            let mut limited = Work::new(ceiling);
            assert_eq!(
                scratch
                    .needed(&owner, &index, &producers, &mut limited,)
                    .unwrap_err(),
                Stop::WorkLimit
            );
            assert_eq!(limited.spent, ceiling);
            assert!(producers.founded_cycles.get().is_none());
            assert_eq!(
                scratch
                    .needed(&owner, &index, &producers, &mut Work::new(u64::MAX),)
                    .unwrap(),
                expected
            );
            assert_eq!(producers.founded_cycles.get(), Some(&expected));
        }
    }
}

fn projected_bodies() -> Theory {
    theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::and_pair([0, 0]),
            Node::and_pair([6, 1]),
            Node::or_pair([1, 2]),
            Node::implies(7, 8),
            Node::implies(2, 1),
            // A constraint contributes no producer body. Its formula positions
            // were nevertheless initialized by the previous full-node pass.
            Node::and_pair([0, 2]),
            Node::implies(11, 3),
            // The negated body is an opaque leaf whose known-false bit must
            // still disable its possible support on every subsequent round.
            Node::implies(4, 1),
        ],
        vec![5, 9, 10, 12, 13],
    )
}

#[test]
fn projected_bodies_match_full_node_relaxation() {
    let owner = projected_bodies();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut projected = Scratch::default();
    let mut full = Scratch::default();
    full.prepare_global_reference(&owner, &index, &producers);
    // Reference the former algorithm: initialize every original node, including
    // irrelevant opaque roots and head-only subformulas. The event engine stays
    // identical, isolating the cone restriction rather than mirroring it.
    full.body_nodes = (0..owner.nodes().len()).collect();
    full.use_original_projection(&owner, &index, &producers);
    for false_bodies in 0..16 {
        let mut never = vec![0; flag_words(owner.nodes().len())];
        for (bit_index, node) in [0, 1, 4, 7].into_iter().enumerate() {
            if false_bodies & (1 << bit_index) != 0 {
                never[node / 64] |= 1 << (node % 64);
            }
        }
        for cut_atoms in 0..8 {
            let atom_never = [cut_atoms];
            projected
                .complete(
                    &owner,
                    &index,
                    &producers,
                    &never,
                    &atom_never,
                    &mut Work::new(u64::MAX),
                )
                .unwrap();
            full.complete(
                &owner,
                &index,
                &producers,
                &never,
                &atom_never,
                &mut Work::new(u64::MAX),
            )
            .unwrap();
            for atom in 0..owner.atom_count() {
                let present = !bit(&atom_never, atom)
                    && (!bit(&projected.cyclic_atoms, atom) || projected.contains(atom));
                assert_eq!(
                    present,
                    full.contains(atom),
                    "atom {atom}, body mask {false_bodies}, atom mask {cut_atoms}"
                );
            }
        }
    }
    assert!(projected.body_nodes.len() < owner.nodes().len());
    assert!(
        !projected.body_nodes.contains(&6),
        "flattened interior stays absorbed"
    );
    assert!(
        !projected.body_nodes.contains(&8),
        "a head-only disjunction is irrelevant"
    );
    assert!(
        !projected.body_nodes.contains(&12),
        "headless constraint root is irrelevant"
    );
}

#[test]
fn unrelated_constraints_add_no_warm_initialization_work() {
    fn warm(owner: &Theory) -> (u64, usize) {
        let index = Narrower::new(owner);
        let producers = producers(owner);
        let mut scratch = Scratch::default();
        scratch.prepare_projected(owner, &index, &producers);
        let mut work = Work::new(u64::MAX);
        scratch
            .complete(
                owner,
                &index,
                &producers,
                &vec![0; flag_words(owner.nodes().len())],
                &[0],
                &mut work,
            )
            .unwrap();
        (work.spent, scratch.body_nodes.len())
    }
    let base = cycle();
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::implies(0, 1),
        Node::implies(1, 0),
        Node::falsum(),
    ];
    nodes.extend((0..200).map(|_| Node::implies(4, 4)));
    let mut roots = vec![2, 3];
    roots.extend(5..nodes.len());
    let noisy = theory(2, nodes, roots);
    assert_eq!(warm(&base), warm(&noisy));
    assert_eq!(warm(&noisy).1, 2);
}

#[test]
fn interrupted_body_preparation_publishes_no_owner() {
    let owner = projected_bodies();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut expected = Scratch::default();
    let mut cone = Work::new(u64::MAX);
    expected
        .prepare_bodies(&owner, &index, &producers, &mut cone)
        .unwrap();
    assert!(expected.owner.as_ref().unwrap().same_instance(&owner));
    assert!(expected.projected.is_none());
    let mut parents = Work::new(u64::MAX);
    expected
        .prepare_bodies(&owner, &index, &producers, &mut parents)
        .unwrap();
    assert!(cone.spent > 0 && parents.spent > 0);
    projected_parents::assert_rows(&expected, &owner, &index);
    projected_heads::assert_rows(&expected, &owner, &producers);
    for (phase, spent) in [cone.spent, parents.spent].into_iter().enumerate() {
        for ceiling in 0..spent {
            let mut scratch = Scratch::default();
            if phase == 1 {
                scratch
                    .prepare_bodies(&owner, &index, &producers, &mut Work::new(u64::MAX))
                    .unwrap();
                assert!(scratch.projected.is_none());
            }
            let mut limited = Work::new(ceiling);
            assert_eq!(
                scratch.prepare_bodies(&owner, &index, &producers, &mut limited),
                Err(Stop::WorkLimit),
                "phase {phase}, work prefix {ceiling}"
            );
            assert_eq!(limited.spent, ceiling);
            assert!(scratch.owner.is_none());
            assert!(scratch.projected.is_none());
            scratch.prepare_projected(&owner, &index, &producers);
            assert_eq!(scratch.body_nodes, expected.body_nodes);
            assert_eq!(scratch.fact_rows, expected.fact_rows);
            projected_parents::assert_rows(&scratch, &owner, &index);
            projected_heads::assert_rows(&scratch, &owner, &producers);
            assert!(scratch.owner.as_ref().unwrap().same_instance(&owner));
        }
    }
}

#[test]
fn body_preparation_requires_exact_original_identity() {
    let first = projected_bodies();
    let second = projected_bodies();
    assert!(!first.same_instance(&second));
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&first, &Narrower::new(&first), &producers(&first));
    let mut none = Work::new(0);
    scratch
        .prepare_bodies(
            &first.clone(),
            &Narrower::new(&first),
            &producers(&first),
            &mut none,
        )
        .unwrap();
    assert_eq!(none.spent, 0);
    projected_parents::assert_rows(&scratch, &first, &Narrower::new(&first));
    projected_heads::assert_rows(&scratch, &first, &producers(&first));
    assert_eq!(
        scratch.prepare_bodies(
            &second,
            &Narrower::new(&second),
            &producers(&second),
            &mut Work::new(0)
        ),
        Err(Stop::WorkLimit)
    );
    assert!(
        scratch.owner.is_none(),
        "failed owner change cannot retain old authority"
    );
    assert!(scratch.projected.is_none());
    scratch.prepare_projected(&second, &Narrower::new(&second), &producers(&second));
    assert!(scratch.owner.as_ref().unwrap().same_instance(&second));
    assert!(!scratch.owner.as_ref().unwrap().same_instance(&first));
    projected_parents::assert_rows(&scratch, &second, &Narrower::new(&second));
    projected_heads::assert_rows(&scratch, &second, &producers(&second));
}

#[test]
fn interrupted_projected_rounds_reinitialize_every_active_cell() {
    let owner = projected_bodies();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let never = vec![0; flag_words(owner.nodes().len())];
    let mut prepared = Scratch::default();
    prepared.prepare_projected(&owner, &index, &producers);
    let mut complete = Work::new(u64::MAX);
    prepared
        .complete(&owner, &index, &producers, &never, &[0], &mut complete)
        .unwrap();
    let expected = prepared.atoms.clone();
    for ceiling in 0..complete.spent {
        let mut scratch = Scratch::default();
        scratch.prepare_projected(&owner, &index, &producers);
        let mut limited = Work::new(ceiling);
        assert_eq!(
            scratch.complete(&owner, &index, &producers, &never, &[0], &mut limited),
            Err(Stop::WorkLimit)
        );
        assert_eq!(limited.spent, ceiling);
        scratch
            .complete(
                &owner,
                &index,
                &producers,
                &never,
                &[0],
                &mut Work::new(u64::MAX),
            )
            .unwrap();
        assert_eq!(scratch.atoms, expected);
    }
}
