use zetesis_algebra::{Cancellation, Countermodels, Failure, Meter, Stop};

use super::super::budget::Budget;
use super::super::source::{MAX_SUPPORT, Support};
use super::super::uniform::BoundSide;
use super::proposals::{Group, Proposals, propose};
use super::{
    ClosedQuery, FrozenPreparation, FrozenPreparationAttempt, FrozenPreparationLimits, Leased,
    known_refutations,
};
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
    /// Per clause, in order, its literal atoms `fl(c, k)`.
    literals: Vec<Vec<usize>>,
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
        let literals = all
            .iter()
            .enumerate()
            .map(|(c, clause)| (0..clause.len()).map(|k| fl_base[c] + k).collect())
            .collect();
        Self {
            theory,
            candidate,
            truth,
            valid,
            current,
            loose,
            literals,
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

    /// Another candidate of the same theory, the atoms `included` admits
    /// saturated, with its truth.
    fn candidate(&self, included: impl Fn(usize) -> bool) -> (Interpretation, Vec<bool>) {
        saturated(&self.theory, included)
    }

    /// Prepare `candidate` of this theory from its own closed query root.
    fn prepared_for(
        &self,
        candidate: &Interpretation,
        truth: &[bool],
    ) -> FrozenPreparationAttempt<Failure<Stop>> {
        let producers = self.producers();
        let narrower = Narrower::new(&self.theory);
        let mut root = Region::all_open(self.theory.atom_count());
        for atom in (0..self.theory.atom_count()).filter(|&atom| !candidate.contains(atom)) {
            root.cut(atom);
        }
        let mut knowledge = narrower.frozen_knowledge();
        let mut scratch = NarrowingScratch::default();
        let cancellation = Cancellation::default();
        let mut quota = Allowance {
            remaining: u64::MAX,
            failure: Stop::WorkLimit,
        };
        let closure_work = {
            let mut meter = Meter::reserved(&mut quota);
            narrower
                .narrow_frozen_known_reserved(
                    FrozenSubject::new(&self.theory, truth),
                    &mut root,
                    &mut knowledge,
                    &mut scratch,
                    &cancellation,
                    &mut meter,
                )
                .statistics
                .work
        };
        let query = ClosedQuery {
            narrower: &narrower,
            producers: Some(&producers),
            subject: FrozenSubject::new(&self.theory, truth),
            candidate,
            region: &root,
            knowledge: &knowledge,
            closure_work,
        };
        FrozenPreparation::prepare_reserved(
            query,
            &mut scratch,
            unbounded(),
            &cancellation,
            &mut quota,
        )
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

/// Every clause the proposals group, as its support with the pattern it
/// forbids, sorted.
fn clause_patterns(proposals: &Proposals) -> Vec<(Vec<usize>, u8)> {
    let mut all: Vec<(Vec<usize>, u8)> = proposals
        .parity
        .iter()
        .flat_map(|group| {
            let support = group.support.atoms[..group.support.len].to_vec();
            group
                .patterns
                .iter()
                .map(move |&pattern| (support.clone(), pattern))
        })
        .collect();
    all.sort();
    all
}

/// Clauses written as a support and, per position, whether the atom is held
/// when the clause is falsified, as the patterns they forbid, sorted.
fn patterns_of(clauses: &[(Vec<usize>, Vec<bool>)]) -> Vec<(Vec<usize>, u8)> {
    let mut all: Vec<(Vec<usize>, u8)> = clauses
        .iter()
        .map(|(support, held)| {
            let pattern = held
                .iter()
                .enumerate()
                .fold(0_u8, |bits, (position, &held)| {
                    bits | (u8::from(held) << position)
                });
            (support.clone(), pattern)
        })
        .collect();
    all.sort();
    all
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
    let mut patterns = group.patterns.clone();
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
    assert_eq!(
        clause_patterns(&proposals),
        vec![(vec![0, 1], 0b11), (vec![0, 2], 0b11), (vec![1, 2], 0b11)]
    );
    assert!(
        proposals
            .parity
            .iter()
            .all(|group| group.guard == fixture.valid)
    );
}

#[test]
fn a_support_beyond_k_max_proposes_no_group_and_is_counted() {
    let wide: Vec<(usize, bool)> = (0..9).map(|j| (j, false)).collect();
    let fixture = Saturation::build(9, &[wide, vec![(0, true), (1, true)]], &[]);
    let proposals = fixture.propose();
    assert_eq!(proposals.parity.len(), 1);
    // All cut, the wide clause is kept to bound its support from below.
    assert_eq!(proposals.wide.len(), 1);
    assert_eq!(proposals.wide[0].support, (0..9).collect::<Vec<_>>());
    assert_eq!(&proposals.parity[0].support.atoms[..2], &[0, 1]);
    assert_eq!(proposals.counts.unsupported_clauses, 1);
    assert_eq!(proposals.counts.clauses, 2);
    // The receipt carries both counts, so a census tells a theory whose
    // clauses are all too wide from one with no saturation shape.
    let attempt = fixture.prepared();
    assert_eq!(
        (
            attempt.statistics.clauses,
            attempt.statistics.unsupported_clauses
        ),
        (2, 1)
    );
}

#[test]
fn proposals_read_only_nodes_true_in_the_candidate() {
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[vec![(0, true), (1, false)]]);
    let proposals = fixture.propose();
    assert_eq!(proposals.counts.clauses, 4);
    assert_eq!(proposals.parity.len(), 1);
    assert_eq!(&proposals.parity[0].support.atoms[..3], &[0, 1, 2]);
    assert_eq!(proposals.parity[0].patterns.len(), 4);
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
    // The guard and the four patterns the gate's clauses leave open; the odd
    // patterns are refuted by the clauses themselves and run no probe.
    assert_eq!(attempt.statistics.probes, 5);
    assert_eq!(attempt.statistics.refuted_patterns, 4);
    assert_eq!(attempt.statistics.inexact_hulls, 0);
    assert_eq!(attempt.statistics.parity.rows, 1);
    assert_eq!(attempt.statistics.parity.wider, 1);
    assert!(!preparation.root_refuted());
    assert!(preparation.parity().is_some());
}

#[test]
fn a_pattern_a_clause_of_the_group_forbids_needs_no_probe() {
    // Each gate's four clauses forbid four of its eight patterns outright:
    // probing them would only rediscover the clause. The probes are the
    // guard's and the other four patterns of each gate, and the pair's two
    // open patterns; the rows and the root's refutation are as before.
    let fixture = Saturation::build(7, &two_chains(), &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.probes, 1 + 4 * 4 + 2);
    assert_eq!(attempt.statistics.parity.rows, 5);
    assert!(preparation.root_refuted());
}

#[test]
fn a_pattern_holding_an_excluded_pair_needs_no_probe() {
    // Five pigeons over four holes: each pair clause forbids its two held,
    // which runs no probe, and each pair's other three patterns are probed.
    // A pigeon's clause, alone in its group, is left to itself in the first
    // round, and no later round runs without a parity row: one unit probe
    // and the pairs', with the pigeons' clauses and the holes' cliques as
    // the bounds and the double count refuting the root.
    let (n, clauses) = pigeonhole(4);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(attempt.statistics.probes, 1 + 40 * 3);
    assert_eq!(attempt.statistics.cardinality.rows, 5 + 4);
    assert!(preparation.root_refuted());
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
fn the_floor_is_exactly_the_preparations_unleased_share() {
    // The floor covers the proposal pass and the systems' loading; the
    // probes are leased on top, each at the root closure's cost plus one
    // index pass, which the receipt reports. A floor of exactly the unleased
    // share prepares, half of it declines with nothing installed, and a tiny
    // floor declines within itself.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let complete = fixture.prepared();
    let (work, leased) = (complete.work, complete.statistics.leased);
    assert!(leased > 0 && work > leased);
    let floor = FrozenPreparationLimits {
        work: work - leased,
        bytes: usize::MAX,
    };
    let (attempt, charged) = fixture.prepare(floor, u64::MAX, &Cancellation::default());
    assert!(attempt.result.unwrap().is_some());
    assert_eq!(attempt.work, charged);
    assert!(attempt.work > floor.work);
    // Every floor below the unleased share, at every interruption point of
    // the proposal pass and the loading: either declined with nothing
    // installed or prepared whole, never a part; the work settled as
    // charged; and once a floor prepares, every larger floor does, since the
    // probes' own lease covers at most their slack.
    let unleased = usize::try_from(work - leased).unwrap();
    let stride = (unleased / 256).max(1);
    let mut prepared_once = false;
    for floor in (0..unleased).step_by(stride) {
        let limits = FrozenPreparationLimits {
            work: u64::try_from(floor).unwrap(),
            bytes: usize::MAX,
        };
        let (attempt, charged) = fixture.prepare(limits, u64::MAX, &Cancellation::default());
        assert_eq!(attempt.work, charged, "floor {floor}");
        if attempt.result.as_ref().unwrap().is_some() {
            assert_eq!(
                attempt.statistics.status,
                super::super::PreparationStatus::Prepared,
                "floor {floor}"
            );
            prepared_once = true;
        } else {
            assert!(
                !prepared_once,
                "floor {floor} declines after a smaller floor prepared"
            );
            assert_eq!(
                attempt.statistics.status,
                super::super::PreparationStatus::WorkDeclined,
                "floor {floor}"
            );
        }
    }
    let half = FrozenPreparationLimits {
        work: (work - leased) / 2,
        bytes: usize::MAX,
    };
    let (attempt, charged) = fixture.prepare(half, u64::MAX, &Cancellation::default());
    assert!(attempt.result.as_ref().unwrap().is_none());
    assert_eq!(
        attempt.statistics.status,
        super::super::PreparationStatus::WorkDeclined
    );
    assert_eq!(attempt.work, charged);
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
    // Two chains less one clause: round one probes the unit and every
    // pattern no clause forbids (four for each whole gate, five for the gate
    // less a clause, two for the differing pair); round two probes only the
    // inexact gate's five again.
    let mut clauses = two_chains();
    clauses.remove(0);
    let fixture = Saturation::build(7, &clauses, &[]);
    let attempt = fixture.prepared();
    assert_eq!(attempt.statistics.rounds, 2);
    assert_eq!(attempt.statistics.probes, 1 + (3 * 4 + 5) + 2 + 5);
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
fn the_known_refutations_charge_each_pair_of_groups_compared() {
    // Two groups, the narrower's support inside the wider's: the floor is
    // charged one operation per clause pattern read, one per pair of groups
    // compared, whether or not the comparison reaches the containment test,
    // and one per pattern examined against a contained group's forbidden
    // pattern.
    let support = |atoms: &[usize]| {
        let mut support = Support {
            atoms: [0; MAX_SUPPORT],
            len: atoms.len(),
        };
        support.atoms[..atoms.len()].copy_from_slice(atoms);
        support
    };
    let wide = Group {
        guard: 9,
        support: support(&[0, 1, 2]),
        patterns: vec![0b001],
    };
    let narrow = Group {
        guard: 9,
        support: support(&[0, 1]),
        patterns: vec![0b01],
    };
    let mut quota = Allowance {
        remaining: u64::MAX,
        failure: Stop::WorkLimit,
    };
    let mut lease = Leased {
        account: &mut quota,
        remaining: u64::MAX,
        used: 0,
        extended: 0,
    };
    let known = known_refutations(&[&wide, &narrow], &mut lease).unwrap();
    let refuted =
        |table: &super::probes::Table| (0..8).filter(|&p| table.contains(p)).collect::<Vec<_>>();
    assert_eq!(
        known.iter().map(refuted).collect::<Vec<_>>(),
        vec![vec![1, 5], vec![1]]
    );
    // Two patterns read, four pairs compared, eight patterns examined.
    assert_eq!(lease.used, 2 + 4 + 8);
}

#[test]
fn probing_charges_the_lease() {
    // The probes narrow copies of the closed root, which the query borrows
    // and so cannot change; what they charge is the lease's, settled
    // against the account as performed.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let (attempt, charged) = fixture.prepare(unbounded(), u64::MAX, &Cancellation::default());
    assert!(attempt.result.unwrap().is_some());
    assert_eq!(attempt.work, charged);
    assert!(attempt.work > 0);
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
    assert_eq!(
        clause_patterns(&proposals),
        patterns_of(&[
            (vec![0, 1, 2], vec![false, true, false]),
            (vec![0, 1, 2], vec![true, true, true]),
        ])
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
    assert_eq!(
        clause_patterns(&proposals),
        patterns_of(&[
            (vec![0, 1, 2], vec![false, false, true]),
            (vec![0, 1, 2], vec![true, true, true]),
        ])
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
    assert_eq!(attempt.statistics.probes, 4);
    assert_eq!(attempt.statistics.saturation_units, 0);
    assert!(preparation.units().is_empty());
    assert_eq!(attempt.statistics.parity.rows, 1);
    // Only the four pattern probes that run are leased: no unit probe for a
    // guard the root already cuts, none for the patterns its clauses forbid.
    let allowance = fixture.closure_work() + Narrower::new(&fixture.theory).work();
    assert_eq!(attempt.statistics.leased, 4 * allowance);
}

#[test]
fn a_preparation_with_nothing_to_install_is_not_prepared() {
    // A gate less the clause forbidding all three held, its guard cut at the
    // root: no unit; the five allowed patterns span the whole space, so no
    // row; their counts run from none to all three, so no bound. Nothing
    // would be installed, and the preparation says so rather than
    // installing an empty one and counting it prepared.
    let mut clauses = gate(0, 1, 2);
    clauses.remove(3);
    let fixture = Saturation::build(3, &clauses, &[]);
    let valid = fixture.valid;
    let attempt = prepared_from(&fixture, |root| assert!(root.cut(valid)));
    assert!(attempt.result.unwrap().is_none());
    assert_eq!(
        (
            attempt.statistics.status,
            attempt.statistics.probes,
            attempt.statistics.parity.rows,
            attempt.statistics.cardinality.rows
        ),
        (super::super::PreparationStatus::NotPrepared, 5, 0, 0)
    );
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
    assert_eq!(
        clause_patterns(&grounded.propose()),
        clause_patterns(&plain.propose())
    );
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
    assert_eq!(proposals.counts.clauses, 2);
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

/// The pigeonhole core over `holes` holes: pigeon `p` in hole `k` is the atom
/// `p · holes + k`; each pigeon's clause forbids the pattern with none of its
/// holes held, and each pair of pigeons in one hole forbids both held.
fn pigeonhole(holes: usize) -> (usize, Vec<Vec<(usize, bool)>>) {
    let atom = |pigeon: usize, hole: usize| pigeon * holes + hole;
    let mut clauses: Vec<Vec<(usize, bool)>> = (0..=holes)
        .map(|pigeon| (0..holes).map(|hole| (atom(pigeon, hole), false)).collect())
        .collect();
    for hole in 0..holes {
        for first in 0..=holes {
            for second in first + 1..=holes {
                clauses.push(vec![(atom(first, hole), true), (atom(second, hole), true)]);
            }
        }
    }
    ((holes + 1) * holes, clauses)
}

/// A preparation's count bounds as `(side, support, value, cardinality)`, in
/// the system's order.
fn count_bounds(preparation: &FrozenPreparation) -> Vec<(BoundSide, Vec<usize>, u64, u64)> {
    let system = preparation.cardinality().expect("count bounds");
    let atoms = system.owner.atom_count();
    system
        .bounds
        .iter()
        .map(|bound| {
            (
                bound.side,
                (0..atoms)
                    .filter(|&atom| bound.support.contains(atom))
                    .collect(),
                bound.value,
                bound.cardinality,
            )
        })
        .collect()
}

#[test]
fn the_pigeon_clauses_are_lower_bounds() {
    // Four pigeons over three holes: each pigeon's clause, alone in its
    // group over its three hole atoms, allows no pattern with none held, so
    // at least one.
    let (n, clauses) = pigeonhole(3);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    let lower: Vec<_> = count_bounds(&preparation)
        .into_iter()
        .filter(|bound| bound.0 == BoundSide::Lower)
        .collect();
    assert_eq!(
        lower,
        (0..4)
            .map(|pigeon| (
                BoundSide::Lower,
                (0..3).map(|hole| pigeon * 3 + hole).collect(),
                1,
                3
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(attempt.statistics.cardinality.lower, 4);
    // The first round leaves a lone clause cutting its whole support to that
    // clause, and no later round runs without a parity row: each pigeon's
    // table is its clause's, with no affine part and so no row.
    assert_eq!(attempt.statistics.parity.rows, 0);
}

#[test]
fn a_hole_clique_is_an_upper_bound() {
    // The pairs of one hole, each forbidding both held, grow into the hole's
    // clique of four pigeons: at most one. A pigeon's own holes exclude one
    // another in this class as well, but only probing its group would show
    // it, and the first round leaves a lone clause to itself: the hole
    // cliques are the upper bounds.
    let (n, clauses) = pigeonhole(3);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    let upper: Vec<_> = count_bounds(&preparation)
        .into_iter()
        .filter(|bound| bound.0 == BoundSide::Upper)
        .collect();
    let holes: Vec<_> = upper.iter().filter(|bound| bound.3 == 4).cloned().collect();
    assert_eq!(
        holes,
        (0..3)
            .map(|hole| (
                BoundSide::Upper,
                (0..4).map(|pigeon| pigeon * 3 + hole).collect(),
                1,
                4
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(upper.len(), 3);
    assert_eq!(attempt.statistics.cardinality.upper, 3);
}

#[test]
fn the_double_count_refutes_the_root_at_h_plus_one_pigeons() {
    // The pigeons' lower bounds and the holes' upper bounds partition the
    // same atoms: four at least against three at most refutes the root.
    // Less one pigeon's clause, the counts agree and the root stands.
    let (n, mut clauses) = pigeonhole(3);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert!(preparation.cardinality().unwrap().root_refuted());
    assert!(preparation.root_refuted());
    assert!(attempt.statistics.root_refuted);
    // The pigeons' clauses and the holes' cliques partition the same atoms:
    // the one family, and it refutes.
    assert_eq!(attempt.statistics.cardinality.families, 1);
    clauses.remove(3);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert!(!preparation.root_refuted());
    // Three pigeons' clauses, and the completing bound of nothing over the
    // dropped pigeon's atoms.
    assert_eq!(attempt.statistics.cardinality.lower, 4);
}

#[test]
fn a_missing_pair_leaves_no_clique() {
    // Without the pair of pigeons 0 and 1 in hole 0, no clique holds both
    // their atoms there: the hole's pairs grow into two cliques of three,
    // neither covering the hole, so no family partitions the atoms both
    // ways and the root stands, as it should: those two pigeons may share.
    let (n, mut clauses) = pigeonhole(3);
    let position = clauses
        .iter()
        .position(|clause| clause == &vec![(0, true), (3, true)])
        .unwrap();
    clauses.remove(position);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    let upper: Vec<_> = count_bounds(&preparation)
        .into_iter()
        .filter(|bound| bound.0 == BoundSide::Upper)
        .collect();
    assert!(
        upper
            .iter()
            .all(|bound| !(bound.1.contains(&0) && bound.1.contains(&3)))
    );
    assert!(upper.contains(&(BoundSide::Upper, vec![0, 6, 9], 1, 3)));
    assert!(upper.contains(&(BoundSide::Upper, vec![3, 6, 9], 1, 3)));
    assert!(!preparation.cardinality().unwrap().root_refuted());
    assert!(!preparation.root_refuted());
}

#[test]
fn a_tables_greatest_allowed_count_is_an_upper_bound() {
    // Over three atoms, the clauses forbidding all held and none held leave
    // the counts one and two: a lower bound of one and an upper bound of two
    // over the same support, and no clique.
    let fixture = Saturation::build(
        3,
        &[
            vec![(0, true), (1, true), (2, true)],
            vec![(0, false), (1, false), (2, false)],
        ],
        &[],
    );
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    assert_eq!(
        count_bounds(&preparation),
        vec![
            (BoundSide::Lower, vec![0, 1, 2], 1, 3),
            (BoundSide::Upper, vec![0, 1, 2], 2, 3),
        ]
    );
    assert!(!preparation.root_refuted());
}

#[test]
fn count_bounds_under_an_open_guard_are_dropped() {
    // The pigeonhole clauses hang off a guard nothing saturates from: its
    // groups are dropped before any probe, and the one bound is the gate's
    // own, over its three atoms.
    let (n, clauses) = pigeonhole(2);
    let fixture = Saturation::build_with(n, &gate(0, 1, 2), &[], &clauses, false);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    // The gate's table bounds its three atoms above, and the completing
    // lower bound of nothing covers them below.
    let bounds = count_bounds(&preparation);
    assert_eq!(bounds.len(), 2);
    assert!(bounds.iter().all(|bound| bound.1 == vec![0, 1, 2]));
    assert_eq!(bounds[1], (BoundSide::Lower, vec![0, 1, 2], 0, 3));
    assert_eq!(attempt.statistics.cardinality.rows, 2);
    assert_eq!(attempt.statistics.parity.rows, 1);
}

#[test]
fn a_dropped_pigeons_atoms_complete_the_lower_side() {
    // Less pigeon 3's clause, the holes' cliques cover its atoms and the
    // pigeons' lower bounds do not: a lower bound of nothing over those
    // atoms completes the partition, so the family of the pigeons against
    // the holes forms, its counts agreeing at the root, and a region holding
    // pigeon 3 in any hole exceeds the holes' room for the other three.
    let (n, mut clauses) = pigeonhole(3);
    clauses.remove(3);
    let fixture = Saturation::build(n, &clauses, &[]);
    let attempt = fixture.prepared();
    let preparation = attempt.result.unwrap().unwrap();
    let bounds = count_bounds(&preparation);
    assert!(bounds.contains(&(BoundSide::Lower, vec![9, 10, 11], 0, 3)));
    assert!(!preparation.root_refuted());
    assert!(attempt.statistics.cardinality.families >= 1);
}

#[test]
fn two_candidates_of_one_theory_prepare_differently() {
    // One theory, two models with different masks on the gate's cone: the
    // saturated candidate, whose guard the unit cuts, loads the gate's row;
    // the model with every universal cut (`current`, the `ny(j)` and the
    // literals they falsify) leaves `valid` out, so it proposes no guard and
    // prepares nothing. Neither preparation belongs to the other candidate.
    let clauses = gate(0, 1, 2);
    let fixture = Saturation::build(3, &clauses, &[]);
    let first = fixture.prepared();
    assert_eq!(first.statistics.parity.rows, 1);
    let falsified: Vec<usize> = clauses
        .iter()
        .enumerate()
        .flat_map(|(c, clause)| {
            clause
                .iter()
                .enumerate()
                .filter(|(_, literal)| !literal.1)
                .map(move |(k, _)| (c, k))
        })
        .map(|(c, k)| fixture.literals[c][k])
        .collect();
    let (candidate, truth) = fixture.candidate(|atom| {
        atom == fixture.current || (3..6).contains(&atom) || falsified.contains(&atom)
    });
    let second = fixture.prepared_for(&candidate, &truth);
    assert_eq!(second.statistics.guards, 0);
    assert!(second.result.unwrap().is_none());
    let preparation = first.result.unwrap().unwrap();
    assert!(preparation.belongs_to(&fixture.theory, &fixture.candidate));
    assert!(!preparation.belongs_to(&fixture.theory, &candidate));
}

#[test]
fn probing_stops_after_the_second_round() {
    // Two chains over four inputs less one clause of the first chain's last
    // gate and one of the pair: left to itself the probing runs four rounds,
    // each tightening a hull the round before left inexact, down to one;
    // capped, it stops after the second, the first by the frozen reading
    // alone and the second under its rows, with two hulls still inexact,
    // and loads what the two rounds established.
    let mut clauses = two_chains_of(4);
    clauses.remove(24);
    clauses.remove(8);
    let fixture = Saturation::build(3 * 4 - 2, &clauses, &[]);
    let attempt = fixture.prepared();
    let statistics = attempt.statistics;
    assert_eq!(
        (
            statistics.rounds,
            statistics.inexact_hulls,
            statistics.root_refuted
        ),
        (2, 2, false)
    );
    assert!(
        statistics.probes < 85 && statistics.parity.rows < 7,
        "{statistics:?}"
    );
    let prepared = attempt.result.unwrap().unwrap();
    assert!(prepared.parity().is_some());
}

#[test]
fn a_probe_the_account_interrupts_is_not_counted() {
    // A probe is counted when it returns its verdict. The account's
    // allowance is swept across a preparation whose first probe is the
    // gate's unit: wherever the account refuses inside that probe no probe
    // is counted, since a counted probe has had its verdict read, which for
    // the first one is the unit.
    let fixture = Saturation::build(3, &gate(0, 1, 2), &[]);
    let complete = fixture.prepared();
    assert_eq!(complete.statistics.saturation_units, 1);
    let total = usize::try_from(complete.work).unwrap();
    for allowance in (0..total).step_by((total / 1024).max(1)) {
        let (attempt, _) = fixture.prepare(
            unbounded(),
            u64::try_from(allowance).unwrap(),
            &Cancellation::default(),
        );
        let statistics = attempt.statistics;
        assert!(
            statistics.probes == 0 || statistics.saturation_units >= 1,
            "allowance {allowance}: {statistics:?}"
        );
    }
}

#[test]
fn a_pigeonhole_beyond_the_group_width_is_refuted_at_the_root() {
    // Ten pigeons in nine holes: each pigeon's clause, over its nine holes,
    // is wider than a group may be, and each pair of pigeons in one hole is
    // a group of two. The pairs' cliques bound each hole by one; the
    // pigeons' clauses, too wide to probe, still bound each pigeon's holes
    // from below by one, since no countermodel cuts them all; the two
    // families partition the same places, and ten pigeons exceed nine holes.
    let (pigeons, holes) = (10, 9);
    let place = |pigeon: usize, hole: usize| pigeon * holes + hole;
    let mut clauses: Vec<Vec<(usize, bool)>> = (0..pigeons)
        .map(|pigeon| {
            (0..holes)
                .map(|hole| (place(pigeon, hole), false))
                .collect()
        })
        .collect();
    for hole in 0..holes {
        for first in 0..pigeons {
            for second in first + 1..pigeons {
                clauses.push(vec![
                    (place(first, hole), true),
                    (place(second, hole), true),
                ]);
            }
        }
    }
    let fixture = Saturation::build(pigeons * holes, &clauses, &[]);
    let statistics = fixture.prepared().statistics;
    assert_eq!(statistics.unsupported_clauses, 10, "{statistics:?}");
    assert!(statistics.root_refuted, "{statistics:?}");
}
