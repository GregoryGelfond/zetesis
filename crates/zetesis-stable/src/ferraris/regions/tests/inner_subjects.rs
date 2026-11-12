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
            let body = if let [only] = conjuncts[..] {
                only
            } else if let [left, right] = conjuncts[..] {
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

    /// What the root's first closure charges.
    fn closure_work(&self) -> u64 {
        let narrower = Narrower::new(&self.theory);
        let mut root = Region::all_open(self.theory.atom_count());
        let mut knowledge = narrower.frozen_knowledge();
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
            &mut NarrowingScratch::default(),
            &cancellation,
            &mut meter,
        );
        attempt.statistics.work
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
                closure_work: self.closure_work(),
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

    /// The membership check under a search budget of `work`: its outcome,
    /// what the walk observed, and the work it spent.
    fn check_within(&self, work: u64) -> (Result<crate::Check, Incomplete>, Statistics, u64) {
        let index = built(&self.theory);
        let cancellation = Cancellation::default();
        let mut workspace = EvaluationWorkspace::default();
        let evaluated = workspace
            .evaluate(&self.candidate, EvaluationLimits::default(), &cancellation)
            .result
            .unwrap();
        let mut query_budget = budget(&cancellation, work);
        let mut statistics = Statistics::default();
        let check = ReductQuery::new(&index).check(
            &evaluated,
            Limits::default(),
            &mut query_budget,
            &mut statistics,
            &mut NarrowingScratch::default(),
        );
        (check, statistics, query_budget.statistics.work)
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
    let index = built(&fixture.theory);
    let result = narrow_inner(
        InnerReading {
            narrower,
            subject: fixture.subject(),
            index: &index,
            candidate: &fixture.candidate,
        },
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
    for (theory, stable, attempts, counts) in [
        (cycle(), false, 1, (2, 0, 1, 4, 10)),
        (positive(), true, 0, (1, 0, 1, 3, 5)),
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
        assert_eq!((preparation.attempts, preparation.prepared), (attempts, 0));
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
    // a countermodel, and the rows of the remaining clauses, probed to
    // exactness in a second round, guide the walk to it in fewer regions
    // than the frozen subject alone needed for the unsatisfiable core (51).
    let mut clauses = two_chains();
    clauses.pop();
    let fixture = Saturation::build(7, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::NonMinimal(_)));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (17, 0, 1)
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
fn a_dropped_clause_changes_the_rows_loaded() {
    // The stable candidate loads a row per gate and nothing more, its root
    // refuted by them; the dropped-clause candidate's second round probes
    // the dropped gate to exactness, and the refutations along the way add
    // their units: the two preparations' rows differ.
    let stable = Saturation::build(7, &two_chains());
    let stable = stable.check().1.reduct.inner_preparation;
    let mut clauses = two_chains();
    clauses.pop();
    let dropped = Saturation::build(7, &clauses);
    let dropped = dropped.check().1.reduct.inner_preparation;
    assert_eq!((stable.rows, dropped.rows), (5, 6));
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

#[test]
fn a_pigeonhole_core_is_decided_at_the_root() {
    // Five pigeons over four holes: the pigeons' lower bounds and the holes'
    // cliques partition the same atoms, five at least against four at most,
    // and the double count refutes the root before any region is walked.
    let (n, clauses) = pigeonhole(4);
    let fixture = Saturation::build(n, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::Stable));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (1, 1, 0)
    );
    let preparation = statistics.reduct.inner_preparation;
    assert_eq!(preparation.root_refuted, 1);
    // Five pigeons' clauses and four holes' cliques.
    assert_eq!(preparation.bounds, 5 + 4);
}

#[test]
fn a_dropped_pigeon_candidate_finds_its_countermodel_in_few_regions() {
    // Less one pigeon's clause the core is satisfiable: the counts agree,
    // the root stands, and the walk finds a countermodel under the bounds.
    let (n, mut clauses) = pigeonhole(4);
    clauses.remove(4);
    let fixture = Saturation::build(n, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::NonMinimal(_)));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (55, 0, 1)
    );
    // Four pigeons' clauses, four holes' cliques, and the bound of nothing
    // completing the lower side over the dropped pigeon's.
    assert_eq!(statistics.reduct.inner_preparation.bounds, 4 + 4 + 1);
}

/// The pigeonhole core read in the other polarity: the atom `p · holes + k`
/// is held when pigeon `p` is not in hole `k`. Each pigeon's clause forbids
/// holding all of its atoms, and each pair of pigeons in one hole forbids
/// cutting both.
fn complemented_pigeonhole(holes: usize) -> (usize, Vec<Vec<(usize, bool)>>) {
    let (n, clauses) = pigeonhole(holes);
    let complemented = clauses
        .into_iter()
        .map(|clause| {
            clause
                .into_iter()
                .map(|(atom, held)| (atom, !held))
                .collect()
        })
        .collect();
    (n, complemented)
}

#[test]
fn a_complemented_pigeonhole_core_is_decided_at_the_root() {
    // Five pigeons over four holes in the other polarity: each hole's pairs
    // forbid cutting both, so its covering clique holds four of five, and
    // each pigeon's table holds three of four at most. The two partition the
    // same atoms, sixteen at least against fifteen at most, and the double
    // count refutes the root before any region is walked.
    let (n, clauses) = complemented_pigeonhole(4);
    let fixture = Saturation::build(n, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::Stable));
    let regions = statistics.reduct.regions;
    assert_eq!(
        (regions.regions, regions.refuted, regions.leaves),
        (1, 1, 0)
    );
    assert_eq!(statistics.reduct.inner_preparation.root_refuted, 1);
}

#[test]
fn a_complemented_core_less_one_pair_keeps_its_countermodel() {
    // Without the pair of pigeons 0 and 1 in hole 0 the core is satisfiable:
    // both may be in that hole. No covering clique holds both their atoms,
    // so the root stands and the walk still finds the countermodel.
    let (n, mut clauses) = complemented_pigeonhole(4);
    let position = clauses
        .iter()
        .position(|clause| clause == &vec![(0, false), (4, false)])
        .unwrap();
    clauses.remove(position);
    let fixture = Saturation::build(n, &clauses);
    let (check, statistics) = fixture.check();
    assert!(matches!(check, crate::Check::NonMinimal(_)));
    assert_eq!(statistics.reduct.inner_preparation.root_refuted, 0);
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
    assert!(entry.state.cardinality.is_some());
    let copy = entry.state.clone();
    assert!(copy.parity.is_some() && copy.cardinality.is_some());
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

#[test]
fn the_index_takes_the_opened_roots_producers_without_extracting_again() {
    // The outer walk extracted the theory's producers when it opened the
    // root; an index seeded with them hands them to the first preparation
    // without a second extraction or charge.
    let fixture = Saturation::build(7, &two_chains());
    let cancellation = Cancellation::default();
    let extraction = producers(&fixture.theory, RegionLimits::default(), &cancellation).unwrap();
    let extracted = extraction.producers.map(Arc::new);
    assert!(extracted.is_some());
    let mut owner = OriginalIndex::seeded(&fixture.theory, u64::MAX, extracted);
    let index = owner.ensure(&cancellation, |_| Ok(())).unwrap();
    let found = index
        .producers(&cancellation, |_| panic!("the seeded index extracts again"))
        .unwrap();
    assert!(found.is_some());
}

#[test]
fn a_preparation_of_another_candidate_is_refused_by_identity() {
    // A preparation serves its own candidate's query alone: offered to a
    // region of another theory's candidate, the inner narrowing refuses it
    // as the wrong theory before reading a row.
    let fixture = Saturation::build(4, &shared_inputs());
    let (narrower, root, knowledge) = fixture.closed();
    let preparation = fixture.preparation(&narrower, &root, &knowledge);
    let twin = Saturation::build(4, &shared_inputs());
    let (twin_narrower, twin_root, _) = twin.closed();
    let mut entry = Entry::new(twin_root, CountermodelKnowledge::fresh(&twin_narrower));
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let index = built(&twin.theory);
    let result = narrow_inner(
        InnerReading {
            narrower: &twin_narrower,
            subject: twin.subject(),
            index: &index,
            candidate: &twin.candidate,
        },
        Some(&preparation),
        &mut entry,
        &mut NarrowingScratch::default(),
        &mut budget,
        &mut RegionCounts::default(),
    );
    assert!(matches!(result, Err(Incomplete::WrongTheory)));
}

#[test]
fn a_spent_preparation_is_neither_pending_nor_installed() {
    // A preparation handed to a publication that could not return it is
    // spent: the walk goes on without rows and never prepares below the
    // root.
    let spent = InnerPreparation::Spent;
    assert!(!spent.is_pending());
    assert!(spent.installed().is_none());
}

/// Every clause of `width` over `n` universals: each subset of that many
/// atoms with each pattern of held atoms.
fn clauses_of_width(n: usize, width: usize) -> Vec<Vec<(usize, bool)>> {
    let mut clauses = Vec::new();
    for subset in 0_usize..1 << n {
        if subset.count_ones() as usize != width {
            continue;
        }
        let atoms: Vec<usize> = (0..n).filter(|&j| subset & (1 << j) != 0).collect();
        for pattern in 0_usize..1 << width {
            clauses.push(
                atoms
                    .iter()
                    .enumerate()
                    .map(|(k, &j)| (j, pattern & (1 << k) != 0))
                    .collect(),
            );
        }
    }
    clauses
}

/// Whether some assignment of the universals satisfies every clause: a
/// clause is satisfied when some literal's atom is not as it lists.
fn satisfiable(n: usize, clauses: &[Vec<(usize, bool)>]) -> bool {
    (0_usize..1 << n).any(|assignment| {
        clauses.iter().all(|clause| {
            clause
                .iter()
                .any(|&(j, held)| (assignment & (1 << j) != 0) != held)
        })
    })
}

#[test]
fn every_small_saturation_core_is_decided_as_by_enumeration() {
    // Over two universals every set of the eight clauses of width one or
    // two, over three every set of the eight clauses of width three and
    // every thirteenth set of the twelve of width two: the candidate is
    // stable exactly when no assignment satisfies the core, and a
    // countermodel found leaves the guard out and falsifies no clause.
    for (n, widths, stride) in [(2, &[1, 2][..], 1), (3, &[3][..], 1), (3, &[2][..], 13)] {
        let clauses: Vec<Vec<(usize, bool)>> = widths
            .iter()
            .flat_map(|&width| clauses_of_width(n, width))
            .collect();
        for selection in (1_usize..1 << clauses.len()).step_by(stride) {
            let core: Vec<Vec<(usize, bool)>> = clauses
                .iter()
                .enumerate()
                .filter(|(index, _)| selection & (1 << index) != 0)
                .map(|(_, clause)| clause.clone())
                .collect();
            let fixture = Saturation::build(n, &core);
            match fixture.check().0 {
                crate::Check::Stable => {
                    assert!(!satisfiable(n, &core), "n={n} core {core:?}");
                }
                crate::Check::NonMinimal(witness) => {
                    assert!(satisfiable(n, &core), "n={n} core {core:?}");
                    assert!(!witness.contains(fixture.valid), "n={n} core {core:?}");
                    for clause in &core {
                        assert!(
                            clause.iter().any(|&(j, held)| !witness.contains(if held {
                                j
                            } else {
                                n + j
                            })),
                            "n={n} core {core:?}: {clause:?} falsified"
                        );
                    }
                }
                other => panic!("n={n} core {core:?}: {other:?}"),
            }
        }
    }
}

#[test]
fn a_stop_inside_the_late_tier_is_a_stopped_query() {
    // A dropped-clause candidate's countermodel is found with the late tier's
    // parity rows. Across work limits, every check either finds it or reports
    // the stop: a stop inside the parity or counting subject is a stopped
    // query, never a refuted region whose countermodel the walk then misses.
    let mut clauses = two_chains();
    clauses.pop();
    let fixture = Saturation::build(7, &clauses);
    let (check, statistics, total) = fixture.check_within(u64::MAX);
    assert!(matches!(check, Ok(crate::Check::NonMinimal(_))));
    assert!(statistics.reduct.regions.arithmetic.parity_passes > 0);
    let stride = usize::try_from(total / 2048).unwrap().max(1);
    let mut stopped_after_the_tier_ran = 0;
    for work in (0..total).step_by(stride) {
        let (check, statistics, _) = fixture.check_within(work);
        match check {
            Ok(crate::Check::NonMinimal(_)) => {}
            Err(Incomplete::WorkLimit) => {
                if statistics.reduct.regions.arithmetic.parity_passes > 0 {
                    stopped_after_the_tier_ran += 1;
                }
            }
            other => panic!("work {work} of {total}: {other:?}"),
        }
    }
    assert!(
        stopped_after_the_tier_ran > 0,
        "of {total} at stride {stride}"
    );
}

#[test]
fn optional_inner_storage_refusal_preserves_the_complete_family() {
    // Either first disjunct forces the guard, which forces every pair.
    // The final independent disjunction gives exactly two minimal models.
    // The pairwise guards supply distinct optional groups over those same
    // atoms, so that their table needs more space than ordinary checking.
    let pairs = 6;
    let guard = 2 * pairs;
    let (left, right) = (guard + 1, guard + 2);
    let mut nodes: Vec<_> = (0..=right).map(Node::atom).collect();
    let mut roots = Vec::new();
    let mut append = |node| {
        nodes.push(node);
        nodes.len() - 1
    };
    for p in 0..pairs {
        let q = pairs + p;
        roots.push(append(Node::or_pair([p, q])));
        roots.push(append(Node::implies(guard, p)));
        roots.push(append(Node::implies(guard, q)));
    }
    roots.push(append(Node::implies(0, guard)));
    roots.push(append(Node::implies(pairs, guard)));
    roots.push(append(Node::or_pair([left, right])));
    for p in 0..pairs {
        for q in p + 1..pairs {
            let both = append(Node::and_pair([p, q]));
            roots.push(append(Node::implies(both, guard)));
        }
    }
    let theory = Theory::new(
        right + 1,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    let expected = [left, right].map(|atom| (0..=guard).chain([atom]).collect::<Vec<_>>());
    let enumerate = |bytes| {
        let limits = Limits {
            max_reduct_bytes: bytes,
            ..Limits::default()
        };
        let mut search = StableModels::new(&theory, limits, Cancellation::default()).unwrap();
        let mut family: Vec<_> = search
            .by_ref()
            .map(|model| model.unwrap().atoms().collect::<Vec<_>>())
            .collect();
        family.sort();
        assert_eq!(family, expected);
        search.statistics().reduct
    };
    // The unrestricted run measures the retained workspace the ordinary
    // reduct check actually requires on this target.
    let unrestricted = enumerate(u64::MAX);
    let baseline = unrestricted.inner_preparation;
    assert_eq!(
        (baseline.attempts, baseline.prepared, baseline.declined),
        (2, 2, 0)
    );
    let tight = enumerate(u64::try_from(unrestricted.peak_workspace_bytes).unwrap());
    let fallback = tight.inner_preparation;
    assert_eq!(
        (fallback.attempts, fallback.prepared, fallback.declined),
        (2, 0, 2)
    );
}
