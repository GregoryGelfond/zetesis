//! Admission, sparse identity, carrier coverage, and eager representation laws.

use zetesis_core::{
    AdmissionError, AdmissionLimits, AdmissionResource, Atom, AtomPattern, ConstructionError,
    Filter, GroundProgram, Model, Predicate, Program, Seed, SeedError, StaticError, StaticLimits,
    Template, Term, Value, WordError,
};
use zetesis_test_support::programs::{atom, number, pattern, program};

fn predicate(name: &str, arity: usize) -> Predicate {
    Predicate::new(name, arity).expect("nonempty test predicate")
}
fn fact(name: &str, value: i32) -> Template {
    Template::new(
        Some(pattern(name, vec![number(value)])),
        vec![],
        vec![],
        vec![],
        vec![],
    )
}

#[test]
fn values_have_typed_canonical_identity() {
    let values = [
        Value::Number(-1),
        Value::Number(1),
        Value::String("1".into()),
        Value::Symbol("1".into()),
    ];
    for pair in values.windows(2) {
        assert!(pair[0] < pair[1]);
    }
    assert!(
        !Filter::Eq(number(1), Term::Constant(Value::String("1".into())))
            .evaluate(&[])
            .expect("closed terms")
    );
    assert!(
        Filter::Neq(number(1), Term::Constant(Value::String("1".into())))
            .evaluate(&[])
            .expect("closed terms")
    );
}

#[test]
fn extrema_surround_finite_terms_without_coercing_printed_spellings() {
    let values = [
        Value::Infimum,
        Value::Number(i32::MIN),
        Value::Number(i32::MAX),
        Value::Symbol("a".into()),
        Value::String("#inf".into()),
        Value::String("#sup".into()),
        Value::Supremum,
    ];
    for (left, a) in values.iter().enumerate() {
        for (right, b) in values.iter().enumerate() {
            assert_eq!(a.compare_terms(b), left.cmp(&right));
            assert_eq!(a == b, left == right);
        }
    }
    let model = Model::new(values.iter().cloned().map(|value| atom("v", vec![value]))).unwrap();
    assert_eq!(model.atoms().len(), values.len());
    let templates = values
        .iter()
        .cloned()
        .map(|value| {
            Template::new(
                Some(pattern("v", vec![Term::Constant(value)])),
                vec![],
                vec![],
                vec![],
                vec![],
            )
        })
        .collect();
    let source = program(templates);
    assert_eq!(source.domain().len(), values.len());
    assert_eq!(
        source.domain().iter().next(),
        Some((&Value::Infimum).into())
    );
    assert_eq!(
        source.domain().iter().next_back(),
        Some((&Value::Supremum).into())
    );
}

#[test]
fn signatures_and_patterns_refuse_arity_mismatches() {
    assert_eq!(
        Predicate::new("", 0),
        Err(ConstructionError::EmptyPredicateName)
    );
    assert!(matches!(
        Atom::new(predicate("a", 2), vec![Value::Number(0)]),
        Err(ConstructionError::ArityMismatch {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        AtomPattern::new(predicate("a", 1), vec![]),
        Err(ConstructionError::ArityMismatch {
            expected: 1,
            actual: 0
        })
    ));
}

#[test]
fn gates_and_filters_never_establish_variable_safety() {
    let unsafe_gate = Template::new(
        None,
        vec![],
        vec![pattern("a", vec![Term::Variable(0)])],
        vec![],
        vec![],
    );
    let error = Program::new(vec![unsafe_gate], AdmissionLimits::default())
        .expect_err("gate is not positive support");
    assert_eq!(error.template_index(), Some(0));
    assert!(matches!(
        error,
        AdmissionError::UnsafeVariable { variable: 0, .. }
    ));
    let unsafe_filter = Template::new(
        None,
        vec![],
        vec![],
        vec![],
        vec![Filter::Eq(Term::Variable(0), number(0))],
    );
    assert!(matches!(
        Program::new(vec![unsafe_filter], AdmissionLimits::default()),
        Err(AdmissionError::UnsafeVariable { .. })
    ));
}

#[test]
fn sparse_variable_ids_and_limits_are_typed_refusals() {
    let sparse = Template::new(
        None,
        vec![pattern("a", vec![Term::Variable(usize::MAX)])],
        vec![],
        vec![],
        vec![],
    );
    assert!(matches!(
        Program::new(vec![sparse], AdmissionLimits::default()),
        Err(AdmissionError::NonDenseVariable {
            expected: 0,
            actual: usize::MAX,
            ..
        })
    ));
    let limits = AdmissionLimits {
        max_templates: 0,
        ..AdmissionLimits::default()
    };
    assert!(matches!(
        Program::new(vec![fact("a", 0)], limits),
        Err(AdmissionError::LimitExceeded {
            resource: AdmissionResource::Templates,
            template: None,
            ..
        })
    ));
    let limits = AdmissionLimits {
        max_predicate_arity: 0,
        ..AdmissionLimits::default()
    };
    assert!(matches!(
        Program::new(vec![fact("a", 0)], limits),
        Err(AdmissionError::LimitExceeded {
            resource: AdmissionResource::PredicateArity,
            template: Some(0),
            ..
        })
    ));
}

#[test]
fn program_preserves_templates_and_scans_filter_only_constants() {
    let template = Template::new(
        None,
        vec![],
        vec![],
        vec![],
        vec![Filter::Eq(number(7), number(9))],
    );
    let program = program(vec![template.clone(), template.clone()]);
    assert_eq!(program.templates().len(), 2);
    for admitted in program.templates() {
        assert_eq!(admitted, template);
    }
    assert_eq!(program.domain().len(), 2);
    assert_eq!(program.domain().at(0).unwrap(), Value::Number(7));
    assert_eq!(program.domain().at(1).unwrap(), Value::Number(9));
    assert!(program.predicates().is_empty());
}

#[test]
fn empty_seed_does_not_expand_a_large_symbolic_carrier() {
    let wide = pattern("gate", vec![number(0); 32]);
    let program = program(vec![
        fact("d", 0),
        fact("d", 1),
        Template::new(None, vec![], vec![], vec![wide], vec![]),
    ]);
    let seed = Seed::new(&program, []).expect("empty seed is always carrier-valid");
    assert!(seed.atoms().is_empty());
    let first = program
        .gate_atoms()
        .next()
        .expect("nonempty carrier")
        .expect("one tuple fits");
    assert!(
        first.atom().values().iter().eq(vec![Value::Number(0); 32]
            .iter()
            .map(zetesis_core::catalog::TermRef::from))
    );
    assert!(!seed.contains(&first));
    assert!(matches!(
        GroundProgram::compile(&program, StaticLimits::default()),
        Err(StaticError::LimitExceeded {
            resource: "atoms",
            ..
        })
    ));
}

#[test]
fn carrier_iteration_and_cloned_cursors_are_exact() {
    for size in 1..=5 {
        let mut templates: Vec<_> = (0..size).map(|i| fact("d", i)).collect();
        templates.push(Template::new(
            None,
            vec![],
            vec![],
            vec![pattern("gate", vec![number(0), number(0)])],
            vec![],
        ));
        let program = program(templates);
        let mut cursor = program.gate_atoms();
        let first = cursor.next().expect("one tuple").expect("tuple allocation");
        let rest: Vec<_> = cursor
            .clone()
            .collect::<Result<_, _>>()
            .expect("tuple allocation");
        let resumed: Vec<_> = cursor.collect::<Result<_, _>>().expect("tuple allocation");
        assert_eq!(rest, resumed);
        let mut all = vec![first];
        all.extend(rest);
        let expected: Vec<_> = (0..size)
            .flat_map(|x| {
                (0..size).map(move |y| atom("gate", vec![Value::Number(x), Value::Number(y)]))
            })
            .collect();
        assert!(
            all.iter()
                .map(zetesis_core::CarrierAtom::atom)
                .eq(expected.iter().map(zetesis_core::catalog::AtomRef::from))
        );
    }
}

#[test]
fn empty_domain_has_nullary_atoms_but_no_positive_arity_tuples() {
    let nullary = pattern("a", vec![]);
    let only_variables = pattern("p", vec![Term::Variable(0)]);
    let program = program(vec![
        Template::new(Some(nullary.clone()), vec![], vec![nullary], vec![], vec![]),
        Template::new(
            None,
            vec![only_variables.clone()],
            vec![only_variables],
            vec![],
            vec![],
        ),
    ]);
    assert!(program.domain().is_empty());
    let gates = program
        .gate_atoms()
        .collect::<Result<Vec<_>, _>>()
        .expect("empty tuple");
    assert!(
        gates
            .iter()
            .map(zetesis_core::CarrierAtom::atom)
            .eq([atom("a", vec![])]
                .iter()
                .map(zetesis_core::catalog::AtomRef::from))
    );
    let graph =
        GroundProgram::compile(&program, StaticLimits::default()).expect("one nullary graph");
    assert_eq!(graph.atom_count(), 1);
    assert_eq!(graph.rules().len(), 1);
}

#[test]
fn static_filters_and_ground_duplicate_antecedents_are_exact() {
    let x = Term::Variable(0);
    let y = Term::Variable(1);
    let template = Template::new(
        Some(pattern("out", vec![x.clone(), y.clone()])),
        vec![
            pattern("d", vec![x.clone()]),
            pattern("d", vec![y.clone()]),
            pattern("d", vec![x.clone()]),
        ],
        vec![pattern("g", vec![x.clone()]), pattern("g", vec![y.clone()])],
        vec![],
        vec![Filter::Eq(x, y)],
    );
    let program = program(vec![fact("d", 0), fact("d", 1), template]);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).expect("bounded graph");
    assert_eq!(graph.atom_count(), 8);
    assert_eq!(graph.rules().len(), 4);
    for rule in &graph.rules()[2..] {
        assert_eq!(
            rule.positive().len(),
            1,
            "different variables alias after substitution"
        );
        assert_eq!(rule.gate_true().len(), 1);
        let head = graph
            .atoms()
            .at(rule.head().expect("headed") as usize)
            .unwrap();
        assert_eq!(head.values().at(0), head.values().at(1));
    }
}

#[test]
fn static_graph_keeps_gate_carrier_even_for_filtered_out_rules() {
    let template = Template::new(
        None,
        vec![],
        vec![],
        vec![pattern("g", vec![number(0)])],
        vec![Filter::Eq(number(0), number(1))],
    );
    let program = program(vec![template]);
    let graph =
        GroundProgram::compile(&program, StaticLimits::default()).expect("two carrier tuples");
    assert!(graph.rules().is_empty());
    assert_eq!(graph.gate_atom_ids().len(), 2);
    let unused = atom("g", vec![Value::Number(1)]);
    let seed = Seed::new(&program, [unused.clone()]).expect("symbolic carrier includes tuple");
    let words = graph
        .seed_words(&seed)
        .expect("all symbolic tuples have dense IDs");
    assert!(
        graph
            .model_from_words(&words)
            .expect("valid words")
            .contains(&unused)
    );
}

#[test]
fn seeds_bind_instance_identity_and_reject_foreign_keys() {
    let gate = pattern("a", vec![]);
    let template = Template::new(Some(gate.clone()), vec![], vec![gate], vec![], vec![]);
    let first = program(vec![template.clone()]);
    let second = program(vec![template]);
    assert!(first.same_instance(&first.clone()));
    assert!(!first.same_instance(&second));
    let seed = Seed::new(&first, [atom("a", vec![])]).expect("matching carrier");
    let graph = GroundProgram::compile(&second, StaticLimits::default()).expect("small graph");
    assert_eq!(graph.seed_words(&seed), Err(SeedError::WrongProgram));
    assert!(matches!(
        Seed::new(&first, [atom("b", vec![])]),
        Err(SeedError::OutsideCarrier { .. })
    ));
}

#[test]
fn word_conversion_crosses_the_old_sixty_four_atom_limit() {
    let templates: Vec<_> = (0..70)
        .map(|index| {
            let p = pattern(&format!("a{index:02}"), vec![]);
            Template::new(Some(p.clone()), vec![], vec![p], vec![], vec![])
        })
        .collect();
    let program = program(templates);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).expect("seventy atoms");
    let chosen = [0usize, 31, 32, 63, 64, 69].map(|index| atom(&format!("a{index:02}"), vec![]));
    let seed = Seed::new(&program, chosen.clone()).expect("gate atoms");
    let words = graph.seed_words(&seed).expect("three words");
    assert_eq!(words.len(), 3);
    assert_eq!(
        graph.model_from_words(&words).expect("exact decode"),
        Model::new(chosen).unwrap()
    );
    assert_eq!(
        graph.model_from_words(&[0, 0, 1 << 6]),
        Err(WordError::TailBits)
    );
    assert!(matches!(
        graph.model_from_words(&[]),
        Err(WordError::Length {
            expected: 3,
            actual: 0
        })
    ));
}

#[test]
fn empty_graph_word_representation_is_empty() {
    let program = program(vec![]);
    let graph = GroundProgram::compile(&program, StaticLimits::default()).expect("empty graph");
    assert_eq!(graph.word_count(), 0);
    assert_eq!(graph.model_from_words(&[]), Ok(Model::default()));
    assert!(matches!(
        graph.model_from_words(&[0]),
        Err(WordError::Length { .. })
    ));
}

#[test]
fn static_work_limits_and_cartesian_overflow_do_not_become_partial_graphs() {
    let program = program(vec![fact("a", 0)]);
    let limits = StaticLimits {
        max_substitutions: 0,
        ..StaticLimits::default()
    };
    assert!(matches!(
        GroundProgram::compile(&program, limits),
        Err(StaticError::LimitExceeded {
            resource: "substitutions",
            ..
        })
    ));
    let limits = StaticLimits {
        max_ground_rules: 0,
        ..StaticLimits::default()
    };
    assert!(matches!(
        GroundProgram::compile(&program, limits),
        Err(StaticError::LimitExceeded {
            resource: "ground rules",
            ..
        })
    ));
    let arity = usize::BITS as usize;
    let limits = AdmissionLimits {
        max_predicate_arity: arity,
        ..AdmissionLimits::default()
    };
    let huge = Program::new(
        vec![
            fact("d", 0),
            fact("d", 1),
            Template::new(
                None,
                vec![],
                vec![],
                vec![pattern("g", vec![number(0); arity])],
                vec![],
            ),
        ],
        limits,
    )
    .expect("symbolically finite");
    assert!(matches!(
        GroundProgram::compile(
            &huge,
            StaticLimits {
                max_atoms: usize::MAX,
                ..StaticLimits::default()
            }
        ),
        Err(StaticError::CountOverflow)
    ));
}

#[test]
fn missing_assignments_are_reported_by_variable_id() {
    let p = pattern("a", vec![Term::Variable(2)]);
    assert_eq!(
        p.instantiate(&[Value::Number(0)])
            .expect_err("missing variable")
            .variable,
        2
    );
    assert_eq!(
        Filter::Neq(number(0), Term::Variable(3))
            .evaluate(&[])
            .expect_err("missing variable")
            .variable,
        3
    );
}
