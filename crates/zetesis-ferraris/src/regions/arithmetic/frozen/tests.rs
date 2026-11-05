use zetesis_algebra::{Cancellation, Countermodels, Failure, Meter, Stop};

use super::super::budget::Budget;
use super::proposals::{Proposals, propose};
use super::{ClosedQuery, FrozenPreparation, FrozenPreparationAttempt, FrozenPreparationLimits};
use crate::test_support::{Allowance, constructed_theory, push};
use crate::{
    EvaluationLimits, EvaluationWorkspace, FrozenSubject, Interpretation, Knowledge, Narrower,
    Narrowing, NarrowingScratch, NodeView, Producers, Region, RegionLimits, Theory, producers,
};

/// The saturation shape of the quantified fixtures over `n` universal
/// variables: roots `y(j) ∨ ny(j)`; per clause `c` a chain `fl(c, k) ←
/// falsifier` and `f(c) ← ⋀ fl(c, k)`; `valid ← current ∧ f(c)` with
/// `current` a fact; and `y(j) ← valid`, `ny(j) ← valid`. A clause lists
/// `(j, held)`: it is falsified when `y(j)` is held (`true`) or cut (`false`),
/// the latter read through `ny(j)`. Masked clauses hang off `valid ← other ∧
/// f(c)` with `other` outside the candidate, so their rule is false in it.
/// Loose clauses hang off `loose ← current ∧ f(c)`, a guard nothing
/// saturates from; without them `loose` has no rule and stays outside the
/// candidate.
struct Saturation {
    theory: Theory,
    candidate: Interpretation,
    truth: Vec<bool>,
    valid: usize,
    current: usize,
    loose: usize,
}

impl Saturation {
    fn build(n: usize, clauses: &[Vec<(usize, bool)>], masked: &[Vec<(usize, bool)>]) -> Self {
        Self::build_with(n, clauses, masked, &[])
    }

    fn build_with(
        n: usize,
        clauses: &[Vec<(usize, bool)>],
        masked: &[Vec<(usize, bool)>],
        loose_clauses: &[Vec<(usize, bool)>],
    ) -> Self {
        let (valid, current, other, loose) = (2 * n, 2 * n + 1, 2 * n + 2, 2 * n + 3);
        let all: Vec<&Vec<(usize, bool)>> =
            clauses.iter().chain(masked).chain(loose_clauses).collect();
        let f = |c: usize| loose + 1 + c;
        let mut fl_base = Vec::new();
        let mut next = loose + 1 + all.len();
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
                let (gate, head) = if c < clauses.len() {
                    (current, valid)
                } else if c < clauses.len() + masked.len() {
                    (other, valid)
                } else {
                    (current, loose)
                };
                let body = push(nodes, NodeView::And(&[gate, f(c)]));
                roots.push(push(nodes, NodeView::Implies(body, head)));
            }
            for j in 0..n {
                roots.push(push(nodes, NodeView::Implies(valid, j)));
                roots.push(push(nodes, NodeView::Implies(valid, n + j)));
            }
            roots
        });
        let unused = |atom: usize| atom == other || (atom == loose && loose_clauses.is_empty());
        let candidate =
            Interpretation::new(&theory, (0..atoms).filter(|&atom| !unused(atom))).unwrap();
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
            loose,
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

    fn subject(&self) -> FrozenSubject<'_> {
        FrozenSubject::new(&self.theory, &self.truth)
    }

    /// The query root: every atom of the candidate open, every other cut.
    fn query_root(&self) -> Region {
        let mut root = Region::all_open(self.theory.atom_count());
        for atom in (0..self.theory.atom_count()).filter(|&atom| !self.candidate.contains(atom)) {
            root.cut(atom);
        }
        root
    }

    /// The query root closed by hand: the fact `current` held, as the first
    /// frozen narrowing holds it.
    fn closed_root(&self) -> Region {
        let mut root = self.query_root();
        root.hold(self.current);
        root
    }

    /// The query root closed by the frozen narrowing, with its knowledge.
    fn closed(&self) -> (Narrower, Region, Knowledge<Countermodels>) {
        let narrower = Narrower::new(&self.theory);
        let mut root = self.query_root();
        let mut knowledge = narrower.frozen_knowledge();
        let mut scratch = NarrowingScratch::default();
        let mut quota = Allowance {
            remaining: u64::MAX,
            failure: Stop::WorkLimit,
        };
        let mut meter = Meter::reserved(&mut quota);
        let attempt = narrower.narrow_frozen_known_reserved(
            self.subject(),
            &mut root,
            &mut knowledge,
            &mut scratch,
            &Cancellation::default(),
            &mut meter,
        );
        assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
        assert!(root.is_held(self.current));
        (narrower, root, knowledge)
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

    /// Prepare under `limits` with `allowance` permits from the account; the
    /// attempt and the permits the account granted and kept.
    fn prepare(
        &self,
        limits: FrozenPreparationLimits,
        allowance: u64,
        cancellation: &Cancellation,
    ) -> (FrozenPreparationAttempt<Failure<Stop>>, u64) {
        let producers = self.producers();
        let (narrower, root, knowledge) = self.closed();
        let mut scratch = NarrowingScratch::default();
        let mut quota = Allowance {
            remaining: allowance,
            failure: Stop::WorkLimit,
        };
        let query = ClosedQuery {
            narrower: &narrower,
            producers: Some(&producers),
            subject: self.subject(),
            candidate: &self.candidate,
            region: &root,
            knowledge: &knowledge,
        };
        let attempt = FrozenPreparation::prepare_reserved(
            query,
            &mut scratch,
            limits,
            cancellation,
            &mut quota,
        );
        (attempt, allowance - quota.remaining)
    }

    fn prepared(&self) -> FrozenPreparationAttempt<Failure<Stop>> {
        self.prepare(unbounded(), u64::MAX, &Cancellation::default())
            .0
    }
}

fn unbounded() -> FrozenPreparationLimits {
    FrozenPreparationLimits {
        work: u64::MAX,
        bytes: usize::MAX,
    }
}

/// The two chains of three inputs computing the same parity, whose results
/// `y(4)` and `y(6)` must differ: a contradiction over seven universals.
fn two_chains() -> Vec<Vec<(usize, bool)>> {
    let mut clauses = gate(0, 1, 3);
    clauses.extend(gate(3, 2, 4));
    clauses.extend(gate(1, 2, 5));
    clauses.extend(gate(5, 0, 6));
    clauses.push(vec![(4, false), (6, false)]);
    clauses.push(vec![(4, true), (6, true)]);
    clauses
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

#[test]
fn holding_the_saturation_atom_reaches_the_candidate_so_it_is_a_unit() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(preparation.units(), &[fixture.valid]);
    assert_eq!(attempt.statistics.saturation_units, 1);
    assert_eq!(attempt.statistics.guards, 1);
}

#[test]
fn the_odd_patterns_of_a_gate_are_refuted_and_the_hull_is_exact() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.probes, 9);
    assert_eq!(attempt.statistics.refuted_patterns, 4);
    assert_eq!(attempt.statistics.inexact_hulls, 0);
    assert_eq!(attempt.statistics.parity.rows, 1);
    assert_eq!(attempt.statistics.parity.wider, 1);
    assert!(!preparation.root_refuted());
    assert!(preparation.parity().is_some());
}

#[test]
fn a_dropped_clause_leaves_an_inexact_hull_and_no_row() {
    let mut clauses = gate(0, 1, 2);
    clauses.pop();
    let fixture = Saturation::build(3, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.refuted_patterns, 3);
    assert_eq!(attempt.statistics.inexact_hulls, 1);
    assert_eq!(attempt.statistics.parity.rows, 0);
    assert!(preparation.parity().is_none());
}

#[test]
fn two_chains_whose_results_must_differ_refute_the_root() {
    let fixture = Saturation::build(7, &two_chains(), &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.parity.rows, 5);
    assert!(preparation.root_refuted());
    assert!(attempt.statistics.root_refuted);
}

#[test]
fn parity_groups_under_an_open_guard_are_dropped() {
    let fixture = Saturation::build_with(3, &gate(0, 1, 2), &[], &gate(0, 1, 2));
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.guards, 2);
    assert_eq!(attempt.statistics.saturation_units, 1);
    assert_eq!(preparation.units(), &[fixture.valid]);
    assert_eq!(attempt.statistics.groups, 2);
    assert_eq!(attempt.statistics.probed_groups, 1);
    assert_eq!(attempt.statistics.parity.rows, 1);
    assert!(fixture.loose > fixture.valid);
}

#[test]
fn a_lease_crossed_mid_probe_declines_the_preparation() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let complete = fixture.prepared();
    let work = complete.work;
    assert!(work > 0);
    let stride = (work / 400).max(1);
    for lease in (1..work).step_by(usize::try_from(stride).unwrap()) {
        let limits = FrozenPreparationLimits {
            work: lease,
            bytes: usize::MAX,
        };
        let (attempt, charged) = fixture.prepare(limits, u64::MAX, &Cancellation::default());
        assert!(attempt.result.as_ref().unwrap().is_none(), "lease {lease}");
        assert_eq!(
            attempt.statistics.status,
            super::super::PreparationStatus::WorkDeclined
        );
        assert!(
            attempt.work <= lease,
            "lease {lease}: charged {}",
            attempt.work
        );
        assert_eq!(attempt.work, charged);
    }
}

#[test]
fn a_stopped_probe_stops_the_preparation() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let work = fixture.prepared().work;
    let (attempt, charged) = fixture.prepare(unbounded(), work / 2, &Cancellation::default());
    assert_eq!(attempt.work, charged);
    assert!(matches!(
        attempt.result,
        Err(super::super::PreparationError::Stopped(Failure::Refused(
            Stop::WorkLimit
        )))
    ));
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let (attempt, _) = fixture.prepare(unbounded(), u64::MAX, &cancellation);
    assert!(matches!(
        attempt.result,
        Err(super::super::PreparationError::Stopped(Failure::Stopped(
            Stop::Cancelled
        )))
    ));
}

#[test]
fn a_preparation_belongs_to_one_candidate() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let preparation = fixture.prepared().result.unwrap().unwrap();
    assert!(preparation.belongs_to(&fixture.theory, &fixture.candidate));
    let smaller = Interpretation::new(&fixture.theory, fixture.candidate.atoms().skip(1)).unwrap();
    assert!(!preparation.belongs_to(&fixture.theory, &smaller));
    let twin = Saturation::build(3, &gate(0, 1, 2), &[]);
    assert!(!preparation.belongs_to(&twin.theory, &twin.candidate));
}

#[test]
fn probing_charges_the_lease_and_leaves_the_root_untouched() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let (_, root, _) = fixture.closed();
    let before: Vec<Option<bool>> = (0..fixture.theory.atom_count())
        .map(|atom| root.decision(atom))
        .collect();
    let (attempt, charged) = fixture.prepare(unbounded(), u64::MAX, &Cancellation::default());
    assert!(attempt.result.unwrap().is_some());
    assert_eq!(attempt.work, charged);
    assert!(attempt.work > 0);
    let (_, root, _) = fixture.closed();
    let after: Vec<Option<bool>> = (0..fixture.theory.atom_count())
        .map(|atom| root.decision(atom))
        .collect();
    assert_eq!(before, after);
}

/// A guard whose clause bodies are nested conjunction pairs, as a formula
/// builder without wide spans writes them: `valid ← y(0) ∧ (y(1) ∧ y(2))`
/// and `valid ← ny(0) ∧ (y(1) ∧ ny(2))`, under `y(j) ∨ ny(j)` roots and
/// saturation from `valid`.
fn nested_pairs() -> (Theory, Interpretation, Vec<bool>) {
    let n = 3;
    let valid = 2 * n;
    let theory = constructed_theory(valid + 1, |nodes| {
        let mut roots = Vec::new();
        for j in 0..n {
            roots.push(push(nodes, NodeView::Or(&[j, n + j])));
            roots.push(push(nodes, NodeView::Implies(valid, j)));
            roots.push(push(nodes, NodeView::Implies(valid, n + j)));
        }
        for literals in [[0, 1, 2], [n, 1, n + 2]] {
            let inner = push(nodes, NodeView::And(&[literals[1], literals[2]]));
            let body = push(nodes, NodeView::And(&[literals[0], inner]));
            roots.push(push(nodes, NodeView::Implies(body, valid)));
        }
        roots
    });
    let candidate = Interpretation::new(&theory, 0..theory.atom_count()).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace
        .evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let truth = evaluated.truth().to_vec();
    (theory, candidate, truth)
}

#[test]
fn a_nested_conjunction_body_is_read_through() {
    let (theory, candidate, truth) = nested_pairs();
    let producers = producers(&theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    let root = Region::all_open(theory.atom_count());
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut meter = Meter::reserved(&mut quota);
    let cancellation = Cancellation::default();
    let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancellation);
    let proposals = propose(&theory, &producers, &candidate, &truth, &root, &mut budget).unwrap();
    assert_eq!(proposals.guards, vec![6]);
    let mut supports: Vec<(Vec<usize>, Vec<bool>)> = proposals
        .clauses
        .iter()
        .map(|clause| (clause.support.clone(), clause.held.clone()))
        .collect();
    supports.sort();
    assert_eq!(
        supports,
        vec![
            (vec![0, 1, 2], vec![false, true, false]),
            (vec![0, 1, 2], vec![true, true, true]),
        ]
    );
    assert_eq!(proposals.parity.len(), 1);
}
