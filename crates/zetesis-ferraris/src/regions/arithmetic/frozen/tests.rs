use zetesis_algebra::{Cancellation, Meter, Stop};

use super::super::budget::Budget;
use super::proposals::{Proposals, propose};
use crate::test_support::{Allowance, constructed_theory, push};
use crate::{
    EvaluationLimits, EvaluationWorkspace, Interpretation, NodeView, Producers, Region,
    RegionLimits, Theory, producers,
};

/// The saturation shape of the quantified fixtures over `n` universal
/// variables: roots `y(j) ∨ ny(j)`; per clause `c` a chain `fl(c, k) ←
/// falsifier` and `f(c) ← ⋀ fl(c, k)`; `valid ← current ∧ f(c)` with
/// `current` a fact; and `y(j) ← valid`, `ny(j) ← valid`. A clause lists
/// `(j, held)`: it is falsified when `y(j)` is held (`true`) or cut (`false`),
/// the latter read through `ny(j)`. Masked clauses hang off `valid ← other ∧
/// f(c)` with `other` outside the candidate, so their rule is false in it.
struct Saturation {
    theory: Theory,
    candidate: Interpretation,
    truth: Vec<bool>,
    valid: usize,
    current: usize,
}

impl Saturation {
    fn build(n: usize, clauses: &[Vec<(usize, bool)>], masked: &[Vec<(usize, bool)>]) -> Self {
        let (valid, current, other) = (2 * n, 2 * n + 1, 2 * n + 2);
        let all: Vec<&Vec<(usize, bool)>> = clauses.iter().chain(masked).collect();
        let f = |c: usize| other + 1 + c;
        let mut fl_base = Vec::new();
        let mut next = other + 1 + all.len();
        for clause in &all {
            fl_base.push(next);
            next += clause.len();
        }
        let atoms = next;
        let theory = constructed_theory(atoms, |nodes| {
            let mut roots = Vec::new();
            for j in 0..n {
                roots.push(push(nodes, NodeView::Or(&[j, n + j])));
            }
            roots.push(current);
            for (c, clause) in all.iter().enumerate() {
                let mut conjuncts = Vec::new();
                for (k, &(j, held)) in clause.iter().enumerate() {
                    let falsifier = if held { j } else { n + j };
                    let literal = fl_base[c] + k;
                    roots.push(push(nodes, NodeView::Implies(falsifier, literal)));
                    conjuncts.push(literal);
                }
                let body = push(nodes, NodeView::And(&conjuncts));
                roots.push(push(nodes, NodeView::Implies(body, f(c))));
                let gate = if c < clauses.len() { current } else { other };
                let body = push(nodes, NodeView::And(&[gate, f(c)]));
                roots.push(push(nodes, NodeView::Implies(body, valid)));
            }
            for j in 0..n {
                roots.push(push(nodes, NodeView::Implies(valid, j)));
                roots.push(push(nodes, NodeView::Implies(valid, n + j)));
            }
            roots
        });
        let candidate =
            Interpretation::new(&theory, (0..atoms).filter(|&atom| atom != other)).unwrap();
        let mut workspace = EvaluationWorkspace::default();
        let evaluated = workspace
            .evaluate(
                &candidate,
                EvaluationLimits::default(),
                &Cancellation::default(),
            )
            .result
            .unwrap();
        assert!(
            evaluated.is_model(),
            "the saturated candidate models the fixture"
        );
        let truth = evaluated.truth().to_vec();
        Self {
            theory,
            candidate,
            truth,
            valid,
            current,
        }
    }

    fn producers(&self) -> Producers {
        producers(
            &self.theory,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .producers
        .expect("every root of the fixture lies in the support fragment")
    }

    /// The query root closed by hand: every atom of the candidate open except
    /// the fact `current`, which the first frozen narrowing holds.
    fn closed_root(&self) -> Region {
        let mut root = Region::all_open(self.theory.atom_count());
        for atom in (0..self.theory.atom_count()).filter(|&atom| !self.candidate.contains(atom)) {
            root.cut(atom);
        }
        root.hold(self.current);
        root
    }

    fn propose(&self) -> Proposals {
        let producers = self.producers();
        let root = self.closed_root();
        let mut quota = Allowance {
            remaining: u64::MAX,
            failure: Stop::WorkLimit,
        };
        let mut meter = Meter::reserved(&mut quota);
        let cancellation = Cancellation::default();
        let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancellation);
        propose(
            &self.theory,
            &producers,
            &self.candidate,
            &self.truth,
            &root,
            &mut budget,
        )
        .unwrap()
    }
}

/// A gate's four clauses: the odd patterns over `(a, i, o)`, falsified when
/// each variable takes the pattern's value.
fn gate(a: usize, i: usize, o: usize) -> Vec<Vec<(usize, bool)>> {
    [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ]
    .into_iter()
    .map(|(sa, si, so)| vec![(a, sa), (i, si), (o, so)])
    .collect()
}

#[test]
fn the_saturation_atom_is_proposed_as_a_guard() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let proposals = fixture.propose();
    assert_eq!(proposals.guards, vec![fixture.valid]);
    assert_eq!(proposals.counts.guards, 1);
}

#[test]
fn a_gates_four_clauses_form_one_group_over_its_y_atoms() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let proposals = fixture.propose();
    assert_eq!(proposals.parity.len(), 1);
    let group = &proposals.parity[0];
    assert_eq!(group.guard, fixture.valid);
    assert_eq!(&group.support.atoms[..group.support.len], &[0, 1, 2]);
    let mut patterns: Vec<u8> = group.clauses.iter().map(|&(_, pattern)| pattern).collect();
    patterns.sort_unstable();
    assert_eq!(patterns, vec![0b001, 0b010, 0b100, 0b111]);
    assert_eq!(proposals.counts.groups, 1);
    assert_eq!(proposals.counts.clauses, 4);
}

#[test]
fn a_pairwise_exclusion_is_proposed_per_pair() {
    let pairs = vec![
        vec![(0, true), (1, true)],
        vec![(0, true), (2, true)],
        vec![(1, true), (2, true)],
    ];
    let fixture = Saturation::build(3, &pairs, &[]);
    let proposals = fixture.propose();
    let exclusions: Vec<(Vec<usize>, Vec<bool>)> = proposals
        .clauses
        .iter()
        .map(|clause| (clause.support.clone(), clause.held.clone()))
        .collect();
    assert_eq!(
        exclusions,
        vec![
            (vec![0, 1], vec![true, true]),
            (vec![0, 2], vec![true, true]),
            (vec![1, 2], vec![true, true]),
        ]
    );
    assert!(
        proposals
            .clauses
            .iter()
            .all(|clause| clause.guard == fixture.valid)
    );
}

#[test]
fn a_support_beyond_k_max_is_dropped_and_counted() {
    let wide: Vec<(usize, bool)> = (0..9).map(|j| (j, false)).collect();
    let fixture = Saturation::build(9, &[wide, vec![(0, true), (1, true)]], &[]);
    let proposals = fixture.propose();
    assert_eq!(proposals.parity.len(), 1);
    assert_eq!(&proposals.parity[0].support.atoms[..2], &[0, 1]);
    assert_eq!(proposals.counts.unsupported_clauses, 1);
    assert_eq!(proposals.clauses.len(), 2);
    assert_eq!(proposals.clauses[0].support.len(), 9);
    assert!(proposals.clauses[0].held.iter().all(|&held| !held));
}

#[test]
fn proposals_read_only_nodes_true_in_the_candidate() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[vec![(0, true), (1, false)]]);
    let proposals = fixture.propose();
    assert_eq!(proposals.clauses.len(), 4);
    assert!(
        proposals
            .clauses
            .iter()
            .all(|clause| clause.support == vec![0, 1, 2])
    );
}
