//! Independent finite-tree reduct checks and certificate refusal boundaries.

use std::time::{Duration, Instant};

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    AdmissionLimits, Interpretation, Node, Theory, TightCheckLimits, TightError, TightPlan,
    TightPlanLimits, TightProducerKind, TightResource, TightVerdict,
};

#[derive(Clone, Debug)]
enum Tree {
    Atom(usize),
    False,
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Imp(Box<Self>, Box<Self>),
}
impl Tree {
    fn atom(atom: usize) -> Self {
        Self::Atom(atom)
    }
    fn neg(self) -> Self {
        self.imp(Self::False)
    }
    fn and(self, other: Self) -> Self {
        Self::And(Box::new(self), Box::new(other))
    }
    fn or(self, other: Self) -> Self {
        Self::Or(Box::new(self), Box::new(other))
    }
    fn imp(self, other: Self) -> Self {
        Self::Imp(Box::new(self), Box::new(other))
    }
    fn truth(&self, world: usize) -> bool {
        match self {
            Self::Atom(atom) => world & (1 << atom) != 0,
            Self::False => false,
            Self::And(a, b) => a.truth(world) && b.truth(world),
            Self::Or(a, b) => a.truth(world) || b.truth(world),
            Self::Imp(a, b) => !a.truth(world) || b.truth(world),
        }
    }
    fn reduct(&self, candidate: usize) -> Self {
        if !self.truth(candidate) {
            return Self::False;
        }
        match self {
            Self::Atom(_) | Self::False => self.clone(),
            Self::And(a, b) => a.reduct(candidate).and(b.reduct(candidate)),
            Self::Or(a, b) => a.reduct(candidate).or(b.reduct(candidate)),
            Self::Imp(a, b) => a.reduct(candidate).imp(b.reduct(candidate)),
        }
    }
    fn emit(&self, nodes: &mut Vec<Node>) -> usize {
        let node = match self {
            Self::Atom(atom) => Node::Atom(*atom),
            Self::False => Node::False,
            Self::And(a, b) => Node::And(a.emit(nodes), b.emit(nodes)),
            Self::Or(a, b) => Node::Or(a.emit(nodes), b.emit(nodes)),
            Self::Imp(a, b) => Node::Implies(a.emit(nodes), b.emit(nodes)),
        };
        nodes.push(node);
        nodes.len() - 1
    }
}
fn theory(atoms: usize, formulas: &[Tree]) -> Theory {
    let mut nodes = Vec::new();
    let roots = formulas.iter().map(|tree| tree.emit(&mut nodes)).collect();
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}
fn interpretation(theory: &Theory, world: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|a| world & (1 << a) != 0),
    )
    .unwrap()
}
fn stable(formulas: &[Tree], candidate: usize) -> bool {
    if !formulas.iter().all(|tree| tree.truth(candidate)) {
        return false;
    }
    let reduced: Vec<_> = formulas.iter().map(|tree| tree.reduct(candidate)).collect();
    !(0..candidate)
        .any(|subset| subset & candidate == subset && reduced.iter().all(|tree| tree.truth(subset)))
}
fn plan(theory: &Theory) -> TightPlan {
    TightPlan::compile(theory, TightPlanLimits::default(), &Cancellation::default()).unwrap()
}
fn compare(theory: &Theory, formulas: &[Tree]) -> usize {
    let plan = plan(theory);
    for candidate in 0..1 << theory.atom_count() {
        let result = plan
            .check(
                &interpretation(theory, candidate),
                TightCheckLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            result.verdict == TightVerdict::Stable,
            stable(formulas, candidate),
            "candidate={candidate}, formulas={formulas:?}"
        );
        match result.verdict {
            TightVerdict::NotModel { root } => {
                let index = theory
                    .roots()
                    .iter()
                    .position(|other| *other == root)
                    .unwrap();
                assert!(!formulas[index].truth(candidate));
            }
            TightVerdict::Residual { unsupported_atom } => {
                assert_ne!(candidate & (1 << unsupported_atom), 0);
                assert!(formulas.iter().all(|tree| tree.truth(candidate)));
            }
            TightVerdict::Stable => {
                // Check every J, not merely the first refutation encountered.
                for tested in 0..1 << theory.atom_count() {
                    if tested != candidate && tested & candidate == tested {
                        assert!(
                            formulas
                                .iter()
                                .any(|tree| !tree.reduct(candidate).truth(tested))
                        );
                    }
                }
            }
        }
    }
    1 << theory.atom_count()
}

#[test]
fn every_small_normal_choice_pair_matches_independently_materialized_reducts() {
    let a = Tree::atom(0);
    let b = Tree::atom(1);
    let bodies = [
        Tree::False.neg(),
        Tree::False,
        a.clone(),
        b.clone(),
        a.clone().neg(),
        b.clone().neg(),
        a.clone().neg().neg(),
        b.clone().neg().neg(),
        a.clone().and(b.clone()),
        a.clone().or(b.clone()),
        a.clone().imp(b.clone()).neg(),
        a.clone().imp(b.clone()).neg().neg(),
        a.clone().neg().or(b.clone()),
        a.clone().and(a.clone()),
    ];
    let rules: Vec<_> = bodies
        .iter()
        .flat_map(|body| {
            [a.clone(), b.clone()].into_iter().flat_map(move |head| {
                [
                    body.clone().imp(head.clone()),
                    body.clone().imp(head.clone().or(head.neg())),
                ]
            })
        })
        .collect();
    let mut worlds = 0;
    let mut cyclic = 0;
    for first in &rules {
        for second in &rules {
            let formulas = [first.clone(), second.clone()];
            let theory = theory(3, &formulas); // One unsupported carrier atom.
            match TightPlan::compile(
                &theory,
                TightPlanLimits::default(),
                &Cancellation::default(),
            ) {
                Ok(_) => worlds += compare(&theory, &formulas),
                Err(TightError::PositiveCycle { .. }) => cyclic += 1,
                result => panic!("unexpected shape refusal: {result:?}"),
            }
        }
    }
    assert_eq!(worlds, 12_544);
    assert_eq!(cyclic, 1_568);
}

#[test]
fn constraints_candidate_guards_and_both_choice_orders_remain_original_formulas() {
    let a = Tree::atom(0);
    let b = Tree::atom(1);
    let cases = [
        vec![],
        vec![Tree::False],
        vec![Tree::False.neg()],
        vec![a.clone()],
        vec![a.clone().or(a.clone().neg())],
        vec![a.clone().neg().or(a.clone())],
        vec![a.clone().neg().neg().imp(a.clone())],
        vec![
            a.clone().or(a.clone().neg()),
            a.clone().imp(b.clone()),
            b.clone().imp(a.clone()).neg().neg(),
        ],
        vec![
            a.clone().or(a.clone().neg()),
            a.clone().imp(b.clone()).neg(),
        ],
        vec![
            a.clone().or(a.clone().neg()),
            a.clone().and(b.clone()).neg(),
        ],
        vec![a.clone().neg(), b.clone().neg().neg()],
    ];
    for formulas in cases {
        compare(&theory(3, &formulas), &formulas);
    }
    let source = theory(
        2,
        &[Tree::False.neg().imp(a.clone()), b.clone().or(b.neg())],
    );
    let plan = plan(&source);
    assert_eq!(plan.producers()[0].kind(), TightProducerKind::Normal);
    assert_eq!(plan.producers()[1].kind(), TightProducerKind::Choice);
    assert_eq!(plan.producers()[0].head(), 0);
    assert_eq!(plan.producers()[0].root(), source.roots()[0]);
    assert!(plan.producers()[0].body().is_some());
    assert_eq!(plan.producers()[1].body(), None);
    assert!(source.same_instance(plan.theory()));
}

#[test]
fn unnegated_implications_disjunctions_cycles_and_missing_root_coverage_refuse() {
    let a = Tree::atom(0);
    let b = Tree::atom(1);
    let cases = [
        a.clone().or(b.clone()),
        a.clone().and(b.clone()),
        a.clone().imp(a.clone()),
        a.clone().imp(a.clone().or(a.clone().neg())),
        a.clone().imp(a.clone()).imp(a.clone()), // p :- p:p.
        a.clone().or(b.clone().neg()),           // Not an atomic choice.
    ];
    for formula in cases {
        let source = theory(2, &[formula]);
        assert!(
            TightPlan::compile(
                &source,
                TightPlanLimits::default(),
                &Cancellation::default()
            )
            .is_err()
        );
    }
    let source = theory(2, &[a.clone(), b.clone().imp(b)]);
    assert!(matches!(
        TightPlan::compile(
            &source,
            TightPlanLimits::default(),
            &Cancellation::default()
        ),
        Err(TightError::PositiveCycle { atom: 1 })
    ));
    let source = theory(2, &[a.clone(), a.or(Tree::atom(1))]);
    assert_eq!(
        TightPlan::compile(
            &source,
            TightPlanLimits::default(),
            &Cancellation::default()
        )
        .unwrap_err(),
        TightError::UnsupportedRoot {
            root: source.roots()[1]
        }
    );
}

#[test]
fn supplied_ranks_and_theory_identity_are_checked() {
    let a = Tree::atom(0);
    let b = Tree::atom(1);
    let formulas = [a.clone(), a.imp(b)];
    let source = theory(3, &formulas);
    let cancellation = Cancellation::default();
    let limits = TightPlanLimits::default();
    let generated = plan(&source);
    assert_eq!(generated.ranks(), &[0, 1, 0]);
    assert!(TightPlan::certify(&source, &[1, 2, 2], limits, &cancellation).is_ok());
    for ranks in [&[0, 0, 0][..], &[2, 1, 0]] {
        assert_eq!(
            TightPlan::certify(&source, ranks, limits, &cancellation).unwrap_err(),
            TightError::RankOrder { head: 1 }
        );
    }
    for ranks in [&[][..], &[0, 1], &[0, 1, 3]] {
        assert_eq!(
            TightPlan::certify(&source, ranks, limits, &cancellation).unwrap_err(),
            TightError::RankShape
        );
    }
    let independent = theory(3, &formulas);
    assert_eq!(
        generated.check(
            &interpretation(&independent, 3),
            TightCheckLimits::default(),
            &cancellation
        ),
        Err(TightError::Stopped(Stop::WrongProgram))
    );
    assert_eq!(
        generated
            .check(
                &interpretation(&source.clone(), 3),
                TightCheckLimits::default(),
                &cancellation
            )
            .unwrap()
            .verdict,
        TightVerdict::Stable
    );
    assert!(format!("{generated:?}").contains("TightPlan"));
}

#[test]
fn exact_construction_and_check_limits_do_not_turn_partial_work_into_acceptance() {
    let source = theory(2, &[Tree::atom(0), Tree::atom(0).imp(Tree::atom(1))]);
    let cancellation = Cancellation::default();
    let certified = plan(&source);
    let stats = certified.statistics();
    let exact = TightPlanLimits {
        max_producers: 2,
        max_dependencies: stats.dependencies,
        max_bytes: stats.construction_bytes,
        max_work: stats.work,
    };
    assert!(TightPlan::compile(&source, exact, &cancellation).is_ok());
    for (limits, resource) in [
        (
            TightPlanLimits {
                max_producers: 1,
                ..exact
            },
            TightResource::Producers,
        ),
        (
            TightPlanLimits {
                max_dependencies: stats.dependencies - 1,
                ..exact
            },
            TightResource::Dependencies,
        ),
        (
            TightPlanLimits {
                max_bytes: stats.construction_bytes - 1,
                ..exact
            },
            TightResource::Bytes,
        ),
        (
            TightPlanLimits {
                max_work: stats.work - 1,
                ..exact
            },
            TightResource::Work,
        ),
        (
            TightPlanLimits {
                max_bytes: 0,
                ..exact
            },
            TightResource::Bytes,
        ),
    ] {
        let error = TightPlan::compile(&source, limits, &cancellation).unwrap_err();
        assert_eq!(error, TightError::Limit(resource));
        assert!(!error.to_string().is_empty());
    }
    let candidate = interpretation(&source, 3);
    let checked = certified
        .check(&candidate, TightCheckLimits::default(), &cancellation)
        .unwrap();
    let exact = TightCheckLimits {
        max_work: checked.work,
        max_bytes: checked.logical_bytes,
    };
    assert_eq!(
        certified
            .check(&candidate, exact, &cancellation)
            .unwrap()
            .verdict,
        TightVerdict::Stable
    );
    assert_eq!(
        certified.check(
            &candidate,
            TightCheckLimits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &cancellation
        ),
        Err(TightError::Limit(TightResource::Work))
    );
    assert_eq!(
        certified.check(
            &candidate,
            TightCheckLimits {
                max_bytes: exact.max_bytes - 1,
                ..exact
            },
            &cancellation
        ),
        Err(TightError::Limit(TightResource::Bytes))
    );
}

#[test]
fn cancellation_and_deadlines_refuse_certification_and_evaluation() {
    let source = theory(2, &[Tree::atom(0), Tree::atom(0).imp(Tree::atom(1))]);
    let certified = plan(&source);
    let candidate = interpretation(&source, 3);
    let exact = TightCheckLimits::default();
    for (cancellation, expected) in [
        (
            {
                let cancellation = Cancellation::default();
                cancellation.cancel();
                cancellation
            },
            Stop::Cancelled,
        ),
        (
            Cancellation::with_deadline(
                Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
            )
            .unwrap(),
            Stop::Deadline,
        ),
    ] {
        assert_eq!(
            TightPlan::compile(&source, TightPlanLimits::default(), &cancellation).unwrap_err(),
            TightError::Stopped(expected)
        );
        assert_eq!(
            certified.check(&candidate, exact, &cancellation),
            Err(TightError::Stopped(expected))
        );
    }
}

#[test]
fn shared_deep_bodies_are_linear_graphs_with_matched_duplicate_edges() {
    let mut nodes = vec![Node::Atom(0), Node::Atom(1)];
    let mut body = 0;
    for _ in 0..20_000 {
        nodes.push(Node::And(body, body));
        body = nodes.len() - 1;
    }
    nodes.push(Node::Implies(body, 1));
    let last = nodes.len() - 1;
    let source = Theory::new(2, nodes, vec![0, last], AdmissionLimits::default()).unwrap();
    let plan = plan(&source);
    assert_eq!(plan.ranks(), &[0, 1]);
    assert_eq!(plan.statistics().dependencies, 40_003);
    assert_eq!(
        plan.check(
            &interpretation(&source, 3),
            TightCheckLimits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .verdict,
        TightVerdict::Stable
    );
}

#[test]
fn empty_universes_and_ranked_chains_keep_every_carrier_atom_explicit() {
    let empty = theory(0, &[]);
    let zero = TightPlanLimits {
        max_producers: 0,
        max_dependencies: 0,
        max_bytes: 0,
        max_work: 0,
    };
    let compiled = TightPlan::compile(&empty, zero, &Cancellation::default()).unwrap();
    assert_eq!(compiled.ranks(), &[]);
    assert!(TightPlan::certify(&empty, &[], zero, &Cancellation::default()).is_ok());
    assert_eq!(
        compiled
            .check(
                &interpretation(&empty, 0),
                TightCheckLimits {
                    max_bytes: 0,
                    max_work: 0,
                },
                &Cancellation::default()
            )
            .unwrap()
            .verdict,
        TightVerdict::Stable
    );
    let chain = [
        Tree::atom(2),
        Tree::atom(2).imp(Tree::atom(0)),
        Tree::atom(0).imp(Tree::atom(3)),
        Tree::atom(3).imp(Tree::atom(1)),
    ];
    let source = theory(4, &chain);
    let compiled = plan(&source);
    assert_eq!(compiled.ranks(), &[1, 3, 0, 2]);
    compare(&source, &chain);
    let cycle = theory(
        3,
        &[
            Tree::atom(2).imp(Tree::atom(0)),
            Tree::atom(0).imp(Tree::atom(1)),
            Tree::atom(1).imp(Tree::atom(2)),
        ],
    );
    assert!(matches!(
        TightPlan::compile(&cycle, TightPlanLimits::default(), &Cancellation::default()),
        Err(TightError::PositiveCycle { .. })
    ));
}
