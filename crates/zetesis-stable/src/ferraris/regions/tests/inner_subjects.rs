//! The inner walk's own subjects: parity rows of a candidate's countermodel
//! class, probed from its closed query root, narrowing beside the frozen
//! subject in the joint closure.

use super::*;
use crate::ferraris::conditions::CountermodelKnowledge;
use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;
use zetesis_algebra::Countermodels;
use zetesis_ferraris::{
    ClosedQuery, EvaluationLimits, EvaluationWorkspace, FormulaParts, FrozenPreparation,
    FrozenPreparationLimits, FrozenSubject, Knowledge, Narrower, OperandSpan, RegionLimits,
    producers,
};

/// The saturation shape of the quantified fixtures over `n` universal
/// variables: roots `y(j) ∨ ny(j)`; per clause a chain `fl(c, k) ←
/// falsifier` and `f(c) ← ⋀ fl(c, k)`; `valid ← current ∧ f(c)` with
/// `current` a fact; and `y(j) ← valid`, `ny(j) ← valid`. A clause lists
/// `(j, held)`: it is falsified when `y(j)` is held (`true`) or cut (`false`),
/// the latter read through `ny(j)`. Atoms: `y(j) = j`, `ny(j) = n + j`,
/// `valid = 2n`, `current = 2n + 1`, then `f(c)`, then the `fl(c, k)`.
struct Saturation {
    theory: Theory,
    candidate: Interpretation,
    truth: Vec<bool>,
    valid: usize,
    current: usize,
}

impl Saturation {
    fn build(n: usize, clauses: &[Vec<(usize, bool)>]) -> Self {
        let (valid, current) = (2 * n, 2 * n + 1);
        let f = |c: usize| current + 1 + c;
        let mut fl_base = Vec::new();
        let mut next = current + 1 + clauses.len();
        for clause in clauses {
            fl_base.push(next);
            next += clause.len();
        }
        let atoms = next;
        let mut nodes: Vec<Node> = (0..atoms).map(Node::atom).collect();
        let mut operands = Vec::new();
        let mut roots = Vec::new();
        for j in 0..n {
            nodes.push(Node::or_pair([j, n + j]));
            roots.push(nodes.len() - 1);
        }
        roots.push(current);
        for (c, clause) in clauses.iter().enumerate() {
            let mut conjuncts = Vec::new();
            for (k, &(j, held)) in clause.iter().enumerate() {
                let falsifier = if held { j } else { n + j };
                let literal = fl_base[c] + k;
                nodes.push(Node::implies(falsifier, literal));
                roots.push(nodes.len() - 1);
                conjuncts.push(literal);
            }
            let body = if let [left, right] = conjuncts[..] {
                nodes.push(Node::and_pair([left, right]));
                nodes.len() - 1
            } else {
                let span = OperandSpan {
                    start: operands.len(),
                    length: conjuncts.len(),
                };
                operands.extend(conjuncts.iter().copied());
                nodes.push(Node::and_span(span));
                nodes.len() - 1
            };
            nodes.push(Node::implies(body, f(c)));
            roots.push(nodes.len() - 1);
            nodes.push(Node::and_pair([current, f(c)]));
            let gated = nodes.len() - 1;
            nodes.push(Node::implies(gated, valid));
            roots.push(nodes.len() - 1);
        }
        for j in 0..n {
            nodes.push(Node::implies(valid, j));
            roots.push(nodes.len() - 1);
            nodes.push(Node::implies(valid, n + j));
            roots.push(nodes.len() - 1);
        }
        let theory = Theory::new(
            atoms,
            FormulaParts::new(nodes, operands).unwrap(),
            roots,
            AdmissionLimits::default(),
        )
        .unwrap();
        let candidate = Interpretation::new(&theory, 0..atoms).unwrap();
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
        Self {
            theory,
            candidate,
            truth,
            valid,
            current,
        }
    }

    fn subject(&self) -> FrozenSubject<'_> {
        FrozenSubject::new(&self.theory, &self.truth)
    }

    /// The query root closed by the frozen narrowing, with its knowledge.
    fn closed(&self) -> (Narrower, Region, Knowledge<Countermodels>) {
        let narrower = Narrower::new(&self.theory);
        let mut root = Region::all_open(self.theory.atom_count());
        let mut knowledge = narrower.frozen_knowledge();
        let mut scratch = NarrowingScratch::default();
        let cancellation = Cancellation::default();
        let mut budget = budget(&cancellation, u64::MAX);
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let mut meter = Meter::reserved(&mut quota);
        let attempt = narrower.narrow_frozen_known_reserved(
            self.subject(),
            &mut root,
            &mut knowledge,
            &mut scratch,
            &cancellation,
            &mut meter,
        );
        assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
        assert!(root.is_held(self.current));
        (narrower, root, knowledge)
    }

    /// The candidate's inner subjects, prepared from its closed root.
    fn preparation(
        &self,
        narrower: &Narrower,
        root: &Region,
        knowledge: &Knowledge<Countermodels>,
    ) -> FrozenPreparation {
        let producers = producers(
            &self.theory,
            RegionLimits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .producers
        .unwrap();
        let cancellation = Cancellation::default();
        let mut budget = budget(&cancellation, u64::MAX);
        let mut quota = BudgetQuota {
            budget: &mut budget,
        };
        let attempt = FrozenPreparation::prepare_reserved(
            ClosedQuery {
                narrower,
                producers: Some(&producers),
                subject: self.subject(),
                candidate: &self.candidate,
                region: root,
                knowledge,
            },
            &mut NarrowingScratch::default(),
            FrozenPreparationLimits {
                work: u64::MAX,
                bytes: usize::MAX,
            },
            &cancellation,
            &mut quota,
        );
        attempt.result.unwrap().unwrap()
    }

    /// The whole membership check of the candidate through the reduct query.
    fn check(&self) -> (crate::Check, Statistics) {
        let index = built(&self.theory);
        let cancellation = Cancellation::default();
        let mut workspace = EvaluationWorkspace::default();
        let evaluated = workspace
            .evaluate(&self.candidate, EvaluationLimits::default(), &cancellation)
            .result
            .unwrap();
        let mut query_budget = budget(&cancellation, u64::MAX);
        let mut statistics = Statistics::default();
        let check = ReductQuery::new(&index)
            .check(
                &evaluated,
                Limits::default(),
                &mut query_budget,
                &mut statistics,
                &mut NarrowingScratch::default(),
            )
            .unwrap();
        (check, statistics)
    }
}

/// A gate's four clauses: the odd patterns over `(a, i, o)`.
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

/// Two chains over three inputs computing the same parity, whose results
/// `y(4)` and `y(6)` must differ: an unsatisfiable core, so its saturated
/// candidate is stable.
fn two_chains() -> Vec<Vec<(usize, bool)>> {
    let mut clauses = gate(0, 1, 3);
    clauses.extend(gate(3, 2, 4));
    clauses.extend(gate(1, 2, 5));
    clauses.extend(gate(5, 0, 6));
    clauses.push(vec![(4, false), (6, false)]);
    clauses.push(vec![(4, true), (6, true)]);
    clauses
}

/// Two gates over the same inputs: `y(2) = y(0) ⊕ y(1)` and `y(3) = y(0) ⊕
/// y(1)`, so the rows combine to `y(2) ⊕ y(3) = 0`, which no single clause
/// propagates.
fn shared_inputs() -> Vec<Vec<(usize, bool)>> {
    let mut clauses = gate(0, 1, 2);
    clauses.extend(gate(0, 1, 3));
    clauses
}

/// Narrow a copy of `region` once through the inner closure with fresh
/// countermodel knowledge, with or without a preparation's late tier.
fn narrow(
    fixture: &Saturation,
    narrower: &Narrower,
    region: &Region,
    preparation: Option<&FrozenPreparation>,
) -> (Narrowing, Entry<CountermodelKnowledge>, RegionCounts) {
    let mut entry = Entry::new(region.clone(), CountermodelKnowledge::fresh(narrower));
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut counts = RegionCounts::default();
    let result = narrow_inner(
        narrower,
        fixture.subject(),
        preparation,
        &mut entry,
        &mut NarrowingScratch::default(),
        &mut budget,
        &mut counts,
    )
    .unwrap();
    (result, entry, counts)
}

#[test]
fn a_theory_without_saturation_shape_walks_as_today() {
    // The counts the frozen walk alone produced on these fixtures before the
    // inner preparation entered the walk: a preparation that proposes nothing
    // changes neither the verdict nor a count.
    for (theory, stable, counts) in [
        (cycle(), false, (2, 0, 1, 4, 10)),
        (positive(), true, (1, 0, 1, 3, 5)),
    ] {
        let index = built(&theory);
        let cancellation = Cancellation::default();
        let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
        let mut workspace = EvaluationWorkspace::default();
        let evaluated = workspace
            .evaluate(&candidate, EvaluationLimits::default(), &cancellation)
            .result
            .unwrap();
        let mut query_budget = budget(&cancellation, u64::MAX);
        let mut statistics = Statistics::default();
        let check = ReductQuery::new(&index)
            .check(
                &evaluated,
                Limits::default(),
                &mut query_budget,
                &mut statistics,
                &mut NarrowingScratch::default(),
            )
            .unwrap();
        assert_eq!(matches!(check, crate::Check::Stable), stable);
        let regions = statistics.reduct.regions;
        assert_eq!(
            (
                regions.regions,
                regions.refuted,
                regions.leaves,
                regions.propagations,
                regions.work
            ),
            counts
        );
        assert_eq!(regions.arithmetic, ArithmeticCounts::default());
        let preparation = statistics.reduct.inner_preparation;
        assert_eq!((preparation.attempts, preparation.prepared), (1, 0));
    }
}

#[test]
fn the_stable_candidate_of_a_parity_core_is_decided_at_the_root() {
    // Before the preparation, the frozen walk alone visited 51 regions,
    // refuting 25, to establish this candidate's stability.
    let fixture = Saturation::build(7, &two_chains());
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::Stable));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (1, 1, 0)
    );
    let preparation = statistics.reduct.inner_preparation;
    assert_eq!((preparation.attempts, preparation.prepared), (1, 1));
    assert_eq!(preparation.saturation_units, 1);
    assert_eq!(preparation.root_refuted, 1);
    assert_eq!(preparation.rows, 5);
    assert!(preparation.work > 0);
}

#[test]
fn a_dropped_clause_candidate_finds_its_countermodel_in_few_regions() {
    // Dropping the last clause makes the core satisfiable: the candidate has
    // a countermodel, and the rows of the remaining clauses guide the walk to
    // it in fewer regions than the frozen subject alone needed for the
    // unsatisfiable core (51).
    let mut clauses = two_chains();
    clauses.pop();
    let fixture = Saturation::build(7, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::NonMinimal(_)));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (19, 1, 1)
    );
    let preparation = statistics.reduct.inner_preparation;
    assert_eq!((preparation.attempts, preparation.prepared), (1, 1));
    assert_eq!(preparation.root_refuted, 0);
    // The shared-input pair: the frozen walk alone visited 17 regions.
    let fixture = Saturation::build(4, &shared_inputs());
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::NonMinimal(_)));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (9, 0, 1)
    );
}

#[test]
fn the_parity_tier_refutes_a_region_that_contradicts_a_basis_row() {
    let fixture = Saturation::build(4, &shared_inputs());
    let (narrower, root, knowledge) = fixture.closed();
    let preparation = fixture.preparation(&narrower, &root, &knowledge);
    let mut region = root.clone();
    assert!(region.cut(fixture.valid) && region.hold(2) && region.cut(3));
    let (frozen_only, _, _) = narrow(&fixture, &narrower, &region, None);
    assert_ne!(frozen_only, Narrowing::Refuted);
    let (with_rows, _, counts) = narrow(&fixture, &narrower, &region, Some(&preparation));
    assert_eq!(with_rows, Narrowing::Refuted);
    assert_ne!(counts.arithmetic, ArithmeticCounts::default());
}

#[test]
fn the_parity_tier_forces_the_last_open_atom_of_a_row() {
    let fixture = Saturation::build(4, &shared_inputs());
    let (narrower, root, knowledge) = fixture.closed();
    let preparation = fixture.preparation(&narrower, &root, &knowledge);
    let mut region = root.clone();
    assert!(region.cut(fixture.valid) && region.hold(2));
    let (frozen_only, entry, _) = narrow(&fixture, &narrower, &region, None);
    assert_ne!(frozen_only, Narrowing::Refuted);
    assert!(entry.region.is_open(3));
    let (with_rows, entry, _) = narrow(&fixture, &narrower, &region, Some(&preparation));
    assert_eq!(with_rows, Narrowing::Fixed { changed: true });
    assert!(entry.region.is_held(3));
}

#[test]
fn inner_knowledge_clones_its_arithmetic_slots_with_the_frozen_state() {
    let fixture = Saturation::build(4, &shared_inputs());
    let (narrower, root, knowledge) = fixture.closed();
    let preparation = fixture.preparation(&narrower, &root, &knowledge);
    let mut region = root.clone();
    assert!(region.cut(fixture.valid) && region.hold(2));
    let (_, entry, _) = narrow(&fixture, &narrower, &region, Some(&preparation));
    assert!(entry.state.parity.is_some());
    assert!(entry.state.cardinality.is_none());
    let copy = entry.state.clone();
    assert!(copy.parity.is_some());
    let mut recycled = CountermodelKnowledge::fresh(&narrower);
    recycled.clone_from(&entry.state);
    assert!(recycled.parity.is_some());
}

#[test]
fn a_served_region_narrows_with_the_queries_preparation() {
    let fixture = Saturation::build(4, &shared_inputs());
    let (narrower, root, knowledge) = fixture.closed();
    let preparation = fixture.preparation(&narrower, &root, &knowledge);
    let index = built(&fixture.theory);
    let cancellation = Cancellation::default();
    for (prepared, expected_refuted) in [(None, 0), (Some(&preparation), 1)] {
        let mut region = root.clone();
        assert!(region.cut(fixture.valid) && region.hold(2) && region.cut(3));
        let mut entry = Entry::new(region, CountermodelKnowledge::fresh(&narrower));
        let mut query_budget = budget(&cancellation, u64::MAX);
        let mut statistics = Statistics::default();
        let mut installed = InnerPreparation::Pending;
        let served = ReductQuery::new(&index)
            .serve(
                PublishedQuery {
                    truth: &fixture.truth,
                    candidate: &fixture.candidate,
                    preparation: prepared,
                    prepare_root: false,
                },
                &mut entry,
                Limits::default(),
                &mut query_budget,
                (&mut statistics, &mut NarrowingScratch::default()),
                &mut installed,
            )
            .unwrap();
        assert!(installed.is_pending());
        assert_eq!(statistics.reduct.regions.refuted, expected_refuted);
        match (prepared, served) {
            (None, Served::Split(_)) | (Some(_), Served::Empty) => {}
            (_, Served::Split(_)) => panic!("the rows refute the served region"),
            (_, Served::Empty) => panic!("the frozen subject alone cannot refute it"),
            (_, Served::Countermodel(_)) => panic!("the region has no leaf"),
        }
    }
}

#[test]
fn a_residual_query_prepares_from_its_own_root() {
    // A residual root deciding y(2) and y(3) against the rows the clauses
    // imply: prepared from that root, the rows refute it at once.
    let fixture = Saturation::build(4, &shared_inputs());
    let index = built(&fixture.theory);
    let cancellation = Cancellation::default();
    let mut workspace = EvaluationWorkspace::default();
    let evaluated = workspace
        .evaluate(
            &fixture.candidate,
            EvaluationLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    let mut residual = Region::all_open(fixture.theory.atom_count());
    assert!(residual.cut(fixture.valid) && residual.hold(2) && residual.cut(3));
    let mut query_budget = budget(&cancellation, u64::MAX);
    let mut statistics = Statistics::default();
    let witness = ReductQuery::new(&index)
        .check_region(
            &evaluated,
            residual,
            Limits::default(),
            &mut query_budget,
            (&mut statistics, &mut NarrowingScratch::default()),
            None,
        )
        .unwrap();
    assert!(witness.is_none());
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (1, 1, 0)
    );
    let preparation = statistics.reduct.inner_preparation;
    assert_eq!((preparation.attempts, preparation.prepared), (1, 1));
    assert_eq!(preparation.root_refuted, 1);
}
