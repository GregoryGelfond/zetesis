use super::*;
use crate::{AdmissionLimits, AtomPattern, Predicate, StaticLimits, Template};

fn program() -> Program {
    let rules = ["a", "b"].map(|name| {
        let atom = AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        Template::new(Some(atom.clone()), vec![], vec![atom], vec![], vec![])
    });
    Program::new(rules.into(), AdmissionLimits::default()).unwrap()
}

#[test]
fn indexed_resolution_performs_no_symbolic_lookup() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let tokens: Vec<_> = program
        .indexed_gate_atoms()
        .map(|token| Arc::new(token.unwrap()))
        .collect();
    for _ in 0..3 {
        let selection =
            SeedSelection::from_gate_atoms(&program, tokens.iter().rev().cloned()).unwrap();
        for (entry, expected) in selection.view().entries().zip([0, 1]) {
            assert_eq!(
                entry.resolve_with(&graph, |_| panic!("indexed entry must not search atoms")),
                Ok(expected)
            );
        }
    }
}

#[test]
fn manual_resolution_uses_one_symbolic_lookup() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let selection = SeedSelection::new(&program, [Arc::new(graph.atoms()[1].clone())]).unwrap();
    let entry = selection.view().entries().next().unwrap();
    let mut lookups = 0;
    let resolved = entry.resolve_with(&graph, |atom| {
        lookups += 1;
        graph.atom_id(atom)
    });
    assert_eq!(resolved, Ok(1));
    assert_eq!(lookups, 1);
}

#[test]
fn missing_indexed_position_never_falls_back() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let entry = SeedAtom {
        program: &program,
        atom: &graph.atoms()[0],
        position: NonZeroUsize::new(usize::MAX),
    };
    assert_eq!(
        entry.resolve_with(&graph, |_| panic!(
            "bad indexed entry must not use symbolic fallback"
        )),
        Err(SeedError::InvalidGatePosition)
    );
}
