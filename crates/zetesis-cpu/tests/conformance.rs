//! Independent finite reduct/minimal-model checks against the lazy CPU oracle.
//!
//! The reference below scans statically instantiated rules. It does not reuse
//! the CPU join, closure, candidate, or acceptance implementations. Exhaustive
//! model minimality is deliberately restricted to tiny propositional programs.

use std::collections::BTreeSet;

use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, Filter, GroundProgram, GroundRule, Model, Predicate,
    Program, Seed, StaticLimits, Template, Term, Value,
};
use zetesis_cpu::{CandidateLimits, Candidates, Control, Limits, check};

fn pattern(name: &str, terms: Vec<Term>) -> AtomPattern {
    let predicate = Predicate::new(name, terms.len()).expect("nonempty test name");
    AtomPattern::new(predicate, terms).expect("matching arity")
}
fn atom(name: &str, values: Vec<Value>) -> Atom {
    let predicate = Predicate::new(name, values.len()).expect("nonempty test name");
    Atom::new(predicate, values).expect("matching arity")
}
fn number(value: i32) -> Term {
    Term::Constant(Value::Number(value))
}
fn program(templates: Vec<Template>) -> Program {
    Program::new(templates, AdmissionLimits::default()).expect("safe finite test program")
}
fn fact(name: &str, values: Vec<Term>) -> Template {
    Template::new(Some(pattern(name, values)), vec![], vec![], vec![], vec![])
}

fn bit(id: u32) -> u32 {
    1u32 << id
}
fn has_all(model: u32, ids: &[u32]) -> bool {
    ids.iter().all(|id| model & bit(*id) != 0)
}
fn enabled(rule: &GroundRule, frozen: u32) -> bool {
    has_all(frozen, rule.gate_true()) && rule.gate_false().iter().all(|id| frozen & bit(*id) == 0)
}

/// Direct satisfaction of every rule in one frozen positive reduct, constraints
/// included. This definition does not call any closure routine.
fn reduct_model(graph: &GroundProgram, frozen: u32, model: u32) -> bool {
    graph.rules().iter().all(|rule| {
        !enabled(rule, frozen)
            || !has_all(model, rule.positive())
            || rule.head().is_some_and(|head| model & bit(head) != 0)
    })
}

/// Minimality compares the interpretation with every proper subset under the
/// same frozen reduct. No least-closure equivalence is assumed by this checker.
fn definition_stable(graph: &GroundProgram, interpretation: u32) -> bool {
    if !reduct_model(graph, interpretation, interpretation) {
        return false;
    }
    let mut subset = interpretation;
    loop {
        if subset != interpretation && reduct_model(graph, interpretation, subset) {
            return false;
        }
        if subset == 0 {
            return true;
        }
        subset = (subset - 1) & interpretation;
    }
}

fn decode(graph: &GroundProgram, mask: u32) -> Model {
    Model::new(
        graph
            .atoms()
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1u32 << index) != 0)
            .map(|(_, atom)| atom.clone()),
    )
}
fn encode_seed(graph: &GroundProgram, seed: &Seed) -> u32 {
    graph
        .atoms()
        .iter()
        .enumerate()
        .fold(0, |mask, (index, atom)| {
            if seed.contains(atom) {
                mask | (1u32 << index)
            } else {
                mask
            }
        })
}

/// An independently implemented synchronous static scan computes consequences
/// from bottom and then checks constraints and the complete gate projection.
fn direct_closure(graph: &GroundProgram, seed: u32) -> (u32, bool, bool) {
    assert!(
        graph.atom_count() < u32::BITS as usize,
        "small reference only"
    );
    let mut closure = 0;
    loop {
        let next = graph.rules().iter().fold(closure, |next, rule| {
            if enabled(rule, seed) && has_all(closure, rule.positive()) {
                rule.head().map_or(next, |head| next | bit(head))
            } else {
                next
            }
        });
        if next == closure {
            break;
        }
        closure = next;
    }
    let violated = graph.rules().iter().any(|rule| {
        rule.head().is_none() && enabled(rule, seed) && has_all(closure, rule.positive())
    });
    let projection = graph
        .gate_atom_ids()
        .iter()
        .fold(0, |mask, id| mask | (closure & bit(*id)));
    (closure, violated, projection != seed)
}

fn assert_seed_matches_static(program: &Program, graph: &GroundProgram, seed: &Seed) -> bool {
    let expected = direct_closure(graph, encode_seed(graph, seed));
    let actual = check(program, seed, Limits::default(), &Control::default())
        .expect("small program finishes within budget");
    assert_eq!(
        actual.closure(),
        &decode(graph, expected.0),
        "closure: {:?}, seed {:?}",
        program.templates(),
        seed.atoms()
    );
    assert_eq!(
        actual.constraint_violated(),
        expected.1,
        "constraint check: {:?}",
        program.templates()
    );
    assert_eq!(
        actual.seed_mismatch(),
        expected.2,
        "projection check: {:?}",
        program.templates()
    );
    assert_eq!(actual.accepted(), !expected.1 && !expected.2);
    actual.accepted()
}

fn assert_all_small_models(program: &Program) {
    let graph =
        GroundProgram::compile(program, StaticLimits::default()).expect("small static graph");
    assert!(
        graph.atom_count() <= 2,
        "minimality campaign is deliberately tiny"
    );
    let expected: BTreeSet<Vec<Atom>> = (0..(1u32 << graph.atom_count()))
        .filter(|interpretation| definition_stable(&graph, *interpretation))
        .map(|interpretation| {
            decode(&graph, interpretation)
                .atoms()
                .iter()
                .cloned()
                .collect()
        })
        .collect();
    let mut actual = BTreeSet::new();
    let mut seen_seeds = BTreeSet::new();
    for candidate in Candidates::new(program, CandidateLimits::default(), Control::default()) {
        let candidate = candidate.expect("tiny carrier exhausts within budget");
        let key: Vec<_> = candidate.atoms().iter().cloned().collect();
        assert!(
            seen_seeds.insert(key),
            "candidate enumeration duplicated a seed"
        );
        if assert_seed_matches_static(program, &graph, &candidate) {
            let (closure, _, _) = direct_closure(&graph, encode_seed(&graph, &candidate));
            assert!(
                definition_stable(&graph, closure),
                "accepted closure must satisfy independent minimality"
            );
            let model: Vec<_> = decode(&graph, closure).atoms().iter().cloned().collect();
            assert!(
                actual.insert(model),
                "two accepted seeds must not reconstruct the same model"
            );
        }
    }
    assert_eq!(seen_seeds.len(), 1usize << graph.gate_atom_ids().len());
    assert_eq!(
        actual,
        expected,
        "complete models of {:?}",
        program.templates()
    );
}

fn tiny_rule(head: usize, body: u32, gate_code: u32) -> Template {
    let atoms = [pattern("a", vec![]), pattern("b", vec![])];
    let positive = atoms
        .iter()
        .enumerate()
        .filter(|(index, _)| body & (1u32 << index) != 0)
        .map(|(_, atom)| atom.clone())
        .collect();
    let mut gate_true = Vec::new();
    let mut gate_false = Vec::new();
    let mut gate = gate_code;
    for atom in &atoms {
        match gate % 3 {
            1 => gate_true.push(atom.clone()),
            2 => gate_false.push(atom.clone()),
            _ => {}
        }
        gate /= 3;
    }
    let head = head.checked_sub(1).map(|index| atoms[index].clone());
    Template::new(head, positive, gate_true, gate_false, vec![])
}

#[test]
fn exhaustive_two_atom_programs_agree_with_reduct_minimality() {
    // 3 heads (constraint/a/b), 4 positive bodies, 3^2 gate polarities.
    // Every program with zero, one, or two rows (duplicates included) is checked.
    let rules: Vec<_> = (0..3)
        .flat_map(|head| {
            (0..4).flat_map(move |body| (0..9).map(move |gates| tiny_rule(head, body, gates)))
        })
        .collect();
    assert_eq!(rules.len(), 108);
    let mut checked = 1;
    assert_all_small_models(&program(vec![]));
    for (index, first) in rules.iter().enumerate() {
        assert_all_small_models(&program(vec![first.clone()]));
        checked += 1;
        for second in &rules[index..] {
            assert_all_small_models(&program(vec![first.clone(), second.clone()]));
            checked += 1;
        }
    }
    assert_eq!(checked, 5_995, "the full bounded campaign must execute");
}

#[test]
fn positive_cycles_cannot_be_seeded_by_frozen_candidate_bits() {
    let a = pattern("a", vec![]);
    let b = pattern("b", vec![]);
    let program = program(vec![
        Template::new(Some(a.clone()), vec![b.clone()], vec![], vec![], vec![]),
        Template::new(Some(b.clone()), vec![a.clone()], vec![], vec![], vec![]),
        Template::new(None, vec![], vec![a.clone()], vec![a], vec![]),
    ]);
    assert_all_small_models(&program);
    let seed = Seed::new(&program, [atom("a", vec![])]).expect("candidate is syntactically valid");
    let checked =
        check(&program, &seed, Limits::default(), &Control::default()).expect("terminates");
    assert!(checked.closure().atoms().is_empty());
    assert!(checked.seed_mismatch());
    assert!(
        !checked.constraint_violated(),
        "contradictory gate never enables"
    );
}

#[test]
fn zero_atom_programs_still_check_unconditional_constraints() {
    let program = program(vec![Template::new(None, vec![], vec![], vec![], vec![])]);
    assert_all_small_models(&program);
    let seed = Seed::new(&program, []).expect("empty seed");
    let checked = check(&program, &seed, Limits::default(), &Control::default())
        .expect("constraint check completes");
    assert!(checked.constraint_violated());
    assert!(!checked.accepted());
}

#[test]
fn sparse_first_candidate_precedes_carrier_expansion() {
    let program = program(vec![
        fact("d", vec![number(0)]),
        fact("d", vec![number(1)]),
        Template::new(
            None,
            vec![],
            vec![pattern("g", vec![number(0); 32])],
            vec![],
            vec![Filter::Neq(number(0), number(0))],
        ),
    ]);
    let mut candidates = Candidates::new(
        &program,
        CandidateLimits {
            max_candidates: 1,
            max_carrier_atoms: 0,
        },
        Control::default(),
    );
    assert_eq!(candidates.discovered_atoms(), 0);
    let first = candidates
        .next()
        .expect("first result")
        .expect("empty seed requires no carrier storage");
    assert!(first.atoms().is_empty());
    assert_eq!(candidates.discovered_atoms(), 0);
    let checked = check(
        &program,
        &first,
        Limits {
            max_work: 100,
            max_derived_atoms: 2,
        },
        &Control::default(),
    )
    .expect("sparse check ignores the 2^32 false carrier tuples");
    assert!(checked.accepted());
    assert_eq!(checked.closure().atoms().len(), 2);
}

#[test]
fn unsupported_but_carrier_valid_gate_atoms_are_logically_rejected() {
    let program = program(vec![Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("g", vec![number(0)])],
        vec![Filter::Eq(number(0), number(1))],
    )]);
    assert_all_small_models(&program);
    let seed = Seed::new(&program, [atom("g", vec![Value::Number(1)])])
        .expect("symbolic gate tuple is valid");
    let checked =
        check(&program, &seed, Limits::default(), &Control::default()).expect("no backend error");
    assert!(!checked.accepted());
    assert!(checked.seed_mismatch());
    assert!(checked.closure().atoms().is_empty());
}

#[test]
fn relational_backtracking_restores_bindings_and_defers_unbound_gates() {
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    let program = program(vec![
        fact("d", vec![number(0)]),
        fact("d", vec![number(1)]),
        fact("pair", vec![number(0), number(1)]),
        fact("pair", vec![number(1), number(1)]),
        Template::new(
            Some(pattern("keep", vec![x.clone(), y.clone()])),
            vec![
                pattern("d", vec![x.clone()]),
                pattern("pair", vec![x.clone(), y.clone()]),
                pattern("d", vec![y.clone()]),
            ],
            vec![],
            vec![pattern("ban", vec![y.clone()])],
            vec![Filter::Neq(x.clone(), y.clone())],
        ),
        Template::new(
            Some(pattern("diagonal", vec![x.clone()])),
            vec![pattern("pair", vec![x.clone(), x.clone()])],
            vec![],
            vec![],
            vec![],
        ),
    ]);
    let graph = GroundProgram::compile(&program, StaticLimits::default())
        .expect("small static relational graph");
    let seed = Seed::new(&program, []).expect("empty seed");
    assert!(assert_seed_matches_static(&program, &graph, &seed));
    let checked =
        check(&program, &seed, Limits::default(), &Control::default()).expect("finite closure");
    assert!(
        checked
            .closure()
            .contains(&atom("keep", vec![Value::Number(0), Value::Number(1)]))
    );
    assert!(
        !checked
            .closure()
            .contains(&atom("keep", vec![Value::Number(1), Value::Number(1)]))
    );
    assert!(
        checked
            .closure()
            .contains(&atom("diagonal", vec![Value::Number(1)]))
    );
    assert!(
        !checked
            .closure()
            .contains(&atom("diagonal", vec![Value::Number(0)]))
    );
}

#[test]
fn positive_gate_becomes_ready_only_after_later_join_input() {
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    let program = program(vec![
        fact("d", vec![number(0)]),
        fact("d", vec![number(1)]),
        Template::new(
            Some(pattern("pick", vec![x.clone()])),
            vec![pattern("d", vec![x.clone()])],
            vec![pattern("pick", vec![x.clone()])],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("out", vec![x.clone(), y.clone()])),
            vec![pattern("d", vec![x]), pattern("d", vec![y.clone()])],
            vec![pattern("pick", vec![y])],
            vec![],
            vec![],
        ),
    ]);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).expect("small graph");
    let seed = Seed::new(&program, [atom("pick", vec![Value::Number(1)])])
        .expect("positive candidate gate");
    assert!(assert_seed_matches_static(&program, &graph, &seed));
    let checked =
        check(&program, &seed, Limits::default(), &Control::default()).expect("finite closure");
    assert!(
        checked
            .closure()
            .contains(&atom("out", vec![Value::Number(0), Value::Number(1)]))
    );
    assert!(
        checked
            .closure()
            .contains(&atom("out", vec![Value::Number(1), Value::Number(1)]))
    );
    assert!(
        !checked
            .closure()
            .contains(&atom("out", vec![Value::Number(0), Value::Number(0)]))
    );
}
