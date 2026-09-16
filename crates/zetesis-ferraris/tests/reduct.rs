//! Independent syntax-tree reduct and finite model enumeration.

use std::time::Instant;
use zetesis_cpu::{Control, Stop};
use zetesis_ferraris::{
    AdmissionError, AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FrozenReduct,
    Interpretation, Limits, Node, Theory, Verdict, check, models, models_reduct,
};

#[derive(Clone, Debug)]
enum Expr {
    False,
    Atom(u8),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Imp(Box<Self>, Box<Self>),
}
impl Expr {
    fn eval(&self, world: u8) -> bool {
        match self {
            Self::False => false,
            Self::Atom(atom) => world & (1 << atom) != 0,
            Self::And(a, b) => a.eval(world) && b.eval(world),
            Self::Or(a, b) => a.eval(world) || b.eval(world),
            Self::Imp(a, b) => !a.eval(world) || b.eval(world),
        }
    }
    fn reduct(&self, world: u8) -> Self {
        if !self.eval(world) {
            return Self::False;
        }
        match self {
            Self::False | Self::Atom(_) => self.clone(),
            Self::And(a, b) => Self::And(Box::new(a.reduct(world)), Box::new(b.reduct(world))),
            Self::Or(a, b) => Self::Or(Box::new(a.reduct(world)), Box::new(b.reduct(world))),
            Self::Imp(a, b) => Self::Imp(Box::new(a.reduct(world)), Box::new(b.reduct(world))),
        }
    }
    fn emit(&self, nodes: &mut Vec<Node>) -> usize {
        let node = match self {
            Self::False => Node::False,
            Self::Atom(atom) => Node::Atom(usize::from(*atom)),
            Self::And(a, b) => Node::And(a.emit(nodes), b.emit(nodes)),
            Self::Or(a, b) => Node::Or(a.emit(nodes), b.emit(nodes)),
            Self::Imp(a, b) => Node::Implies(a.emit(nodes), b.emit(nodes)),
        };
        nodes.push(node);
        nodes.len() - 1
    }
}
fn theory(formulas: &[Expr]) -> Theory {
    let mut nodes = Vec::new();
    let roots = formulas.iter().map(|expr| expr.emit(&mut nodes)).collect();
    Theory::new(2, nodes, roots, AdmissionLimits::default()).unwrap()
}
fn interpretation(theory: &Theory, world: u8) -> Interpretation {
    Interpretation::new(theory, (0..2).filter(|atom| world & (1 << atom) != 0)).unwrap()
}
fn compare(formulas: &[Expr]) {
    let program = theory(formulas);
    let control = Control::default();
    let mut workspace = EvaluationWorkspace::default();
    for candidate in 0..4 {
        let model = interpretation(&program, candidate);
        let classical = formulas.iter().all(|expr| expr.eval(candidate));
        assert_eq!(
            models(&program, &model, Limits::default(), &control).unwrap(),
            classical
        );
        let truth = workspace
            .evaluate(&model, EvaluationLimits::default(), &control)
            .result
            .unwrap();
        for (root, formula) in program.roots().iter().zip(formulas) {
            assert_eq!(truth.node_truth(*root), Some(formula.eval(candidate)));
        }
        assert_eq!(truth.is_model(), classical);
        let reduct: Vec<_> = formulas.iter().map(|expr| expr.reduct(candidate)).collect();
        let frozen = FrozenReduct::new(&model, Limits::default(), &control).unwrap();
        for tested in 0..4 {
            let actual = models_reduct(
                &program,
                &model,
                &interpretation(&program, tested),
                Limits::default(),
                &control,
            )
            .unwrap();
            assert_eq!(
                actual,
                reduct.iter().all(|expr| expr.eval(tested)),
                "M={candidate}, J={tested}, {formulas:?}"
            );
            assert_eq!(
                frozen
                    .is_satisfied_by(
                        &interpretation(&program, tested),
                        Limits::default(),
                        &control,
                    )
                    .unwrap(),
                actual,
                "reused freeze: M={candidate}, J={tested}, {formulas:?}"
            );
        }
        let stable = classical
            && !(0..4).any(|tested| {
                tested != candidate
                    && tested & !candidate == 0
                    && reduct.iter().all(|expr| expr.eval(tested))
            });
        let result = check(&program, &model, Limits::default(), &control).unwrap();
        assert_eq!(result.accepted(), stable, "M={candidate}, {formulas:?}");
        if let Verdict::NonMinimal { witness } = result.verdict() {
            let tested = witness.atoms().fold(0u8, |bits, atom| bits | (1 << atom));
            assert_ne!(tested, candidate);
            assert_eq!(tested & !candidate, 0);
            assert!(reduct.iter().all(|expr| expr.eval(tested)));
        }
    }
}

#[test]
fn formula_transform_matches_independent_materialized_reduct() {
    let atoms = vec![Expr::False, Expr::Atom(0), Expr::Atom(1)];
    let mut basis = atoms.clone();
    for left in &atoms {
        for right in &atoms {
            basis.push(Expr::And(Box::new(left.clone()), Box::new(right.clone())));
            basis.push(Expr::Or(Box::new(left.clone()), Box::new(right.clone())));
            basis.push(Expr::Imp(Box::new(left.clone()), Box::new(right.clone())));
        }
    }
    // All pairs of depth-one expressions exercise nested implication and shared
    // truth patterns, including non-Horn reducts and double default negation.
    for left in &basis {
        for right in &basis {
            compare(&[Expr::And(Box::new(left.clone()), Box::new(right.clone()))]);
            compare(&[Expr::Or(Box::new(left.clone()), Box::new(right.clone()))]);
            compare(&[Expr::Imp(Box::new(left.clone()), Box::new(right.clone()))]);
            compare(&[left.clone(), right.clone()]);
        }
    }
}

#[test]
fn disjunctive_reduct_has_incomparable_minimal_models() {
    let program = theory(&[Expr::Or(Box::new(Expr::Atom(0)), Box::new(Expr::Atom(1)))]);
    let control = Control::default();
    assert!(
        check(
            &program,
            &interpretation(&program, 1),
            Limits::default(),
            &control
        )
        .unwrap()
        .accepted()
    );
    assert!(
        check(
            &program,
            &interpretation(&program, 2),
            Limits::default(),
            &control
        )
        .unwrap()
        .accepted()
    );
    let result = check(
        &program,
        &interpretation(&program, 3),
        Limits::default(),
        &control,
    )
    .unwrap();
    let Verdict::NonMinimal { witness } = result.verdict() else {
        panic!("expected proper countermodel")
    };
    assert_eq!(witness.atoms().collect::<Vec<_>>(), vec![0]);
}

#[test]
fn empty_theory_and_empty_candidate_have_exact_boundary() {
    let program = theory(&[]);
    let control = Control::default();
    let limits = Limits {
        max_subsets: 0,
        ..Limits::default()
    };
    assert!(
        check(&program, &interpretation(&program, 0), limits, &control)
            .unwrap()
            .accepted()
    );
    assert_eq!(
        check(&program, &interpretation(&program, 1), limits, &control).unwrap_err(),
        Stop::CandidateLimit
    );
    assert!(matches!(
        check(
            &program,
            &interpretation(&program, 1),
            Limits::default(),
            &control
        )
        .unwrap()
        .verdict(),
        Verdict::NonMinimal { .. }
    ));
}

#[test]
fn work_and_subset_limits_do_not_certify_partial_search() {
    let program = theory(&[Expr::And(Box::new(Expr::Atom(0)), Box::new(Expr::Atom(1)))]);
    let model = interpretation(&program, 3);
    let control = Control::default();
    let complete = check(&program, &model, Limits::default(), &control).unwrap();
    assert!(complete.accepted());
    assert_eq!(complete.statistics().subsets, 3);
    let exact = Limits {
        max_work: complete.statistics().work,
        max_subsets: 3,
    };
    assert!(check(&program, &model, exact, &control).unwrap().accepted());
    assert_eq!(
        check(
            &program,
            &model,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &control
        )
        .unwrap_err(),
        Stop::WorkLimit
    );
    assert_eq!(
        check(
            &program,
            &model,
            Limits {
                max_subsets: 2,
                ..exact
            },
            &control
        )
        .unwrap_err(),
        Stop::CandidateLimit
    );
    control.cancel();
    assert_eq!(
        check(&program, &model, exact, &control).unwrap_err(),
        Stop::Cancelled
    );
    assert_eq!(
        check(
            &program,
            &model,
            exact,
            &Control::with_deadline(Instant::now()).unwrap()
        )
        .unwrap_err(),
        Stop::Deadline
    );
}

#[test]
fn identity_admission_and_word_boundaries() {
    let program = Theory::new(65, vec![], vec![], AdmissionLimits::default()).unwrap();
    let model = Interpretation::new(&program, [0, 64, 64]).unwrap();
    assert_eq!(model.atoms().collect::<Vec<_>>(), vec![0, 64]);
    assert!(!model.contains(65));
    let result = check(
        &program.clone(),
        &model,
        Limits::default(),
        &Control::default(),
    )
    .unwrap();
    assert!(
        matches!(result.verdict(),Verdict::NonMinimal { witness } if witness.atoms().next().is_none())
    );
    let foreign = Theory::new(65, vec![], vec![], AdmissionLimits::default()).unwrap();
    assert_eq!(
        check(&foreign, &model, Limits::default(), &Control::default()).unwrap_err(),
        Stop::WrongProgram
    );
    assert_eq!(
        Interpretation::new(&program, [65]).unwrap_err(),
        AdmissionError::Atom
    );
    assert!(matches!(
        Theory::new(1, vec![Node::And(0, 0)], vec![], AdmissionLimits::default()),
        Err(AdmissionError::Edge)
    ));
    assert!(matches!(
        Theory::new(1, vec![Node::Atom(1)], vec![], AdmissionLimits::default()),
        Err(AdmissionError::Atom)
    ));
    assert!(matches!(
        Theory::new(1, vec![], vec![0], AdmissionLimits::default()),
        Err(AdmissionError::Root)
    ));
}

#[test]
fn shared_dag_nodes_preserve_frozen_truth_under_multiple_roots() {
    // Reuse the same negation, double negation, and choice nodes in several
    // contexts. The independent tree repeats syntax instead of sharing slots.
    let program = Theory::new(
        2,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Implies(2, 1),
            Node::Or(0, 2),
            Node::Implies(3, 0),
            Node::And(4, 5),
        ],
        vec![6, 4, 5],
        AdmissionLimits::default(),
    )
    .unwrap();
    let a = Expr::Atom(0);
    let not_a = Expr::Imp(Box::new(a.clone()), Box::new(Expr::False));
    let double_not_a = Expr::Imp(Box::new(not_a.clone()), Box::new(Expr::False));
    let choice = Expr::Or(Box::new(a.clone()), Box::new(not_a));
    let guarded = Expr::Imp(Box::new(double_not_a), Box::new(a));
    let formulas = [
        Expr::And(Box::new(choice.clone()), Box::new(guarded.clone())),
        choice,
        guarded,
    ];
    let control = Control::default();
    for candidate in 0..4 {
        let model = interpretation(&program, candidate);
        for tested in 0..4 {
            let expected = formulas
                .iter()
                .all(|formula| formula.reduct(candidate).eval(tested));
            assert_eq!(
                models_reduct(
                    &program,
                    &model,
                    &interpretation(&program, tested),
                    Limits::default(),
                    &control
                )
                .unwrap(),
                expected
            );
        }
        // Atom 1 is unsupported. Choices admit precisely the two subsets of {0}.
        assert_eq!(
            check(&program, &model, Limits::default(), &control)
                .unwrap()
                .accepted(),
            candidate < 2
        );
    }
}

#[test]
fn exhaustive_subset_carries_cross_sparse_machine_word_boundaries() {
    let program = Theory::new(
        130,
        vec![
            Node::Atom(0),
            Node::Atom(63),
            Node::Atom(64),
            Node::Atom(129),
            Node::And(0, 1),
            Node::And(2, 3),
            Node::And(4, 5),
        ],
        vec![6],
        AdmissionLimits::default(),
    )
    .unwrap();
    let model = Interpretation::new(&program, [0, 63, 64, 129]).unwrap();
    let control = Control::default();
    let complete = check(&program, &model, Limits::default(), &control).unwrap();
    assert!(complete.accepted());
    // All fifteen proper subsets fail the conjunction. Four selected bits
    // require 26 bit flips to advance the binary counter from 0 to 15.
    assert_eq!(complete.statistics().subsets, 15);
    assert_eq!(complete.statistics().work, 8 + 130 + 15 * 8 + 26);
    let exact = Limits {
        max_work: complete.statistics().work,
        max_subsets: 15,
    };
    assert!(check(&program, &model, exact, &control).unwrap().accepted());
    assert_eq!(
        check(
            &program,
            &model,
            Limits {
                max_subsets: 14,
                ..exact
            },
            &control
        )
        .unwrap_err(),
        Stop::CandidateLimit
    );
    assert_eq!(
        check(
            &program,
            &model,
            Limits {
                max_work: exact.max_work - 1,
                ..exact
            },
            &control
        )
        .unwrap_err(),
        Stop::WorkLimit
    );
}

#[test]
fn classical_and_reduct_entrypoints_reject_foreign_interpretations() {
    let program = theory(&[Expr::Atom(0)]);
    let foreign = theory(&[Expr::Atom(0)]);
    let own = interpretation(&program, 1);
    let other = interpretation(&foreign, 1);
    let limits = Limits::default();
    let control = Control::default();
    assert_eq!(
        models(&program, &other, limits, &control).unwrap_err(),
        Stop::WrongProgram
    );
    assert_eq!(
        models_reduct(&program, &other, &own, limits, &control).unwrap_err(),
        Stop::WrongProgram
    );
    assert_eq!(
        models_reduct(&program, &own, &other, limits, &control).unwrap_err(),
        Stop::WrongProgram
    );
}
