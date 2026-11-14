//! Dependency precedence and preference authority, independent of membership.

use super::{GeneratorOrder, GeneratorOrderError, GeneratorOrderLimits};
use crate::{
    AdmissionLimits, FormulaParts, Narrower, Node, OriginalSubject, Region, RegionLimits, Theory,
};
use zetesis_algebra::{Cancellation, Stop};

mod depths;
mod shortened;

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        atoms,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn extract(theory: &Theory) -> crate::Extraction {
    crate::producers(theory, RegionLimits::default(), &Cancellation::default()).unwrap()
}

fn prepare(theory: &Theory) -> super::GeneratorOrderAttempt {
    let producers = extract(theory);
    GeneratorOrder::prepare(
        OriginalSubject::new(theory, producers.producers.as_ref()),
        GeneratorOrderLimits {
            max_bytes: usize::MAX,
            max_work: u64::MAX,
        },
        &Cancellation::default(),
    )
}

/// The order's choice for a region that has shortened nothing: by choice
/// depth, then component, then atom.
fn choose(order: &GeneratorOrder, theory: &Theory, region: &Region) -> Option<usize> {
    let narrower = Narrower::new(theory);
    order
        .preferred(
            &narrower,
            theory,
            region,
            &narrower.knowledge(),
            &mut crate::regions::Meter::local(u64::MAX),
        )
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap()
}

fn marked(order: &GeneratorOrder, atom: usize) -> bool {
    order.heads[atom / 64] & (1 << (atom % 64)) != 0
}

fn upstream() -> Theory {
    // Upstream choice b, downstream choice a :- b. Atom numbering intentionally
    // opposes dependency order; no source names or argument positions exist.
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::implies(1, 2),
            Node::or_pair([5, 1]),
            Node::implies(1, 4),
        ],
        vec![6, 7],
    )
}

#[test]
fn a_generator_precedes_its_downstream_choice() {
    let theory = upstream();
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(order.ranks[1] < order.ranks[0]);
    let mut region = Region::all_open(2);
    assert_eq!(choose(&order, &theory, &region), Some(1));
    assert!(region.cut(1));
    assert_eq!(choose(&order, &theory, &region), Some(0));
    assert!(region.hold(0));
    assert_eq!(choose(&order, &theory, &region), None);
}

#[test]
fn ordinary_singleton_producers_remove_pure_marks() {
    // a | b; c -> b; choice(c); a -> c. Only b loses its mark: an explicit
    // atomic choice retains precedence even when an ordinary rule derives it.
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::or_pair([0, 1]),
            Node::implies(2, 1),
            Node::implies(2, 3),
            Node::or_pair([2, 6]),
            Node::implies(0, 2),
        ],
        vec![4, 5, 7, 8],
    );
    let order = prepare(&theory).result.unwrap().unwrap();
    assert_eq!(
        (marked(&order, 0), marked(&order, 1), marked(&order, 2)),
        (true, false, true)
    );
    let mut region = Region::all_open(3);
    assert_eq!(choose(&order, &theory, &region), Some(0));
    assert!(region.cut(0));
    assert_eq!(choose(&order, &theory, &region), Some(2));
    assert!(region.cut(2));
    assert_eq!(choose(&order, &theory, &region), Some(1));
}

#[test]
fn repeated_heads_do_not_create_a_false_generator() {
    // a | a is an ordinary singleton producer, despite repeated syntax.
    let theory = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::or_pair([0, 0]),
            Node::implies(1, 2),
            Node::or_pair([1, 4]),
        ],
        vec![3, 5],
    );
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(!marked(&order, 0));
    assert!(marked(&order, 1));
}

#[test]
fn both_implication_operands_precede_generated_heads() {
    // (b -> c) -> choice(a). Both signs are dependency inputs; this is not
    // positive-program certification and cannot justify an unfounded cut.
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([4, 0]),
            Node::implies(1, 2),
            Node::implies(6, 5),
        ],
        vec![7],
    );
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(order.ranks[1] < order.ranks[0]);
    assert!(order.ranks[2] < order.ranks[0]);
}

#[test]
fn disjunctive_heads_share_their_body_predecessors() {
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::or_pair([0, 1]),
            Node::and_pair([2, 2]),
            Node::implies(4, 3),
        ],
        vec![5],
    );
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(order.ranks[2] < order.ranks[0]);
    assert!(order.ranks[2] < order.ranks[1]);
}

#[test]
fn every_small_dependency_graph_has_correct_components() {
    // All 64 directed graphs on three distinct vertices. Every atom has a
    // choice; alternating edges use a positive or default-negative body.
    // An independent transitive closure decides mutual reachability.
    let pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    for selected in 0_u8..64 {
        let mut nodes = vec![Node::atom(0), Node::atom(1), Node::atom(2), Node::falsum()];
        let mut roots = Vec::new();
        for atom in 0..3 {
            let not = nodes.len();
            nodes.push(Node::implies(atom, 3));
            roots.push(nodes.len());
            nodes.push(Node::or_pair([not, atom]));
        }
        let mut reachable = [[false; 3]; 3];
        for (bit, &(from, to)) in pairs.iter().enumerate() {
            if selected & (1 << bit) != 0 {
                let body = if bit % 2 == 0 {
                    from
                } else {
                    let index = nodes.len();
                    nodes.push(Node::implies(from, 3));
                    index
                };
                roots.push(nodes.len());
                nodes.push(Node::implies(body, to));
                reachable[from][to] = true;
            }
        }
        for via in 0..3 {
            for from in 0..3 {
                for to in 0..3 {
                    reachable[from][to] |= reachable[from][via] && reachable[via][to];
                }
            }
        }
        let theory = theory(3, nodes, roots);
        let order = prepare(&theory).result.unwrap().unwrap();
        for (from, row) in reachable.iter().enumerate() {
            for (to, &reaches) in row.iter().enumerate() {
                if reaches {
                    assert!(order.ranks[from] <= order.ranks[to]);
                }
                assert_eq!(
                    order.ranks[from] == order.ranks[to],
                    from == to || reaches && reachable[to][from],
                    "graph {selected}"
                );
            }
        }
        // Ties inside one component must be resolved by semantic atom identity.
        if selected == 63 {
            assert_eq!(choose(&order, &theory, &Region::all_open(3)), Some(0));
        }
    }
}

#[test]
fn unused_formulas_preserve_dependency_precedence() {
    // An unrelated tautological root and a dead formula may change ranks of
    // incomparable components, but cannot reverse b's required precedence.
    let theory = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::implies(1, 2),
            Node::or_pair([5, 1]),
            Node::implies(1, 4),
            Node::atom(2),
            Node::implies(8, 8),
            Node::implies(0, 8),
        ],
        vec![6, 7, 9],
    );
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(order.ranks[1] < order.ranks[0]);
    assert_eq!(choose(&order, &theory, &Region::all_open(3)), Some(1));
}

#[test]
fn an_order_refuses_an_unrelated_same_shape_owner() {
    let theory = upstream();
    let other = upstream();
    let order = prepare(&theory).result.unwrap().unwrap();
    assert!(order.belongs_to(&theory.clone()));
    let narrower = Narrower::new(&theory);
    let known = narrower.knowledge();
    let mut work = crate::regions::Meter::local(u64::MAX);
    assert_eq!(
        order
            .preferred(&narrower, &other, &Region::all_open(2), &known, &mut work)
            .map_err(zetesis_algebra::Failure::into_stop),
        Err(Stop::InvalidProgram)
    );
    assert_eq!(
        order
            .preferred(&narrower, &theory, &Region::all_open(3), &known, &mut work)
            .map_err(zetesis_algebra::Failure::into_stop),
        Err(Stop::InvalidProgram)
    );
    assert_eq!(work.charged(), 0);
}

#[test]
fn unsupported_heads_decline_before_graph_admission() {
    let theory = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::falsum(),
            Node::implies(0, 2),
            Node::or_pair([0, 3]),
            Node::and_pair([0, 1]),
        ],
        vec![4, 5],
    );
    let attempt = prepare(&theory);
    assert!(attempt.result.unwrap().is_none());
    assert_eq!((attempt.work, attempt.peak_bytes), (0, 0));
}

#[test]
fn a_theory_without_generators_avoids_preparation() {
    let theory = theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![0, 2],
    );
    let attempt = prepare(&theory);
    assert!(attempt.result.unwrap().is_none());
    assert_eq!((attempt.work, attempt.peak_bytes), (0, 0));
}

struct Allowance {
    remaining: u64,
    batch: u64,
    spent: u64,
}

impl zetesis_algebra::Quota for Allowance {
    type Error = Stop;
    fn reserve(&mut self, wanted: u64) -> Result<u64, Stop> {
        let grant = wanted.min(self.remaining).min(self.batch);
        if grant == 0 {
            return Err(Stop::WorkLimit);
        }
        self.remaining -= grant;
        self.spent += grant;
        Ok(grant)
    }
    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
        self.spent -= unspent;
    }
}

#[test]
fn every_preparation_prefix_has_exact_quota_work() {
    let theory = upstream();
    let extraction = extract(&theory);
    let original = OriginalSubject::new(&theory, extraction.producers.as_ref());
    let expected = prepare(&theory);
    let needed = expected.work;
    let expected = expected.result.unwrap().unwrap();
    for limit in [0, 1, 8, needed / 2, needed - 1, needed, needed + 5] {
        for batch in [1, 7, 16_384] {
            let mut quota = Allowance {
                remaining: limit,
                batch,
                spent: 0,
            };
            let attempt = crate::test_support::stop_generator(GeneratorOrder::prepare_reserved(
                original,
                usize::MAX,
                &Cancellation::default(),
                &mut zetesis_algebra::Meter::reserved(&mut quota),
            ));
            assert_eq!(attempt.work, limit.min(needed));
            assert_eq!(quota.spent, attempt.work);
            if limit < needed {
                assert!(matches!(
                    attempt.result,
                    Err(GeneratorOrderError::Stopped(Stop::WorkLimit))
                ));
            } else {
                let order = attempt.result.unwrap().unwrap();
                assert_eq!(order.heads, expected.heads);
                assert_eq!(order.ranks, expected.ranks);
                assert_eq!(order.depths, expected.depths);
            }
        }
    }
}

/// A chain of `n` choices, each guarded by the next: upstream's shape
/// stretched, so the order's arrays run past several observation boundaries.
fn chain(n: usize) -> Theory {
    let falsum = n;
    let mut nodes: Vec<Node> = (0..n).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let mut choices = Vec::with_capacity(n);
    for atom in 0..n {
        let absent = nodes.len();
        nodes.push(Node::implies(atom, falsum));
        choices.push(nodes.len());
        nodes.push(Node::or_pair([atom, absent]));
    }
    let mut roots = vec![choices[n - 1]];
    for (atom, &choice) in choices.iter().enumerate().take(n - 1) {
        roots.push(nodes.len());
        nodes.push(Node::implies(atom + 1, choice));
    }
    theory(n, nodes, roots)
}

#[test]
fn every_prefix_across_observation_boundaries_has_exact_quota_work() {
    // The filled arrays here are longer than POLL_INTERVAL, so their charged
    // runs end at observation boundaries and wherever the account refills.
    // Each limit within two operations of a boundary or of a refill, against
    // accounts that refill below, at and above the boundary, refuses at
    // exactly its prefix, as a tick per copy would.
    let theory = chain(150);
    let extraction = extract(&theory);
    let original = OriginalSubject::new(&theory, extraction.producers.as_ref());
    let expected = prepare(&theory);
    let needed = expected.work;
    assert!(
        needed > 4 * zetesis_algebra::POLL_INTERVAL,
        "needed {needed}"
    );
    let expected = expected.result.unwrap().unwrap();
    let interval = zetesis_algebra::POLL_INTERVAL;
    for batch in [1, 7, interval - 1, interval, interval + 1, 16_384] {
        let near = |limit: u64, step: u64| {
            let offset = limit % step;
            offset <= 2 || step - offset <= 2
        };
        let limits = (0..=needed + 1).filter(|&limit| {
            limit + 2 >= needed
                || near(limit, interval)
                || (batch >= interval && near(limit, batch))
        });
        for limit in limits {
            let mut quota = Allowance {
                remaining: limit,
                batch,
                spent: 0,
            };
            let attempt = crate::test_support::stop_generator(GeneratorOrder::prepare_reserved(
                original,
                usize::MAX,
                &Cancellation::default(),
                &mut zetesis_algebra::Meter::reserved(&mut quota),
            ));
            assert_eq!(
                attempt.work,
                limit.min(needed),
                "batch {batch} limit {limit}"
            );
            assert_eq!(quota.spent, attempt.work, "batch {batch} limit {limit}");
            if limit < needed {
                assert!(
                    matches!(
                        attempt.result,
                        Err(GeneratorOrderError::Stopped(Stop::WorkLimit))
                    ),
                    "batch {batch} limit {limit}"
                );
            } else {
                let order = attempt.result.unwrap().unwrap();
                assert_eq!(order.heads, expected.heads);
                assert_eq!(order.ranks, expected.ranks);
                assert_eq!(order.depths, expected.depths);
            }
        }
    }
}

#[test]
fn storage_refusal_keeps_its_spent_prefix() {
    let theory = upstream();
    let extraction = extract(&theory);
    let original = OriginalSubject::new(&theory, extraction.producers.as_ref());
    let completed = prepare(&theory);
    let peak = usize::try_from(completed.peak_bytes).unwrap();
    let order = completed.result.unwrap().unwrap();
    assert!(order.retained_bytes() < completed.peak_bytes);
    for allowance in [0, peak - 1, peak] {
        let mut quota = Allowance {
            remaining: u64::MAX,
            batch: 7,
            spent: 0,
        };
        let attempt = crate::test_support::stop_generator(GeneratorOrder::prepare_reserved(
            original,
            allowance,
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut quota),
        ));
        assert_eq!(quota.spent, attempt.work);
        if allowance < peak {
            assert!(
                matches!(attempt.result, Err(GeneratorOrderError::Storage { required, limit })
                if required > limit as u128 && limit == allowance)
            );
        } else {
            let admitted = attempt.result.unwrap().unwrap();
            assert_eq!(admitted.ranks, order.ranks);
            assert_eq!(admitted.depths, order.depths);
        }
    }
}

#[test]
fn cancellation_precedes_the_inapplicable_fast_path() {
    let theory = upstream();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let attempt = GeneratorOrder::prepare(
        OriginalSubject::new(&theory, None),
        GeneratorOrderLimits {
            max_bytes: 0,
            max_work: 0,
        },
        &cancellation,
    );
    assert!(matches!(
        attempt.result,
        Err(GeneratorOrderError::Stopped(Stop::Cancelled))
    ));
    assert_eq!((attempt.work, attempt.peak_bytes), (0, 0));
}

#[test]
fn native_body_conditions_precede_their_generated_head() {
    use crate::{ExtremumCut, ExtremumCutRule, ExtremumRow, FormulaNodes, NodeView, Threshold};

    let rule = ExtremumCutRule::Suffix(2);
    let guards = [
        NodeView::Threshold(Threshold {
            conditions: &[1, 2, 1],
            weights: &[1, 0, 1],
            bound: 1,
        }),
        NodeView::ExtremumCut(ExtremumCut {
            row: ExtremumRow {
                conditions: &[1, 2, 1],
                ranks: &[0, 2, 1],
                rank_count: 3,
            },
            rule: &rule,
        }),
    ];
    for guard in guards {
        // The scheduling graph deliberately includes every admitted occurrence,
        // including repeated and zero-weight numeric conditions. It does not
        // claim to compute only logically necessary dependency edges.
        let mut nodes = FormulaNodes::new(
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::atom(2),
                    Node::falsum(),
                    Node::implies(0, 3),
                    Node::or_pair([0, 4]),
                ],
                vec![],
            )
            .unwrap(),
        );
        let mut transaction = nodes.transaction();
        let body = transaction.push(guard, usize::MAX, usize::MAX).unwrap();
        let root = transaction
            .push(NodeView::Implies(body, 5), usize::MAX, usize::MAX)
            .unwrap();
        transaction.commit();
        let theory = Theory::new(
            3,
            nodes.into_parts(),
            vec![root],
            AdmissionLimits::default(),
        )
        .unwrap();
        let order = prepare(&theory).result.unwrap().unwrap();
        assert!(order.ranks[1] < order.ranks[0]);
        assert!(order.ranks[2] < order.ranks[0]);
    }
}

#[test]
fn a_refused_selection_publishes_no_partial_winner() {
    let theory = upstream();
    let order = prepare(&theory).result.unwrap().unwrap();
    let narrower = Narrower::new(&theory);
    for limit in 0..=3 {
        let mut quota = Allowance {
            remaining: limit,
            batch: 7,
            spent: 0,
        };
        let attempt = crate::test_support::stop_preference(&order.preferred_atom_reserved(
            &narrower,
            &theory,
            &Region::all_open(2),
            &narrower.knowledge(),
            &Cancellation::default(),
            &mut zetesis_algebra::Meter::reserved(&mut quota),
        ));
        assert_eq!(attempt.work, limit.min(2));
        assert_eq!(quota.spent, attempt.work);
        assert_eq!(
            attempt.result,
            if limit < 2 {
                Err(Stop::WorkLimit)
            } else {
                Ok(Some(1))
            }
        );
    }
}

#[test]
fn a_refused_byte_observation_records_peak_without_committing_current() {
    let cancellation = Cancellation::default();
    let mut work = zetesis_algebra::Meter::local(0);
    let mut budget = super::Budget {
        max_bytes: 16,
        cancellation: &cancellation,
        work: &mut work,
        bytes: 5,
        peak: 5,
    };
    assert_eq!(
        budget.observe(20),
        Err(GeneratorOrderError::Storage {
            required: 20,
            limit: 16
        })
    );
    assert_eq!(
        (budget.bytes, budget.peak, budget.work.charged()),
        (5, 20, 0)
    );
}

#[test]
fn historical_peak_does_not_refuse_a_fitting_unpolled_observation() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut work = zetesis_algebra::Meter::local(0);
    let mut budget = super::Budget {
        max_bytes: 16,
        cancellation: &cancellation,
        work: &mut work,
        bytes: 5,
        peak: 20,
    };
    for bytes in [0, 16] {
        assert_eq!(budget.observe(bytes), Ok(()));
        assert_eq!(
            (budget.bytes, budget.peak, budget.work.charged()),
            (bytes, 20, 0)
        );
    }
}

/// An iterator that counts how many times its clones are started: the number
/// of traversals a consumer makes of the incidence events.
#[derive(Clone)]
struct Counted<I> {
    inner: I,
    started: bool,
    traversals: std::rc::Rc<std::cell::Cell<usize>>,
}

impl<I: Iterator> Iterator for Counted<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            self.traversals.set(self.traversals.get() + 1);
        }
        self.inner.next()
    }
}

#[test]
fn the_dependency_graph_reads_the_incidences_twice() {
    // Both adjacencies, outgoing and incoming, are built from two readings
    // of the incidence events, one counting and one placing, not from two
    // per adjacency: the events come through a long chain of adaptors over
    // the theory, and each reading is paid in full.
    let theory = upstream();
    let extraction = extract(&theory);
    let producers = extraction.producers.as_ref().unwrap();
    let traversals = std::rc::Rc::new(std::cell::Cell::new(0));
    let incidences = Counted {
        inner: super::edges(&theory, producers),
        started: false,
        traversals: std::rc::Rc::clone(&traversals),
    };
    let cancellation = Cancellation::default();
    let mut meter = crate::regions::Meter::local(u64::MAX);
    let mut budget = super::Budget {
        max_bytes: usize::MAX,
        cancellation: &cancellation,
        work: &mut meter,
        bytes: 0,
        peak: 0,
    };
    let order = super::build(&theory, producers, incidences, &mut budget).unwrap();
    assert!(order.is_some());
    assert_eq!(traversals.get(), 2);
}
