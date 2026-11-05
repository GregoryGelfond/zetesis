use zetesis_algebra::{Cancellation, Countermodels, Failure, Meter, Stop};

use super::super::budget::Budget;
use super::proposals::{Proposals, propose};
use super::{ClosedQuery, FrozenPreparation, FrozenPreparationAttempt, FrozenPreparationLimits};
use crate::test_support::{Allowance, constructed_theory, push};
use crate::{
    EvaluationLimits, EvaluationWorkspace, FrozenSubject, Interpretation, Knowledge, Narrower,
    Narrowing, NarrowingScratch, NodeView, Producers, Region, RegionLimits, Theory, Threshold,
    producers,
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
        Self::build_with(n, clauses, masked, &[], false)
    }

    /// With `facts`, the shape the grounder writes: a tautology, the fact
    /// `universal(j)` simplified away, guards each disjunction
    /// `y(j) ∨ ny(j) ← (⊥ → ⊥)`; the fact
    /// `literal(c, k)` joins the body of `fl(c, k)`, which also keeps the
    /// other polarity's rule, `fl(c, k) ← other ∧ falsifier'`, false in the
    /// candidate; and `f(c)`'s body is the conditional literal's expansion,
    /// `clause(c) ∧ ⋀ (literal(c, k) → fl(c, k))`, nested as left-recursive
    /// pairs.
    fn build_with(
        n: usize,
        clauses: &[Vec<(usize, bool)>],
        masked: &[Vec<(usize, bool)>],
        loose_clauses: &[Vec<(usize, bool)>],
        facts: bool,
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
        // The fact atoms, when present: `universal(j)`, then one per
        // literal, then one per clause.
        let universal = |j: usize| next + j;
        let literal_fact = |c: usize, k: usize| next + n + fl_base[c] - fl_base[0] + k;
        let clause_fact = |c: usize| {
            next + n + (fl_base[all.len() - 1] - fl_base[0]) + all[all.len() - 1].len() + c
        };
        let atoms = if facts {
            clause_fact(all.len() - 1) + 1
        } else {
            next
        };
        let theory = constructed_theory(atoms, |nodes| {
            let mut roots = Vec::new();
            let falsum = push(nodes, NodeView::False);
            let tautology = push(nodes, NodeView::Implies(falsum, falsum));
            for j in 0..n {
                let pair = push(nodes, NodeView::Or(&[j, n + j]));
                if facts {
                    roots.push(universal(j));
                    roots.push(push(nodes, NodeView::Implies(tautology, pair)));
                } else {
                    roots.push(pair);
                }
            }
            roots.push(current);
            for (c, clause) in all.iter().enumerate() {
                let mut conjuncts = Vec::new();
                if facts {
                    roots.push(clause_fact(c));
                    conjuncts.push(clause_fact(c));
                }
                for (k, &(j, held)) in clause.iter().enumerate() {
                    let falsifier = if held { j } else { n + j };
                    let literal = fl_base[c] + k;
                    let body = if facts {
                        roots.push(literal_fact(c, k));
                        let masked = if held { n + j } else { j };
                        let dead = push(nodes, NodeView::And(&[other, masked]));
                        roots.push(push(nodes, NodeView::Implies(dead, literal)));
                        push(nodes, NodeView::And(&[literal_fact(c, k), falsifier]))
                    } else {
                        falsifier
                    };
                    roots.push(push(nodes, NodeView::Implies(body, literal)));
                    if facts {
                        conjuncts.push(push(nodes, NodeView::Implies(literal_fact(c, k), literal)));
                    } else {
                        conjuncts.push(literal);
                    }
                }
                let body = if facts {
                    conjuncts[1..].iter().fold(conjuncts[0], |left, &right| {
                        push(nodes, NodeView::And(&[left, right]))
                    })
                } else {
                    push(nodes, NodeView::And(&conjuncts))
                };
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
        let (candidate, truth) = saturated(&theory, |atom| !unused(atom));
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

    /// What the root's first closure charges.
    fn closure_work(&self) -> u64 {
        let narrower = Narrower::new(&self.theory);
        let mut root = self.query_root();
        let mut knowledge = narrower.frozen_knowledge();
        let mut quota = Allowance {
            remaining: u64::MAX,
            failure: Stop::WorkLimit,
        };
        let mut meter = Meter::reserved(&mut quota);
        let attempt = narrower.narrow_frozen_known_reserved(
            self.subject(),
            &mut root,
            &mut knowledge,
            &mut NarrowingScratch::default(),
            &Cancellation::default(),
            &mut meter,
        );
        attempt.statistics.work
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
        let (_, root, _) = self.closed();
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
            closure_work: self.closure_work(),
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

/// The candidate holding every atom `included`, which models `theory`, and
/// its node truth.
fn saturated(theory: &Theory, included: impl Fn(usize) -> bool) -> (Interpretation, Vec<bool>) {
    let candidate = Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|&atom| included(atom)),
    )
    .unwrap();
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
    (candidate, truth)
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
    let fixture = Saturation::build_with(3, &gate(0, 1, 2), &[], &gate(0, 1, 2), false);
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
fn the_lease_floor_covers_the_proposals_and_each_round_pays_for_its_probes() {
    // The floor must cover the proposal pass and the rows' loading; each
    // probing round leases its probes times the narrower's work on top, so a
    // floor below the whole preparation's work still prepares, while a floor
    // below the proposal pass declines it with nothing installed.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let complete = fixture.prepared();
    let work = complete.work;
    assert!(work > 0);
    let floor = FrozenPreparationLimits {
        work: work / 2,
        bytes: usize::MAX,
    };
    let (attempt, charged) = fixture.prepare(floor, u64::MAX, &Cancellation::default());
    assert!(attempt.result.unwrap().is_some());
    assert_eq!(attempt.work, charged);
    assert!(attempt.work > floor.work);
    let tiny = FrozenPreparationLimits {
        work: 10,
        bytes: usize::MAX,
    };
    let (attempt, charged) = fixture.prepare(tiny, u64::MAX, &Cancellation::default());
    assert!(attempt.result.as_ref().unwrap().is_none());
    assert_eq!(
        attempt.statistics.status,
        super::super::PreparationStatus::WorkDeclined
    );
    assert_eq!(attempt.work, charged);
    assert!(attempt.work <= 10);
}

#[test]
fn a_later_round_reprobes_only_the_inexact_groups() {
    // Two chains less one clause: round one probes the unit and every group
    // (four gates and the differing pair, 4 · 8 + 4 patterns); round two
    // probes only the inexact gate's eight patterns.
    let mut clauses = two_chains();
    clauses.remove(0);
    let fixture = Saturation::build(7, &clauses, &[]);
    let attempt = fixture.prepared();
    assert_eq!(attempt.statistics.rounds, 2);
    assert_eq!(attempt.statistics.probes, 1 + 4 * 8 + 4 + 8);
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

/// A chain through an atom the reader cannot read through, as the grounder
/// defines its `#count` facts: `index ← fact ∧ (1·fact ≥ 1)`, with both
/// clauses of `valid` passing through `index`. The chain stops there as at a
/// base, and the root's decision rules as for any base: decided true, it
/// contributes nothing, and the clauses' support is the universal atoms alone.
#[test]
fn an_unreadable_definition_decided_at_the_root_contributes_nothing() {
    let n = 3;
    let (valid, current, fact, index, f_a, f_b) = (6, 7, 8, 9, 10, 11);
    let theory = constructed_theory(12, |nodes| {
        let mut roots = Vec::new();
        for j in 0..n {
            roots.push(push(nodes, NodeView::Or(&[j, n + j])));
            roots.push(push(nodes, NodeView::Implies(valid, j)));
            roots.push(push(nodes, NodeView::Implies(valid, n + j)));
        }
        roots.push(current);
        roots.push(fact);
        let threshold = push(
            nodes,
            NodeView::Threshold(Threshold {
                conditions: &[fact],
                weights: &[1],
                bound: 1,
            }),
        );
        let body = push(nodes, NodeView::And(&[fact, threshold]));
        roots.push(push(nodes, NodeView::Implies(body, index)));
        for (head, literals) in [(f_a, [0, 1, 2]), (f_b, [n, n + 1, 2])] {
            let body = push(
                nodes,
                NodeView::And(&[index, literals[0], literals[1], literals[2]]),
            );
            roots.push(push(nodes, NodeView::Implies(body, head)));
            let body = push(nodes, NodeView::And(&[current, head]));
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
    let producers = producers(&theory, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    // The query root closed by the frozen reading decides the fact-defined
    // atoms and leaves the guard open.
    let narrower = Narrower::new(&theory);
    let mut root = Region::all_open(theory.atom_count());
    let mut knowledge = narrower.frozen_knowledge();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut meter = Meter::reserved(&mut quota);
    let cancellation = Cancellation::default();
    let attempt = narrower.narrow_frozen_known_reserved(
        FrozenSubject::new(&theory, &truth),
        &mut root,
        &mut knowledge,
        &mut NarrowingScratch::default(),
        &cancellation,
        &mut meter,
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
    assert_eq!(
        (root.decision(index), root.decision(valid)),
        (Some(true), None)
    );
    let mut budget = Budget::new(&mut meter, u64::MAX, usize::MAX, &cancellation);
    let proposals = propose(&theory, &producers, &candidate, &truth, &root, &mut budget).unwrap();
    assert_eq!(proposals.guards, vec![valid]);
    let mut supports: Vec<(Vec<usize>, Vec<bool>)> = proposals
        .clauses
        .iter()
        .map(|clause| (clause.support.clone(), clause.held.clone()))
        .collect();
    supports.sort();
    assert_eq!(
        supports,
        vec![
            (vec![0, 1, 2], vec![false, false, true]),
            (vec![0, 1, 2], vec![true, true, true]),
        ]
    );
    assert_eq!(proposals.parity.len(), 1);
}

/// Prepare from the closed root with `decide` applied and closed again: a
/// residual root a walk or a device handed over.
fn prepared_from(
    fixture: &Saturation,
    decide: impl FnOnce(&mut Region),
) -> FrozenPreparationAttempt<Failure<Stop>> {
    let producers = fixture.producers();
    let (narrower, mut root, mut knowledge) = fixture.closed();
    decide(&mut root);
    let mut scratch = NarrowingScratch::default();
    let cancellation = Cancellation::default();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let closed = {
        let mut meter = Meter::reserved(&mut quota);
        narrower.narrow_frozen_known_reserved(
            fixture.subject(),
            &mut root,
            &mut knowledge,
            &mut scratch,
            &cancellation,
            &mut meter,
        )
    };
    assert_ne!(closed.result.unwrap(), Narrowing::Refuted);
    let query = ClosedQuery {
        narrower: &narrower,
        producers: Some(&producers),
        subject: fixture.subject(),
        candidate: &fixture.candidate,
        region: &root,
        knowledge: &knowledge,
        closure_work: fixture.closure_work(),
    };
    FrozenPreparation::prepare_reserved(query, &mut scratch, unbounded(), &cancellation, &mut quota)
}

#[test]
fn a_guard_cut_at_the_root_needs_no_probe() {
    // A residual root where a split or a device already cut the guard: its
    // clauses hold there unconditionally, so the groups are probed without a
    // unit probe and nothing is cut again.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let valid = fixture.valid;
    let attempt = prepared_from(&fixture, |root| assert!(root.cut(valid)));
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.guards, 1);
    assert_eq!(attempt.statistics.probes, 8);
    assert_eq!(attempt.statistics.saturation_units, 0);
    assert!(preparation.units().is_empty());
    assert_eq!(attempt.statistics.parity.rows, 1);
}

#[test]
fn fact_bodies_and_guarded_disjunctions_do_not_split_the_groups() {
    // The grounder's shape: facts in the bodies, a fact guarding each
    // disjunction, and a masked second definition of each literal atom. The
    // facts, held at the closed root, contribute no base atom; the guarded
    // disjunction still pairs `y(j)` with `ny(j)`; and the masked rule,
    // absent from the reduct, leaves the literal atom one live definition.
    let plain = Saturation::build(3, &gate(0, 1, 2), &[]);
    let grounded = Saturation::build_with(3, &gate(0, 1, 2), &[], &[], true);
    let expected: Vec<(Vec<usize>, Vec<bool>)> = plain
        .propose()
        .clauses
        .iter()
        .map(|clause| (clause.support.clone(), clause.held.clone()))
        .collect();
    let actual: Vec<(Vec<usize>, Vec<bool>)> = grounded
        .propose()
        .clauses
        .iter()
        .map(|clause| (clause.support.clone(), clause.held.clone()))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(grounded.propose().parity.len(), 1);
    let attempt = grounded.prepared();
    assert_eq!(attempt.statistics.parity.rows, 1);
    assert_eq!(attempt.statistics.refuted_patterns, 4);
}

#[test]
fn a_deeply_nested_conjunction_is_read_through() {
    // Nine literals nested as pairs, with the fact guard and the conditional
    // literal's implications: a nesting past any producer-chain bound, which
    // the reader follows to every base atom.
    let wide: Vec<(usize, bool)> = (0..9).map(|j| (j, j % 2 == 0)).collect();
    let fixture = Saturation::build_with(9, &[wide, vec![(0, true), (1, true)]], &[], &[], true);
    let proposals = fixture.propose();
    assert_eq!(proposals.clauses.len(), 2);
    assert_eq!(proposals.clauses[0].support, (0..9).collect::<Vec<_>>());
    assert_eq!(proposals.counts.unsupported_clauses, 1);
    assert_eq!(proposals.parity.len(), 1);
}

#[test]
fn a_second_probing_round_under_the_rows_narrows_the_dropped_gate() {
    // Two chains whose results must differ, less one clause of the first
    // gate: the frozen subject alone allows five patterns at that gate, so
    // its hull is inexact. Under the other gates' rows and the differing
    // results, every even pattern at that gate is refuted, so the second
    // round leaves exactly the dropped clause's pattern, three units.
    let mut clauses = two_chains();
    clauses.remove(0);
    let fixture = Saturation::build(7, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert!(!preparation.root_refuted());
    // The second round made every hull exact, so no third round ran.
    assert_eq!(attempt.statistics.rounds, 2);
    assert_eq!(attempt.statistics.inexact_hulls, 0);
    let units: Vec<(usize, bool)> = preparation.parity().expect("rows").units().collect();
    assert!(units.iter().any(|&(atom, _)| atom == 0), "{units:?}");
    assert!(units.iter().any(|&(atom, _)| atom == 1), "{units:?}");
    assert!(units.iter().any(|&(atom, _)| atom == 3), "{units:?}");
}

#[test]
fn the_rows_decide_consistency_under_the_regions_decisions() {
    // Rows y0 ⊕ y1 ⊕ y2 = 0 and y2 ⊕ y3 = 0 under the cut guard: deciding
    // y0 held, y1 and y2 cut, y3 held contradicts the first row; y2 held
    // with y3 cut contradicts the second; y0 held alone is consistent, and
    // so is y0, y1 held with y2, y3 cut.
    let mut clauses = gate(0, 1, 2);
    clauses.extend(gate(0, 1, 3));
    let fixture = Saturation::build(4, &clauses, &[]);
    let valid = fixture.valid;
    let attempt = prepared_from(&fixture, |root| assert!(root.cut(valid)));
    let preparation = attempt.result.unwrap().unwrap();
    let system = preparation.parity().expect("rows");
    let (_, closed, _) = fixture.closed();
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut meter = Meter::reserved(&mut quota);
    for (decisions, consistent) in [
        (vec![(0, true), (1, false), (2, false), (3, true)], false),
        (vec![(2, true), (3, false)], false),
        (vec![(0, true)], true),
        (vec![(0, true), (1, true), (2, false), (3, false)], true),
    ] {
        let mut region = closed.clone();
        assert!(region.cut(valid));
        for &(atom, held) in &decisions {
            assert!(if held {
                region.hold(atom)
            } else {
                region.cut(atom)
            });
        }
        assert_eq!(
            system.consistent_in_reserved(&region, &mut meter).unwrap(),
            consistent,
            "{decisions:?}"
        );
    }
}

/// Two chains over `n` inputs computing the same parity, the second over the
/// inputs in reverse, whose results must differ: inputs `0..n`, the first
/// chain's outputs `n..2n - 1`, the second's `2n - 1..3n - 2`.
fn two_chains_of(n: usize) -> Vec<Vec<(usize, bool)>> {
    let mut clauses = gate(0, 1, n);
    for k in 2..n {
        clauses.extend(gate(n + k - 2, k, n + k - 1));
    }
    clauses.extend(gate(n - 1, n - 2, 2 * n - 1));
    for k in 2..n {
        clauses.extend(gate(2 * n + k - 3, n - 1 - k, 2 * n + k - 2));
    }
    clauses.push(vec![(2 * n - 2, false), (3 * n - 3, false)]);
    clauses.push(vec![(2 * n - 2, true), (3 * n - 3, true)]);
    clauses
}

#[test]
fn the_second_round_pins_the_dropped_gate_at_every_width() {
    // The same linear argument at every size: the sum of the remaining gate
    // rows and the differing results says the dropped gate is odd, so its
    // even patterns are refuted under the rows and its hull becomes exact,
    // across the word boundary of the atom masks included.
    for n in [7, 12, 20, 21, 24] {
        let mut clauses = two_chains_of(n);
        clauses.remove(0);
        let fixture = Saturation::build(3 * n - 2, &clauses, &[]);
        let attempt = fixture.prepared();
        let preparation = attempt.result.unwrap().unwrap();
        assert!(!preparation.root_refuted(), "n={n}");
        assert_eq!(attempt.statistics.rounds, 2, "n={n}");
        assert_eq!(attempt.statistics.inexact_hulls, 0, "n={n}");
        let units: Vec<usize> = preparation
            .parity()
            .expect("rows")
            .units()
            .map(|(atom, _)| atom)
            .collect();
        for atom in [0, 1, n] {
            assert!(units.contains(&atom), "n={n}: units {units:?}");
        }
    }
}
