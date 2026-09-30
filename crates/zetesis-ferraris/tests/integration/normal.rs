//! Independent lazy closure and general subset-minimality agree after lowering.

use proptest::prelude::*;
use zetesis_core::{
    AtomPattern, GroundProgram, Model, Predicate, Program, SeedSelection, StaticLimits, Template,
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionError, AdmissionLimits, Interpretation, from_ground_program,
    from_ground_program_supported,
};

fn pattern(index: usize) -> AtomPattern {
    AtomPattern::new(Predicate::new(["a", "b", "c"][index], 0).unwrap(), vec![]).unwrap()
}

fn patterns(mask: u8) -> Vec<AtomPattern> {
    (0..3)
        .filter(|index| mask & (1 << index) != 0)
        .map(pattern)
        .collect()
}

fn program(rules: &[(u8, u8, u8, u8)]) -> Program {
    let templates = rules
        .iter()
        .map(|&(head, positive, gate_true, gate_false)| {
            Template::new(
                (head < 3).then(|| pattern(usize::from(head))),
                patterns(positive),
                patterns(gate_true),
                patterns(gate_false),
                vec![],
            )
        })
        .collect();
    Program::new(templates, zetesis_core::AdmissionLimits::default()).unwrap()
}

fn compare(program: &Program) {
    let ground = GroundProgram::compile(program, StaticLimits::default()).unwrap();
    let theory = from_ground_program(&ground, AdmissionLimits::default()).unwrap();
    let supported = from_ground_program_supported(&ground, AdmissionLimits::default()).unwrap();
    assert_eq!(ground.atom_count(), theory.atom_count());
    for mask in 0usize..(1 << ground.atom_count()) {
        let model = Model::from_positions(
            ground.atom_catalog(),
            (0..ground.atom_count()).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        let seed = SeedSelection::from_carrier_atoms(
            program,
            model
                .atoms()
                .iter()
                .filter_map(|atom| program.locate_atom(atom, true).unwrap()),
        )
        .unwrap()
        .to_seed();
        let closure = zetesis_cpu::check(
            program,
            &seed,
            zetesis_cpu::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let candidate = Interpretation::new(
            &theory,
            (0..ground.atom_count()).filter(|index| mask & (1 << index) != 0),
        )
        .unwrap();
        let general = zetesis_ferraris::check(
            &theory,
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        let supported_candidate = Interpretation::new(&supported, candidate.atoms()).unwrap();
        let guarded = zetesis_ferraris::check(
            &supported,
            &supported_candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            guarded.accepted(),
            general.accepted(),
            "guarded translation: {program:?}, {model:?}"
        );
        assert_eq!(
            general.accepted(),
            closure.accepted() && closure.closure() == &model,
            "program {program:?}, candidate {model:?}"
        );
    }
}

#[test]
fn choice_support_cycles_constraints_and_unused_atoms_keep_their_semantics() {
    for rules in [
        vec![],
        vec![(0, 0, 1, 0)],               // {a}: two models, unlike a :- a.
        vec![(0, 1, 0, 0)],               // Unsupported positive self-loop.
        vec![(0, 2, 0, 0), (1, 1, 0, 0)], // Unsupported positive cycle.
        vec![(0, 0, 0, 2), (1, 0, 0, 1)], // Default-negation cycle.
        vec![(0, 0, 1, 0), (3, 1, 0, 0)], // Choice constrained false.
        vec![(0, 0, 1, 0), (3, 0, 0, 1)], // Choice constrained true.
        vec![(3, 0, 0, 0)],               // Empty constraint.
        vec![(0, 0, 1, 1)],               // Contradictory gates do not support a.
        vec![(0, 0, 0, 0), (2, 4, 0, 0)], // Unsupported carrier c.
    ] {
        compare(&program(&rules));
    }
}

#[test]
fn exact_formula_dimensions_are_admitted_before_construction() {
    let ground =
        GroundProgram::compile(&program(&[(0, 0, 1, 0)]), StaticLimits::default()).unwrap();
    let full = from_ground_program(&ground, AdmissionLimits::default()).unwrap();
    let exact = AdmissionLimits {
        max_atoms: full.atom_count(),
        max_nodes: full.nodes().len(),
        max_roots: full.roots().len(),
    };
    assert!(from_ground_program(&ground, exact).is_ok());
    for limits in [
        AdmissionLimits {
            max_atoms: exact.max_atoms - 1,
            ..exact
        },
        AdmissionLimits {
            max_nodes: exact.max_nodes - 1,
            ..exact
        },
        AdmissionLimits {
            max_roots: exact.max_roots - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            from_ground_program(&ground, limits),
            Err(AdmissionError::Limit)
        ));
    }
}

#[test]
fn supported_guards_obey_exact_admission_limits() {
    let ground = GroundProgram::compile(
        &program(&[(0, 0, 1, 0), (0, 2, 0, 0), (2, 4, 0, 0)]),
        StaticLimits::default(),
    )
    .unwrap();
    let full = from_ground_program_supported(&ground, AdmissionLimits::default()).unwrap();
    let exact = AdmissionLimits {
        max_atoms: full.atom_count(),
        max_nodes: full.nodes().len(),
        max_roots: full.roots().len(),
    };
    assert!(from_ground_program_supported(&ground, exact).is_ok());
    assert!(matches!(
        from_ground_program_supported(
            &ground,
            AdmissionLimits {
                max_nodes: exact.max_nodes - 1,
                ..exact
            }
        ),
        Err(AdmissionError::Limit)
    ));
    assert!(matches!(
        from_ground_program_supported(
            &ground,
            AdmissionLimits {
                max_roots: exact.max_roots - 1,
                ..exact
            }
        ),
        Err(AdmissionError::Limit)
    ));
}

#[test]
fn unsupported_classical_atoms_are_pruned_before_countermodel_search() {
    let graph = GroundProgram::compile(&program(&[(0, 0, 0, 2)]), StaticLimits::default()).unwrap();
    let plain = from_ground_program(&graph, AdmissionLimits::default()).unwrap();
    let supported = from_ground_program_supported(&graph, AdmissionLimits::default()).unwrap();
    let unused = graph
        .atoms()
        .iter()
        .position(|atom| atom.predicate().name() == "b")
        .unwrap();
    for (theory, expected) in [(&plain, true), (&supported, false)] {
        let candidate = Interpretation::new(theory, [unused]).unwrap();
        assert_eq!(
            zetesis_ferraris::models(
                theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default()
            )
            .unwrap(),
            expected
        );
    }
}

proptest! {
    #[test]
    fn generated_three_atom_normal_programs_preserve_stability(
        rules in prop::collection::vec((0u8..4, 0u8..8, 0u8..8, 0u8..8), 0..12)
    ) {
        compare(&program(&rules));
    }
}
