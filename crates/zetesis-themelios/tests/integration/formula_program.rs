//! Canonical programs share formula semantics, ownership and refusal boundaries.

use std::{collections::BTreeSet, sync::Arc};

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_reference_support::{canonical, exhaustive, formula};
use zetesis_test_support::records::Records;
use zetesis_themelios::logical::symbol::VarName;
use zetesis_themelios::logical::{
    Name, Sign, Symbol, Term,
    program::{
        Atom, Choice, ChoiceElement, Condition, Disjunction, DisjunctionElement, Guard, Head,
        LiteralInner, Program, Relation, Rule, Statement,
    },
    raise::raise,
};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ConstraintCheckCause, ConstraintCheckLimits,
    ConstraintVerdict, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaMaterialization, ParsedSource, PreparedFormula, ProgramAdmissionOptions, ProgramSubject,
    ReconstructionError, prepare_program_formula,
};

const SOURCE: &str = include_str!("../fixtures/formula-program.lp");

fn atom(name: &str) -> Atom {
    Atom::constant(Name::new(name).unwrap())
}

fn prepare(program: Arc<Program>) -> PreparedFormula {
    prepare_program_formula(
        program,
        ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn bounded_choice() -> Rule {
    let bound = || Guard {
        relation: Some(Relation::Le),
        term: 1.into(),
    };
    Rule::fact(Choice::new(
        Some(bound()),
        ["p", "q"].map(|name| ChoiceElement::new(atom(name).into(), Condition::default())),
        Some(bound()),
    ))
}

fn expected_choices() -> Records {
    ["p", "q"]
        .map(|name| (BTreeSet::from([name.to_owned()]), None))
        .into()
}

#[test]
fn constructed_bounded_choice_has_exactly_two_answers() {
    let program = Arc::new(Program::of([bounded_choice()]));
    let prepared = prepare(Arc::clone(&program));
    assert!(prepared.source().is_none());
    assert!(std::ptr::eq(prepared.original_program(), program.as_ref()));
    let admitted = prepared.ground().unwrap();
    assert!(admitted.source().is_none());
    assert!(std::ptr::eq(admitted.original_program(), program.as_ref()));
    assert_eq!(exhaustive(&admitted), expected_choices());
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .all(|site| { site.statement_id().is_some() && site.location().is_none() })
    );
}

#[test]
fn constructed_disjunction_preserves_minimality() {
    let head = Disjunction::new(
        ["p", "q"].map(|name| DisjunctionElement::new(atom(name).into(), Condition::default())),
    );
    let program = Arc::new(Program::of([Rule::fact(head)]));
    assert_eq!(
        exhaustive(&prepare(program).ground().unwrap()),
        expected_choices()
    );
}

fn observations(owner: &AdmittedFormula, names: &BTreeSet<String>) -> Vec<Symbol> {
    let positions = owner
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(position, atom)| names.contains(&canonical(atom)).then_some(position));
    let model = Model::from_positions(owner.atom_catalog(), positions).unwrap();
    owner
        .metadata()
        .observations()
        .evaluate(
            &model,
            zetesis_themelios::observation::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .symbols()
        .to_vec()
}

#[test]
fn raised_formula_preserves_source_semantics() {
    let parsed = ParsedSource::new(SOURCE.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    let program = Arc::new(raised.into_program());
    let typed = prepare(Arc::clone(&program)).ground().unwrap();
    let source = formula(SOURCE);
    assert!(std::ptr::eq(typed.original_program(), program.as_ref()));
    assert_eq!(typed.atoms(), source.atoms());
    assert_eq!(typed.theory().nodes(), source.theory().nodes());
    assert_eq!(typed.theory().roots(), source.theory().roots());
    assert_eq!(typed.metadata().output(), source.metadata().output());
    let expected = exhaustive(&source);
    assert_eq!(expected.len(), 2);
    assert_eq!(exhaustive(&typed), expected);
    let mut shown = BTreeSet::new();
    for (names, costs) in &expected {
        assert!(costs.is_some());
        let actual = observations(&typed, names);
        assert_eq!(actual, observations(&source, names));
        shown.insert(actual);
    }
    let expected_shown = [1, 2].map(|value| {
        vec![Symbol::Function {
            name: Name::new("chosen").unwrap(),
            arguments: vec![Symbol::Number(value)],
            sign: Sign::Positive,
        }]
    });
    assert_eq!(shown, BTreeSet::from(expected_shown));
}

#[test]
fn constructed_hybrid_retains_original_constraint_identity() {
    let program = Arc::new(Program::of([bounded_choice(), Rule::constraint(atom("q"))]));
    let owner = prepare(Arc::clone(&program)).ground_hybrid().unwrap();
    assert!(owner.source().is_none());
    assert!(std::ptr::eq(owner.original_program(), program.as_ref()));
    assert_eq!(owner.streamed_templates(), 1);
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let mut accepted = BTreeSet::new();
    for name in ["p", "q"] {
        let selected: Vec<_> = owner
            .atom_catalog()
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(position, atom)| (atom.predicate().name() == name).then_some(position))
            .collect();
        assert_eq!(selected.len(), 1);
        let candidate = Interpretation::new(owner.core_theory(), selected.iter().copied()).unwrap();
        assert!(
            check(
                owner.core_theory(),
                &candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .accepted()
        );
        let model = Model::from_positions(owner.atom_catalog(), selected).unwrap();
        match checker.check(&model, &Cancellation::default()).unwrap() {
            ConstraintVerdict::Satisfied => {
                accepted.insert(name);
            }
            ConstraintVerdict::Violated { site } => {
                assert!(site.location().is_none());
                let statement = program
                    .statements()
                    .nth(site.statement_id().unwrap().index())
                    .unwrap();
                assert!(
                    matches!(statement.get(), Statement::Rule(rule) if matches!(rule.head().get(), Head::Falsum))
                );
            }
        }
    }
    assert_eq!(accepted, BTreeSet::from(["p"]));
}

fn terminal_program() -> Arc<Program> {
    let x = Term::variable(VarName::new("X").unwrap());
    let seed = |value| Atom::new(Name::new("seed").unwrap(), [value]);
    let receipt = Atom::new(Name::new("receipt").unwrap(), [x.clone()]);
    Arc::new(Program::of([
        Rule::fact(seed(Term::from(1))),
        Rule::new(receipt, seed(x)),
    ]))
}

#[test]
fn constructed_adaptive_receipt_reconstructs_the_original_answer() {
    let program = terminal_program();
    let FormulaMaterialization::Terminal(owner) =
        prepare(Arc::clone(&program)).ground_adaptive().unwrap()
    else {
        panic!("the variable terminal definition is deferred");
    };
    assert!(owner.source().is_none());
    assert!(std::ptr::eq(owner.original_program(), program.as_ref()));
    let positions = 0..owner.base_atom_catalog().atoms().len();
    let candidate = Interpretation::new(owner.base_theory(), positions.clone()).unwrap();
    assert!(
        check(
            owner.base_theory(),
            &candidate,
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .accepted()
    );
    let base = Model::from_positions(owner.base_atom_catalog(), positions).unwrap();
    let full = owner
        .reconstruction()
        .unwrap()
        .reconstruct(&base, &Cancellation::default())
        .unwrap();
    assert_eq!(
        full.atoms().iter().map(canonical).collect::<BTreeSet<_>>(),
        BTreeSet::from(["receipt(1)".to_owned(), "seed(1)".to_owned()]),
    );
}

#[test]
fn late_constructed_refusal_retains_the_original_statement() {
    let x = Term::variable(VarName::new("X").unwrap());
    let p = |value| Atom::new(Name::new("p").unwrap(), [value]);
    let q = Atom::new(Name::new("q").unwrap(), [Term::from(1) / x.clone()]);
    let program = Arc::new(Program::of([
        Rule::fact(p(Term::from(0))),
        Rule::new(q, p(x)),
    ]));
    let original = Arc::downgrade(&program);
    let failures = [
        prepare(Arc::clone(&program)).ground().unwrap_err(),
        prepare(Arc::clone(&program)).ground_hybrid().unwrap_err(),
        prepare(Arc::clone(&program)).ground_adaptive().unwrap_err(),
        prepare(program)
            .ground_with_count_plan(
                zetesis_themelios::CountPlanLimits::default(),
                &Cancellation::default(),
                None,
            )
            .unwrap_err(),
    ];
    for failure in failures {
        let FormulaFailure::Program { program, error } = &failure else {
            panic!("the consumed input must accompany its refusal");
        };
        assert!(Arc::ptr_eq(program, &original.upgrade().unwrap()));
        assert!(matches!(
            error.as_ref(),
            FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
        ));
        assert!(failure.site().unwrap().location().is_none());
        assert!(failure.diagnostics().is_empty());
        let ProgramSubject::Statement(statement) = failure.subject().unwrap() else {
            panic!("the failed division belongs to its original rule");
        };
        assert!(
            program
                .statements()
                .any(|original| std::ptr::eq(original, statement))
        );
        let Statement::Rule(rule) = statement.get() else {
            panic!("rule subject")
        };
        let Head::Literal(literal) = rule.head().get() else {
            panic!("literal head")
        };
        assert!(
            matches!(&literal.inner, LiteralInner::Atom(atom) if atom.get().name.as_str() == "q")
        );
    }
}

#[test]
fn constructed_hybrid_setup_refusal_retains_its_program() {
    let program = Arc::new(Program::of([bounded_choice(), Rule::constraint(atom("q"))]));
    let original = Arc::downgrade(&program);
    let owner = prepare(program).ground_hybrid().unwrap();
    let failure = owner
        .checker(ConstraintCheckLimits {
            max_work: 0,
            ..ConstraintCheckLimits::default()
        })
        .err()
        .expect("the snapshot needs work");
    drop(owner);
    let ConstraintCheckCause::Source(error) = failure.cause else {
        panic!("snapshot work has a logical source refusal");
    };
    assert!(matches!(error.cause(), FormulaFailure::Limit { .. }));
    assert_retained_program(&error, &original);
}

#[test]
fn constructed_hybrid_check_refusal_retains_its_statement() {
    let program = Arc::new(Program::of([bounded_choice(), Rule::constraint(atom("q"))]));
    let original = Arc::downgrade(&program);
    let owner = prepare(program).ground_hybrid().unwrap();
    let setup_work = owner
        .checker(ConstraintCheckLimits::default())
        .unwrap()
        .statistics()
        .work;
    let mut checker = owner
        .checker(ConstraintCheckLimits {
            max_work: setup_work,
            ..ConstraintCheckLimits::default()
        })
        .unwrap();
    let model =
        Model::from_positions(owner.atom_catalog(), 0..owner.atom_catalog().atoms().len()).unwrap();
    let failure = checker.check(&model, &Cancellation::default()).unwrap_err();
    drop(checker);
    drop(model);
    drop(owner);
    let ConstraintCheckCause::Source(error) = failure.cause else {
        panic!("constraint preparation needs additional source work");
    };
    assert!(matches!(error.cause(), FormulaFailure::Limit { .. }));
    assert_retained_program(&error, &original);
    let ProgramSubject::Statement(statement) = error.subject().unwrap() else {
        panic!("the failed check belongs to the original constraint");
    };
    assert!(
        matches!(statement.get(), Statement::Rule(rule) if matches!(rule.head().get(), Head::Falsum))
    );
}

fn terminal_stop() -> (ReconstructionError, std::sync::Weak<Program>) {
    let program = terminal_program();
    let original = Arc::downgrade(&program);
    let FormulaMaterialization::Terminal(owner) = prepare(program).ground_adaptive().unwrap()
    else {
        panic!("the variable terminal definition is deferred");
    };
    let base = Model::from_positions(
        owner.base_atom_catalog(),
        0..owner.base_atom_catalog().atoms().len(),
    )
    .unwrap();
    let mut reconstruction = owner.reconstruction().unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let failure = reconstruction
        .reconstruct(&base, &cancellation)
        .unwrap_err();
    drop(reconstruction);
    drop(base);
    drop(owner);
    (failure, original)
}

#[test]
fn constructed_terminal_stop_retains_its_classification() {
    let (failure, _) = terminal_stop();
    assert_eq!(failure.stop(), Some(Stop::Cancelled));
}

#[test]
fn constructed_terminal_stop_retains_its_program() {
    let (failure, original) = terminal_stop();
    let ReconstructionError::Source(error) = failure else {
        panic!("reconstruction polls cancellation at a source work boundary");
    };
    assert_retained_program(&error, &original);
}

fn assert_retained_program(error: &FormulaFailure, original: &std::sync::Weak<Program>) {
    let FormulaFailure::Program { program, .. } = error else {
        panic!("the refusal retains its original logical input");
    };
    assert!(Arc::ptr_eq(program, &original.upgrade().unwrap()));
    assert!(error.site().unwrap().location().is_none());
    assert!(error.diagnostics().is_empty());
}
