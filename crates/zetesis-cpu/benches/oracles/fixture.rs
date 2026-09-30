//! Deterministic static programs and worlds for the oracle measurements.

use zetesis_core::{
    AdmissionLimits, GroundProgram, Program, Seed, SeedSelection, StaticLimits, Template,
};
use zetesis_test_support::programs::nullary_pattern as atom;

/// Families expose both rule-order sensitivity and independent parallel work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Consequences precede their support, requiring repeated dense scans.
    ReverseChain,
    /// Independent gated consequences share a single positive root.
    Wide,
}

/// A compiled static program of one family: eight gates, a root consequence,
/// `consequences - 1` gated rules and one constraint, so batches hold both
/// accepted and constraint-rejected worlds.
pub fn program(family: Family, consequences: usize) -> GroundProgram {
    let gates: Vec<_> = (0..8).map(|index| atom(&format!("g{index}"))).collect();
    let atoms: Vec<_> = (0..consequences)
        .map(|index| atom(&format!("p{index:04}")))
        .collect();
    let mut templates: Vec<_> = gates
        .iter()
        .map(|gate| {
            Template::new(
                Some(gate.clone()),
                vec![],
                vec![gate.clone()],
                vec![],
                vec![],
            )
        })
        .collect();
    templates.push(Template::new(
        Some(atoms[0].clone()),
        vec![],
        vec![],
        vec![],
        vec![],
    ));
    let mut rules: Vec<_> = (1..consequences)
        .map(|index| {
            let (positive, gate) = match family {
                Family::ReverseChain => (atoms[index - 1].clone(), gates[0].clone()),
                Family::Wide => (atoms[0].clone(), gates[index % 8].clone()),
            };
            Template::new(
                Some(atoms[index].clone()),
                vec![positive],
                vec![gate],
                vec![],
                vec![],
            )
        })
        .collect();
    if family == Family::ReverseChain {
        rules.reverse();
    }
    templates.extend(rules);
    templates.push(Template::new(
        None,
        vec![atoms[consequences - 1].clone()],
        vec![gates[7].clone()],
        vec![],
        vec![],
    ));
    let program =
        Program::new(templates, AdmissionLimits::default()).expect("the fixture program is safe");
    GroundProgram::compile(
        &program,
        StaticLimits {
            max_atoms: consequences + 8,
            max_ground_rules: consequences + 9,
            max_substitutions: consequences + 9,
        },
    )
    .expect("the fixture program is within its static limits")
}

/// `count` worlds with shifted gate patterns; `salt` changes their content in
/// place. Counts above 256 repeat patterns, scaling the batch rather than
/// adding distinct candidates.
pub fn seeds(program: &GroundProgram, count: usize, salt: usize) -> Vec<Seed> {
    let gates: Vec<_> = program
        .program()
        .gate_atoms()
        .map(|atom| atom.expect("eight nullary gates"))
        .collect();
    (0..count)
        .map(|world| {
            let mask = world.wrapping_add(salt) & 255;
            SeedSelection::from_carrier_atoms(
                program.program(),
                gates
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, atom)| atom.clone()),
            )
            .expect("fixture gates belong to their program")
            .to_seed()
        })
        .collect()
}
