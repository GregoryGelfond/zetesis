//! The typed formula boundary shares source semantics without a text roundtrip.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use themelios_base::source::SourceId;
use zetesis_reference_support::{canonical, exhaustive_models, resource_limits};

use zetesis_themelios::logical::{
    program::{Atom, Program, Rule, Script, Statement},
    provenance::Origin,
    raise::raise,
    symbol::{Name, Symbol, VarName},
    term::{Term, Variable},
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, CompilationFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, ParsedSource, ProfileFeature, ProgramAdmissionOptions, ProgramFailureKind,
    ProgramResource, ProgramSubject, admit_program_formula, validate_program_formula,
};

const SOURCE: &str = include_str!("../fixtures/program-admission.lp");

fn raised(text: &str) -> Program {
    let parsed = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    raised.into_program()
}

fn answers(program: &AdmittedFormula) -> BTreeSet<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for model in exhaustive_models(program) {
        assert!(result.insert(model.atoms().iter().map(canonical).collect()));
    }
    result
}

fn fact(value: Term) -> Program {
    Program::of([Rule::fact(Atom::new(Name::new("p").unwrap(), [value]))])
}

fn admitted(program: Arc<Program>) -> AdmittedFormula {
    admit_program_formula(
        program,
        ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

#[test]
fn typed_admission_preserves_source_answer_sets() {
    let typed = admitted(Arc::new(raised(SOURCE)));
    let source = zetesis_reference_support::admit(SOURCE, &FormulaLimits::default()).unwrap();
    let expected: BTreeSet<BTreeSet<String>> = (0..4)
        .map(|mask| {
            let mut atoms = BTreeSet::from(["d(1)".to_owned(), "d(2)".to_owned()]);
            for n in 1..=2 {
                atoms.insert(format!(
                    "{}p({n})",
                    if mask & (1 << (n - 1)) == 0 { "-" } else { "" }
                ));
            }
            if mask & 1 != 0 {
                atoms.insert("q(1)".to_owned());
            }
            atoms
        })
        .collect();
    assert_eq!(expected.len(), 4);
    assert_eq!(answers(&source), expected);
    assert_eq!(answers(&typed), expected);
}

#[test]
fn constructed_symbols_keep_their_types() {
    let name = Name::new("a").unwrap();
    let atom = Atom::new(
        Name::new("p").unwrap(),
        [
            Term::Symbolic(Symbol::String("a".into())),
            Term::Symbolic(Symbol::Function {
                name,
                arguments: vec![],
                sign: zetesis_themelios::logical::symbol::Sign::Positive,
            }),
        ],
    );
    let program = Program::of([Rule::fact(atom)]);
    let admitted = admitted(Arc::new(program));
    assert_eq!(
        answers(&admitted),
        BTreeSet::from([BTreeSet::from(["p(\"a\",a)".to_owned()]),])
    );
}

#[test]
fn typed_evidence_resolves_original_statements() {
    let program = Arc::new(raised(SOURCE));
    let admitted = admitted(Arc::clone(&program));
    assert!(std::ptr::eq(admitted.original_program(), program.as_ref()));
    for site in admitted.formula_origins().iter().flatten() {
        let index = site.statement_id().unwrap().index();
        let evidence = admitted.original_program().statements().nth(index).unwrap();
        assert!(std::ptr::eq(
            program.statements().nth(index).unwrap(),
            evidence
        ));
    }
}

fn coherence_evidence(
    input: &AdmittedFormula,
    roots: &crate::support::root_emission::Roots,
) -> BTreeMap<[zetesis_core::Atom; 2], Vec<zetesis_themelios::ProgramSite>> {
    let mut pairs = BTreeMap::new();
    for index in roots.indices(zetesis_themelios::GroundingPhase::Coherence) {
        let mut atoms = crate::support::root_emission::coherence_atoms(
            input.theory(),
            input.theory().roots()[index],
        )
        .map(|position| {
            input
                .atoms()
                .at(position)
                .unwrap()
                .to_atom(zetesis_core::ValueLimits::default())
                .unwrap()
        });
        atoms.sort();
        assert_ne!(atoms[0], atoms[1]);
        assert!(
            pairs
                .insert(atoms, input.formula_origins()[index].clone())
                .is_none()
        );
    }
    pairs
}

fn opposite_pair(values: Vec<zetesis_core::Value>) -> [zetesis_core::Atom; 2] {
    let positive = zetesis_core::Atom::new(
        zetesis_core::Predicate::new("p", values.len()).unwrap(),
        values,
    )
    .unwrap();
    let mut pair = [positive.clone(), positive.contrary()];
    pair.sort();
    pair
}

#[test]
fn coherence_keeps_exact_ground_atom_occurrences() {
    let program = Arc::new(raised(SOURCE));
    let (typed, typed_roots) = crate::support::root_emission::ground(
        zetesis_themelios::prepare_program_formula(
            Arc::clone(&program),
            ProgramAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap(),
    );
    assert!(std::ptr::eq(typed.original_program(), program.as_ref()));
    let (source, source_roots) = crate::support::root_emission::source(SOURCE);
    let expected = BTreeMap::from([
        (
            opposite_pair(vec![zetesis_core::Value::Number(1)]),
            vec![(3, 28, 52), (2, 53, 74)],
        ),
        (
            opposite_pair(vec![zetesis_core::Value::Number(2)]),
            vec![(3, 28, 52)],
        ),
    ]);
    // The p(2) pair first occurs together in the negative rule, whereas p(1)
    // also names its earlier body occurrence. These are ground-atom sites,
    // not an inventory of all producers or first predicate signatures.
    for (input, roots) in [(&typed, &typed_roots), (&source, &source_roots)] {
        let evidence = coherence_evidence(input, roots);
        let actual = evidence
            .into_iter()
            .map(|(pair, sites)| {
                let sites = sites
                    .into_iter()
                    .map(|site| {
                        let index = site.statement_id().unwrap().index();
                        let original = input.original_program().statements().nth(index).unwrap();
                        let expected_original = program.statements().nth(index).unwrap();
                        assert_eq!(original, expected_original);
                        if input.source().is_none() {
                            assert!(std::ptr::eq(original, expected_original));
                        }
                        let location = site.location().unwrap();
                        assert_eq!(location.source, SourceId::new(0));
                        let start = location.span.start().get();
                        let end = location.span.end().get();
                        let text =
                            &SOURCE[usize::try_from(start).unwrap()..usize::try_from(end).unwrap()];
                        assert_eq!(
                            text,
                            match index {
                                3 => "-p(X) :- d(X), not p(X).",
                                2 => "q(X) :- p(X), X != 2.",
                                _ => panic!("unexpected coherence evidence"),
                            }
                        );
                        (index, start, end)
                    })
                    .collect::<Vec<_>>();
                (pair, sites)
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected);
    }
}

#[test]
fn constructed_coherence_keeps_both_sites_without_coordinates() {
    let positive = Atom::constant(Name::new("p").unwrap());
    let mut negative = positive.clone();
    negative.sign = zetesis_themelios::logical::symbol::Sign::Negative;
    let program = Arc::new(Program::of([Rule::fact(positive), Rule::fact(negative)]));
    let (input, roots) = crate::support::root_emission::ground(
        zetesis_themelios::prepare_program_formula(
            Arc::clone(&program),
            ProgramAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap(),
    );
    assert!(std::ptr::eq(input.original_program(), program.as_ref()));
    assert!(answers(&input).is_empty());
    let mut evidence = coherence_evidence(&input, &roots);
    let sites = evidence.remove(&opposite_pair(vec![])).unwrap();
    assert!(evidence.is_empty());
    assert_eq!(sites.len(), 2);
    assert_eq!(
        sites
            .iter()
            .map(|site| site.statement_id().unwrap().index())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1])
    );
    for site in sites {
        assert_eq!(site.location(), None);
        let index = site.statement_id().unwrap().index();
        let original = input.original_program().statements().nth(index).unwrap();
        assert!(std::ptr::eq(
            original,
            program.statements().nth(index).unwrap()
        ));
        assert_eq!(
            original.provenance().origins().collect::<Vec<_>>(),
            [&Origin::Constructed]
        );
    }
}

#[test]
fn constructed_failures_need_no_source_location() {
    let program = Arc::new(fact(Term::Variable(Variable::Named(
        VarName::new("X").unwrap(),
    ))));
    let error = admit_program_formula(
        Arc::clone(&program),
        ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.cause(),
        FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Evaluation { error: zetesis_themelios::logical::term::EvalError::NotGround { variable: Variable::Named(name) }, .. }) if name.as_str() == "X"
    ));
    let ProgramSubject::Statement(statement) = error.subject().unwrap() else {
        panic!("actual statement");
    };
    assert!(std::ptr::eq(
        statement,
        program.statements().next().unwrap()
    ));
    assert_eq!(
        statement.provenance().origins().collect::<Vec<_>>(),
        [&Origin::Constructed]
    );
    assert!(error.site().unwrap().location().is_none());
    assert!(error.diagnostics().is_empty());
}

#[test]
fn scripts_are_refused_as_statements() {
    let program = Program::of([Statement::Script(Script::new(
        Name::new("python").unwrap(),
        "",
    ))]);
    let error = validate_program_formula(&program, ProgramAdmissionOptions::default()).unwrap_err();
    assert!(matches!(
        error.kind,
        ProgramFailureKind::Compilation(CompilationFailure::Profile(ProfileFeature::Statement))
    ));
    assert!(
        matches!(error.subject, ProgramSubject::Statement(statement) if matches!(statement.get(), Statement::Script(_)))
    );
}

#[test]
fn node_limits_accumulate_across_statements() {
    let program = Program::of([
        Rule::fact(Atom::constant(Name::new("p").unwrap())),
        Rule::fact(Atom::constant(Name::new("q").unwrap())),
    ]);
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::Nodes, 8),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::Nodes)
    );
}

#[test]
fn logical_body_width_has_its_own_ceiling() {
    let program = raised(SOURCE);
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::BodyElements, 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::BodyElements)
    );
    assert!(matches!(error.subject, ProgramSubject::Statement(statement)
        if program.statements().any(|original| std::ptr::eq(original, statement))));
}

#[test]
fn text_limits_accumulate_across_rules() {
    let program = Program::of([
        Rule::fact(Atom::constant(Name::new("first").unwrap())),
        Rule::fact(Atom::constant(Name::new("second").unwrap())),
    ]);
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::TextBytes, 8),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::TextBytes)
    );
}

#[test]
fn named_parts_are_refused_without_a_fake_span() {
    let program = raised("#program step(t). p(t).");
    let error = validate_program_formula(&program, ProgramAdmissionOptions::default()).unwrap_err();
    assert!(matches!(error.subject, ProgramSubject::Part(key) if key.name.as_str() == "step"));
}

#[test]
fn compound_unary_operands_are_bounded_before_evaluation() {
    let argument = (0..32).fold(Term::Symbolic(Symbol::Number(1)), |child, _| {
        Term::BinaryOperation {
            operator: zetesis_themelios::logical::term::BinaryOp::Add,
            left: Box::new(child),
            right: Box::new(Term::Symbolic(Symbol::Number(1))),
        }
    });
    let program = fact(Term::UnaryOperation {
        operator: zetesis_themelios::logical::term::UnaryOp::Negate,
        argument: Box::new(argument),
    });
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::Depth, 12),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::Depth)
    );
}

#[test]
fn symbol_interiors_obey_depth_limits() {
    let symbol = (0..32).fold(Symbol::Number(1), |child, _| Symbol::Tuple(vec![child]));
    let program = fact(Term::Symbolic(symbol));
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::Depth, 12),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::Depth)
    );
}

#[test]
fn strings_obey_logical_text_limits() {
    let program = fact(Term::Symbolic(Symbol::String("x".repeat(32))));
    let error = validate_program_formula(
        &program,
        resource_limits::program(ProgramResource::TextBytes, 16),
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::TextBytes)
    );
}

#[test]
fn admitted_formula_keeps_its_original_owner_alive() {
    let input = {
        let program = Arc::new(fact(Term::Symbolic(Symbol::Number(1))));
        let input = admitted(Arc::clone(&program));
        assert!(std::ptr::eq(input.original_program(), program.as_ref()));
        input
    };
    assert_eq!(
        answers(&input),
        BTreeSet::from([BTreeSet::from(["p(1)".to_owned()])])
    );
    assert_eq!(input.original_program().statements().count(), 1);
}
