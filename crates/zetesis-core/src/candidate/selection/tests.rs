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
    let atom = Atom::new(Predicate::new("b", 0).unwrap(), vec![]).unwrap();
    let selection = SeedSelection::new(&program, [Arc::new(atom)]).unwrap();
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
    let atom = Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap();
    let carrier = program.locate_atom(&atom, true).unwrap().unwrap();
    let entry = SeedAtom {
        program: &program,
        carrier: &carrier,
        position: NonZeroUsize::new(usize::MAX),
    };
    assert_eq!(
        entry.resolve_with(&graph, |_| panic!(
            "bad indexed entry must not use symbolic fallback"
        )),
        Err(SeedError::InvalidGatePosition)
    );
}

#[test]
fn retained_selection_shares_the_indexed_witness() {
    let program = program();
    let gate = Arc::new(program.indexed_gate_atoms().next().unwrap().unwrap());
    let selection = SeedSelection::from_gate_atoms(&program, [gate.clone()]).unwrap();
    let seed = selection.to_seed();
    assert!(Arc::ptr_eq(&seed.selection.0, &selection.0));
    let Entry::Indexed(stored) = &selection.0.atoms[0] else {
        panic!("indexed witness retained");
    };
    assert!(Arc::ptr_eq(stored, &gate));
}

#[test]
fn duplicate_manual_entry_keeps_the_indexed_witness() {
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let gate = Arc::new(program.indexed_gate_atoms().next().unwrap().unwrap());
    let selected = SeedSelection::held_and_selected(&program, [gate.carrier()], [gate]).unwrap();
    assert_eq!(selected.view().atoms().len(), 1);
    let entry = selected.view().entries().next().unwrap();
    assert_eq!(
        entry.resolve_with(&graph, |_| panic!("duplicate preserves indexed lookup")),
        Ok(0)
    );
}

#[test]
fn accepted_shared_ingress_is_not_retained() {
    let program = program();
    let atom = Arc::new(Atom::new(Predicate::new("a", 0).unwrap(), vec![]).unwrap());
    let caller = Arc::downgrade(&atom);
    let selected = SeedSelection::new(&program, [atom]).unwrap();
    assert!(caller.upgrade().is_none());
    assert_eq!(
        selected.view().atoms().next().unwrap().predicate().name(),
        "a"
    );
}

#[test]
fn full_carrier_nongate_token_is_refused() {
    let pattern = AtomPattern::new(Predicate::new("fact", 0).unwrap(), vec![]).unwrap();
    let program = Program::new(
        vec![Template::new(Some(pattern), vec![], vec![], vec![], vec![])],
        AdmissionLimits::default(),
    )
    .unwrap();
    let atom = program.carrier_atoms().next().unwrap().unwrap();
    assert!(matches!(
        SeedSelection::from_carrier_atoms(&program, [atom]),
        Err(SeedSelectionError::OutsideGateCarrier { .. })
    ));
}
