//! Canonical gate positions index the filtered complete carrier exactly.

use std::sync::Arc;
use zetesis_core::{
    AdmissionLimits, Atom, AtomPattern, GateAtom, GroundProgram, Predicate, Program, SeedError,
    SeedSelection, SeedSelectionError, Sign, StaticLimits, Template, Term, Value, ValueLimits,
    ValueNode,
};

fn program() -> Program {
    let patterns: Vec<_> = [
        ("a", 0, Sign::Positive, false),
        ("b", 1, Sign::Negative, true),
        ("b", 1, Sign::Positive, true),
        ("c", 0, Sign::Negative, false),
        ("d", 2, Sign::Positive, true),
        ("z", 0, Sign::Positive, true),
    ]
    .into_iter()
    .map(|(name, arity, sign, gate)| {
        let pattern = AtomPattern::new(
            Predicate::with_sign(name, arity, sign).unwrap(),
            (0..arity)
                .map(|_| Term::Constant(Value::Number(1)))
                .collect(),
        )
        .unwrap();
        Template::new(
            Some(pattern.clone()),
            vec![],
            if gate { vec![pattern] } else { vec![] },
            vec![],
            vec![],
        )
    })
    .collect();
    let mut rules = patterns;
    for value in [
        Value::String("1".into()),
        Value::Symbol("1".into()),
        Value::Infimum,
        Value::Supremum,
        Value::Number(-1),
        Value::from_nodes(
            vec![ValueNode::Tuple { arity: 1 }, ValueNode::Number(1)],
            ValueLimits::default(),
        )
        .unwrap(),
    ] {
        let p = AtomPattern::new(
            Predicate::new("value", 1).unwrap(),
            vec![Term::Constant(value)],
        )
        .unwrap();
        rules.push(Template::new(Some(p), vec![], vec![], vec![], vec![]));
    }
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn tokens(program: &Program) -> Vec<Arc<GateAtom>> {
    program
        .indexed_gate_atoms()
        .map(|token| Arc::new(token.unwrap()))
        .collect()
}

#[test]
fn full_carrier_matches_independent_atom_storage_order() {
    let program = program();
    let mut expected = Vec::new();
    // Generate the fixture's entire product independently of AtomIter and its
    // odometer. For binary tuples the first coordinate changes fastest, so the
    // subsequent Atom::Ord sort must supply the intended lexicographic order.
    for predicate in program.predicates().iter().rev() {
        match predicate.arity() {
            0 => expected.push(Atom::new(predicate.clone(), vec![]).unwrap()),
            1 => {
                for value in program.domain().iter().rev() {
                    expected.push(Atom::new(predicate.clone(), vec![value.clone()]).unwrap());
                }
            }
            2 => {
                for second in program.domain().iter().rev() {
                    for first in program.domain().iter().rev() {
                        expected.push(
                            Atom::new(predicate.clone(), vec![first.clone(), second.clone()])
                                .unwrap(),
                        );
                    }
                }
            }
            _ => panic!("fixture uses only nullary, unary and binary signatures"),
        }
    }
    expected.extend_from_within(..);
    expected.sort();
    expected.dedup();
    let actual = program
        .carrier_atoms()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn gate_positions_match_typed_interleaved_carrier_order() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let tokens = tokens(&program);
    let gate_atoms: Vec<_> = program.gate_atoms().map(Result::unwrap).collect();
    assert_eq!(tokens.len(), gate_atoms.len());
    assert!(graph.atom_count() > tokens.len());
    let selected = SeedSelection::from_gate_atoms(&program, tokens.iter().cloned()).unwrap();
    for (index, entry) in selected.view().entries().enumerate() {
        assert_eq!(entry.atom(), &gate_atoms[index]);
        let id = entry.resolve_in(&graph).unwrap();
        assert_eq!(id, graph.gate_atom_ids()[index]);
        assert_eq!(graph.atoms()[id as usize], gate_atoms[index]);
    }
}

#[test]
fn sorting_keeps_token_position_and_payload_together() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let tokens = tokens(&program);
    let selected =
        SeedSelection::from_gate_atoms(&program, tokens.iter().rev().chain(&tokens).cloned())
            .unwrap();
    assert_eq!(selected.view().atoms().len(), tokens.len());
    for (entry, token) in selected.view().entries().zip(&tokens) {
        assert!(std::ptr::eq(entry.atom(), token.atom()));
        assert_eq!(
            &graph.atoms()[entry.resolve_in(&graph).unwrap() as usize],
            token.atom()
        );
    }
    let owned = selected.to_seed();
    let mut packed = vec![u32::MAX; graph.word_count()];
    graph.seed_words_into(selected.view(), &mut packed).unwrap();
    assert_eq!(graph.seed_words(&owned).unwrap(), packed);
    drop(tokens);
    assert_eq!(
        graph
            .model_from_words(&packed)
            .unwrap()
            .atoms()
            .iter()
            .collect::<Vec<_>>(),
        owned.atoms().iter().collect::<Vec<_>>()
    );
}

#[test]
fn recompiled_same_program_preserves_positional_meaning() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let other = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selected = SeedSelection::from_gate_atoms(&program, tokens(&program)).unwrap();
    for entry in selected.view().entries() {
        assert_eq!(entry.resolve_in(&graph), entry.resolve_in(&other));
    }
}

#[test]
fn independently_admitted_program_cannot_rebind_tokens() {
    let first = program();
    let second = program();
    assert!(matches!(
        SeedSelection::from_gate_atoms(&second, tokens(&first)),
        Err(SeedSelectionError::WrongProgram)
    ));
    let graph = GroundProgram::compile(&second, StaticLimits::default()).unwrap();
    let selected = SeedSelection::from_gate_atoms(&first, tokens(&first)).unwrap();
    let mut words = vec![u32::MAX; graph.word_count()];
    assert_eq!(
        graph.seed_words_into(selected.view(), &mut words),
        Err(SeedError::WrongProgram)
    );
    assert!(words.iter().all(|word| *word == u32::MAX));
}

#[test]
fn nullary_gate_positions_do_not_require_domain_values() {
    let atom = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let program = Program::new(
        vec![Template::new(
            Some(atom.clone()),
            vec![],
            vec![atom],
            vec![],
            vec![],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    assert!(program.domain().is_empty());
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selected = SeedSelection::from_gate_atoms(&program, tokens(&program)).unwrap();
    assert_eq!(
        selected.view().entries().next().unwrap().resolve_in(&graph),
        Ok(0)
    );
    let empty = Program::new(vec![], AdmissionLimits::default()).unwrap();
    assert!(empty.indexed_gate_atoms().next().is_none());
}
