//! The typed relational boundary shares source semantics without a text roundtrip.

use std::collections::BTreeSet;

use zetesis_core::Program as NativeProgram;
use zetesis_cpu::{Cancellation, CandidateLimits, CandidateTermination, Candidates, Limits, check};
use zetesis_reference_support::canonical;
use zetesis_themelios::logical::{
    program::{Atom, Program, Rule, Script, Statement},
    provenance::Origin,
    raise::raise,
    symbol::{Name, Symbol, VarName},
    term::{Term, Variable},
};
use zetesis_themelios::{
    AdmissionOptions, CompilationFailure, ParsedSource, ProfileFeature, ProgramAdmissionOptions,
    ProgramFailureKind, ProgramResource, ProgramSubject, admit, admit_program,
};

const SOURCE: &str = include_str!("../fixtures/program-admission.lp");

fn raised(text: &str) -> Program {
    let parsed = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    raised.into_program()
}

fn answers(program: &NativeProgram) -> BTreeSet<BTreeSet<String>> {
    let cancellation = Cancellation::default();
    let mut candidates = Candidates::new(program, CandidateLimits::default(), cancellation.clone());
    let mut result = BTreeSet::new();
    for seed in candidates.by_ref() {
        let membership = check(program, &seed.unwrap(), Limits::default(), &cancellation).unwrap();
        if membership.accepted() {
            assert!(result.insert(membership.closure().atoms().iter().map(canonical).collect()));
        }
    }
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    result
}

fn fact(value: Term) -> Program {
    Program::of([Rule::fact(Atom::new(Name::new("p").unwrap(), [value]))])
}

#[test]
fn typed_admission_preserves_source_answer_sets() {
    let program = raised(SOURCE);
    let typed = admit_program(&program, ProgramAdmissionOptions::default()).unwrap();
    let source = admit(SOURCE.into(), AdmissionOptions::default()).unwrap();
    let expected = answers(source.program());
    assert_eq!(expected.len(), 4);
    assert_eq!(answers(typed.program()), expected);
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
    let admitted = admit_program(&program, ProgramAdmissionOptions::default()).unwrap();
    assert_eq!(
        answers(admitted.program()),
        BTreeSet::from([BTreeSet::from(["p(\"a\",a)".to_owned()]),])
    );
}

#[test]
fn typed_evidence_borrows_the_original_statements() {
    let program = raised(SOURCE);
    let admitted = admit_program(&program, ProgramAdmissionOptions::default()).unwrap();
    for evidence in admitted.template_statements() {
        for &statement in evidence {
            assert!(
                program
                    .statements()
                    .any(|original| std::ptr::eq(original, statement))
            );
        }
    }
}

#[test]
fn coherence_constraints_keep_both_logical_origins() {
    let program = raised(SOURCE);
    let admitted = admit_program(&program, ProgramAdmissionOptions::default()).unwrap();
    let evidence = admitted.template_statements().last().unwrap();
    assert_eq!(evidence.len(), 2);
    assert_ne!(evidence[0], evidence[1]);
    assert_eq!(
        admitted.program().templates().len(),
        program.statements().count() + 1
    );
}

#[test]
fn constructed_failures_need_no_source_location() {
    let program = fact(Term::Variable(Variable::Named(VarName::new("X").unwrap())));
    let error = admit_program(&program, ProgramAdmissionOptions::default()).unwrap_err();
    assert!(matches!(
        error.kind,
        ProgramFailureKind::Core(zetesis_core::AdmissionError::UnsafeVariable { .. })
    ));
    let ProgramSubject::Statement(statement) = error.subject else {
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
}

#[test]
fn scripts_are_refused_as_statements() {
    let program = Program::of([Statement::Script(Script::new(
        Name::new("python").unwrap(),
        "",
    ))]);
    let error = admit_program(&program, ProgramAdmissionOptions::default()).unwrap_err();
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
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_nodes: 8,
            ..ProgramAdmissionOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::Nodes)
    );
}

#[test]
fn logical_body_width_has_its_own_ceiling() {
    let program = raised(SOURCE);
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_body_elements: 1,
            ..ProgramAdmissionOptions::default()
        },
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
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_text_bytes: 8,
            ..ProgramAdmissionOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::TextBytes)
    );
}

#[test]
fn named_parts_are_refused_without_a_fake_span() {
    let program = raised("#program step(t). p(t).");
    let error = admit_program(&program, ProgramAdmissionOptions::default()).unwrap_err();
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
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_depth: 12,
            ..ProgramAdmissionOptions::default()
        },
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
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_depth: 12,
            ..ProgramAdmissionOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::Depth)
    );
}

#[test]
fn strings_obey_logical_text_limits() {
    let program = fact(Term::Symbolic(Symbol::String("x".repeat(32))));
    let error = admit_program(
        &program,
        ProgramAdmissionOptions {
            max_text_bytes: 16,
            ..ProgramAdmissionOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error.kind, ProgramFailureKind::Limit(limit) if limit.resource == ProgramResource::TextBytes)
    );
}

#[test]
fn coherence_respects_native_template_limits() {
    let program = raised(SOURCE);
    let mut options = ProgramAdmissionOptions::default();
    options.core_limits.max_templates = program.statements().count();
    let error = admit_program(&program, options).unwrap_err();
    assert!(matches!(
        error.kind,
        ProgramFailureKind::Core(zetesis_core::AdmissionError::LimitExceeded {
            resource: zetesis_core::AdmissionResource::Templates,
            ..
        })
    ));
}

#[test]
fn the_native_program_outlives_borrowed_evidence() {
    let native = {
        let program = fact(Term::Symbolic(Symbol::Number(1)));
        admit_program(&program, ProgramAdmissionOptions::default())
            .unwrap()
            .into_program()
    };
    assert_eq!(
        answers(&native),
        BTreeSet::from([BTreeSet::from(["p(1)".to_owned()])])
    );
}
