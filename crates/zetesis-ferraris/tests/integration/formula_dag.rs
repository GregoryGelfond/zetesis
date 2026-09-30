//! Shrinking generated DAGs checked against an independently materialized tree.

use proptest::prelude::*;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FrozenReduct, Interpretation, Limits, Node, Theory, Verdict, check, models,
    models_reduct,
};

const MAX_EXPANSION: usize = 1024;

#[derive(Clone, Debug)]
struct Case {
    atoms: usize,
    nodes: Vec<Node>,
    roots: Vec<usize>,
    candidate: u8,
    tested: u8,
}

fn cases() -> impl Strategy<Value = Case> {
    (
        1usize..=6,
        prop::collection::vec(any::<(u8, u8, u8)>(), 1..=48),
        prop::collection::vec(any::<u8>(), 1..=4),
        any::<u8>(),
        any::<u8>(),
        any::<[u8; 6]>(),
        any::<bool>(),
    )
        .prop_map(
            |(atoms, instructions, roots, candidate, tested, keys, deep)| {
                let mut permutation: Vec<_> = (0..atoms).collect();
                permutation.sort_unstable_by_key(|atom| (keys[*atom], *atom));
                let mut nodes = Vec::new();
                let mut costs = Vec::new();
                for (index, (operation, left, right)) in instructions.into_iter().enumerate() {
                    let atom = Node::Atom(permutation[usize::from(left) % atoms]);
                    let operation = if deep && index > 0 {
                        operation % 3 + 2
                    } else {
                        operation % 5
                    };
                    let (node, cost) = if operation < 2 || index == 0 {
                        (if operation == 0 { Node::False } else { atom }, 1)
                    } else {
                        let a = if deep {
                            index - 1
                        } else {
                            usize::from(left) % index
                        };
                        let b = if deep {
                            0
                        } else if right % 4 == 0 {
                            a
                        } else {
                            usize::from(right) % index
                        };
                        let cost = 1 + costs[a] + costs[b];
                        if cost > MAX_EXPANSION {
                            (atom, 1)
                        } else {
                            (
                                match operation {
                                    2 => Node::And(a, b),
                                    3 => Node::Or(a, b),
                                    _ => Node::Implies(a, b),
                                },
                                cost,
                            )
                        }
                    };
                    nodes.push(node);
                    costs.push(cost);
                }
                let roots = roots
                    .into_iter()
                    .map(|root| usize::from(root) % nodes.len())
                    .collect();
                let mask = (1u8 << atoms) - 1;
                Case {
                    atoms,
                    nodes,
                    roots,
                    candidate: candidate & mask,
                    tested: tested & mask,
                }
            },
        )
}

#[derive(Clone, Debug)]
enum Tree {
    False,
    Atom(usize),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
}

impl Tree {
    // Expands shared nodes into separate tree occurrences. The generator caps
    // expansion per root independently of the DAG's forty-eight-node bound.
    fn expand(nodes: &[Node], root: usize) -> Self {
        match nodes[root] {
            Node::False => Self::False,
            Node::Atom(atom) => Self::Atom(atom),
            Node::And(a, b) => Self::And(
                Box::new(Self::expand(nodes, a)),
                Box::new(Self::expand(nodes, b)),
            ),
            Node::Or(a, b) => Self::Or(
                Box::new(Self::expand(nodes, a)),
                Box::new(Self::expand(nodes, b)),
            ),
            Node::Implies(a, b) => Self::Implies(
                Box::new(Self::expand(nodes, a)),
                Box::new(Self::expand(nodes, b)),
            ),
        }
    }

    fn eval(&self, world: u8) -> bool {
        match self {
            Self::False => false,
            Self::Atom(atom) => world & (1 << atom) != 0,
            Self::And(a, b) => a.eval(world) && b.eval(world),
            Self::Or(a, b) => a.eval(world) || b.eval(world),
            Self::Implies(a, b) => !a.eval(world) || b.eval(world),
        }
    }

    fn reduct(&self, candidate: u8) -> Self {
        if !self.eval(candidate) {
            return Self::False;
        }
        match self {
            Self::False | Self::Atom(_) => self.clone(),
            Self::And(a, b) => {
                Self::And(Box::new(a.reduct(candidate)), Box::new(b.reduct(candidate)))
            }
            Self::Or(a, b) => {
                Self::Or(Box::new(a.reduct(candidate)), Box::new(b.reduct(candidate)))
            }
            Self::Implies(a, b) => {
                Self::Implies(Box::new(a.reduct(candidate)), Box::new(b.reduct(candidate)))
            }
        }
    }
}

fn interpretation(theory: &Theory, world: u8) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| world & (1 << atom) != 0),
    )
    .expect("bounded generated interpretation")
}

proptest! {
    #[test]
    fn generated_shared_and_deep_dags_match_explicit_tree_reduct(case in cases()) {
        let roots: Vec<_> = case.roots.iter().map(|root| Tree::expand(&case.nodes, *root)).collect();
        let reduct: Vec<_> = roots.iter().map(|root| root.reduct(case.candidate)).collect();
        let theory = Theory::new(case.atoms, case.nodes, case.roots, AdmissionLimits::default())
            .expect("generator preserves topological admission");
        let candidate = interpretation(&theory, case.candidate);
        let tested = interpretation(&theory, case.tested);
        let cancellation = Cancellation::default();
        let limits = Limits::default();
        let classical = roots.iter().all(|root| root.eval(case.candidate));
        prop_assert_eq!(models(&theory, &candidate, limits, &cancellation).unwrap(), classical);
        prop_assert_eq!(models_reduct(&theory, &candidate, &tested, limits, &cancellation).unwrap(),
            reduct.iter().all(|root| root.eval(case.tested)));
        let frozen = FrozenReduct::new(&candidate, limits, &cancellation).unwrap();
        for world in 0..1u8 << case.atoms {
            prop_assert_eq!(
                frozen.is_satisfied_by(&interpretation(&theory, world), limits, &cancellation).unwrap(),
                reduct.iter().all(|root| root.eval(world))
            );
        }
        let stable = classical && !(0..1u8 << case.atoms).any(|subset| {
            subset != case.candidate && subset & !case.candidate == 0
                && reduct.iter().all(|root| root.eval(subset))
        });
        let actual = check(&theory, &candidate, limits, &cancellation).unwrap();
        prop_assert_eq!(actual.accepted(), stable);
        if let Verdict::NonMinimal { witness } = actual.verdict() {
            let subset = witness.atoms().fold(0u8, |bits, atom| bits | (1 << atom));
            prop_assert!(witness.theory().same_instance(&theory));
            prop_assert_ne!(subset, case.candidate);
            prop_assert_eq!(subset & !case.candidate, 0);
            prop_assert!(reduct.iter().all(|root| root.eval(subset)));
        }
    }

    #[test]
    fn frozen_queries_obey_materialized_root_work(case in cases()) {
        let reduct: Vec<_> = case.roots.iter()
            .map(|root| Tree::expand(&case.nodes, *root).reduct(case.candidate))
            .collect();
        let root_tests = reduct.iter().position(|root| !root.eval(case.tested))
            .map_or(reduct.len(), |failed| failed + 1);
        let theory = Theory::new(case.atoms, case.nodes, case.roots, AdmissionLimits::default())
            .unwrap();
        let candidate = interpretation(&theory, case.candidate);
        let tested = interpretation(&theory, case.tested);
        let node_work = u64::try_from(theory.nodes().len()).unwrap();
        let tested_work = node_work + u64::try_from(root_tests).unwrap();
        let limits = |max_work| Limits { max_work, max_subsets: 0 };
        let cancellation = Cancellation::default();
        let frozen = FrozenReduct::new(&candidate, limits(node_work), &cancellation).unwrap();
        let expected = reduct.iter().all(|root| root.eval(case.tested));
        prop_assert_eq!(frozen.is_satisfied_by(&tested, limits(tested_work), &cancellation).unwrap(), expected);
        prop_assert_eq!(
            frozen.is_satisfied_by(&tested, limits(tested_work - 1), &cancellation).unwrap_err(),
            zetesis_cpu::Stop::WorkLimit
        );
        prop_assert_eq!(
            models_reduct(&theory, &candidate, &tested, limits(node_work + tested_work), &cancellation).unwrap(),
            expected
        );
        prop_assert_eq!(
            models_reduct(&theory, &candidate, &tested, limits(node_work + tested_work - 1), &cancellation).unwrap_err(),
            zetesis_cpu::Stop::WorkLimit
        );
    }
}
