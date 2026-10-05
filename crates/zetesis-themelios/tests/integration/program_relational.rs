//! Canonical relational preparation preserves the existing compiler's semantics.

use std::{collections::BTreeSet, sync::Arc, time::Instant};

use zetesis_core::{GroundProgram, Model, Seed, StaticLimits};
use zetesis_cpu::{
    Cancellation, CandidateLimits, CandidateTermination, Candidates, Limits, Stop, check,
    check_static,
};
use zetesis_themelios::logical::{
    Name, Symbol, Term,
    program::{
        Atom, Body, BodyElement, Choice, ChoiceElement, Condition, Const, Direction, Guard,
        Optimize, OptimizeElement, Program, Project, Relation, Rule, Show, Statement, weight,
    },
    symbol::{Sign, VarName},
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaPurpose, ProgramFailureKind, ProgramRelationalOptions, ProgramSubject, admit_extended,
    prepare_program_relational, symbols,
};

fn atom(name: &str, terms: impl IntoIterator<Item = Term>) -> Atom {
    Atom::new(Name::new(name).unwrap(), terms)
}

fn variable() -> Term {
    Term::variable(VarName::new("X").unwrap())
}

fn body(atoms: impl IntoIterator<Item = Atom>) -> Body {
    Body::new(
        atoms
            .into_iter()
            .map(|atom| BodyElement::Literal(atom.into())),
    )
}

fn interval() -> Term {
    Term::Interval {
        lower: Box::new(1.into()),
        upper: Box::new(2.into()),
    }
}

fn rules() -> [Statement; 3] {
    [
        Rule::fact(atom("d", [interval()])).into(),
        Rule::fact(Choice::new(
            None,
            [ChoiceElement::new(
                atom("picked", []).into(),
                Condition::empty(),
            )],
            None,
        ))
        .into(),
        Rule::new(
            atom("seen", [variable()]),
            body([atom("d", [variable()]), atom("picked", [])]),
        )
        .into(),
    ]
}

fn symbol(name: &str, arguments: impl IntoIterator<Item = Symbol>) -> Symbol {
    Symbol::Function {
        name: Name::new(name).unwrap(),
        arguments: arguments.into_iter().collect(),
        sign: Sign::Positive,
    }
}

fn full(model: &Model) -> BTreeSet<Symbol> {
    model
        .atoms()
        .iter()
        .map(|atom| symbols::atom_with(atom, 4096, || Ok::<_, Stop>(())).unwrap())
        .collect()
}

fn models(
    program: &zetesis_core::Program,
    mut membership: impl FnMut(&Seed) -> Option<Model>,
) -> Vec<Model> {
    let mut candidates =
        Candidates::new(program, CandidateLimits::default(), Cancellation::default());
    let result = candidates
        .by_ref()
        .filter_map(|seed| membership(&seed.unwrap()))
        .collect();
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Exhausted)
    );
    result
}

fn lazy_models(program: &zetesis_core::Program) -> Vec<Model> {
    models(program, |seed| {
        let checked = check(program, seed, Limits::default(), &Cancellation::default()).unwrap();
        checked.accepted().then(|| checked.closure().clone())
    })
}

fn family(models: &[Model]) -> BTreeSet<BTreeSet<Symbol>> {
    let family = models.iter().map(full).collect::<BTreeSet<_>>();
    assert_eq!(family.len(), models.len());
    family
}

fn expected() -> BTreeSet<BTreeSet<Symbol>> {
    let facts = BTreeSet::from([symbol("d", [1.into()]), symbol("d", [2.into()])]);
    let mut chosen = facts.clone();
    chosen.extend([
        symbol("picked", []),
        symbol("seen", [1.into()]),
        symbol("seen", [2.into()]),
    ]);
    [facts, chosen].into()
}

fn answers() -> ProgramRelationalOptions {
    ProgramRelationalOptions {
        purpose: FormulaPurpose::AnswerSets,
        ..ProgramRelationalOptions::default()
    }
}

fn objective() -> Statement {
    Optimize::new(
        Direction::Minimize,
        [OptimizeElement::new(
            weight(Term::from(1) / Term::from(0)),
            [],
            Condition::empty(),
        )],
    )
    .into()
}

fn projection() -> Statement {
    Project::atom_body(
        atom("selected", [Term::from(1) / Term::from(0)]),
        Body::empty(),
    )
    .into()
}

#[test]
fn relational_execution_preserves_the_answer_family() {
    let input = prepare_program_relational(Arc::new(Program::of(rules())), answers()).unwrap();
    let graph = GroundProgram::compile(input.program(), StaticLimits::default()).unwrap();
    let eager = models(input.program(), |seed| {
        let checked =
            check_static(&graph, seed, Limits::default(), &Cancellation::default()).unwrap();
        checked
            .accepted()
            .then(|| checked.interpretation(&graph).unwrap())
    });
    // These invoke the source-join and static-graph membership implementations,
    // respectively; agreement alone would not detect a shared incomplete family.
    assert_eq!(family(&lazy_models(input.program())), expected());
    assert_eq!(family(&eager), expected());
}

#[test]
fn extended_source_charges_are_preserved() {
    let source = admit_extended(
        "d(1..2). {picked}. seen(X) :- d(X), picked.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let typed = prepare_program_relational(
        Arc::new(Program::of(rules())),
        ProgramRelationalOptions::default(),
    )
    .unwrap();
    assert_eq!(typed.expansion_usage(), source.expansion_usage());
    assert_eq!(family(&lazy_models(source.program())), expected());
    assert_eq!(family(&lazy_models(typed.program())), expected());
}

#[test]
fn excluded_directives_do_not_consume_semantic_budgets() {
    let program = Arc::new(Program::of(
        rules().into_iter().chain([objective(), projection()]),
    ));
    let mut options = answers();
    options.expansion.max_metadata_statements = 0;
    let input = prepare_program_relational(Arc::clone(&program), options).unwrap();
    assert!(std::ptr::eq(input.original_program(), program.as_ref()));
    assert!(!input.metadata().project_selection().is_explicit());
    assert_eq!(family(&lazy_models(input.program())), expected());
}

#[test]
fn ordinary_preparation_retains_directive_refusals() {
    for directive in [objective(), projection()] {
        let program = Arc::new(Program::of([directive.clone()]));
        let error =
            prepare_program_relational(Arc::clone(&program), ProgramRelationalOptions::default())
                .unwrap_err();
        let ProgramSubject::Statement(subject) = error.subject().unwrap() else {
            panic!("the original unsupported directive is the failure subject");
        };
        assert_eq!(subject.get(), &directive);
        assert!(std::ptr::eq(
            error.original_program().unwrap(),
            program.as_ref()
        ));
    }
}

#[test]
fn show_metadata_preserves_full_interpretations() {
    let show = Show::term_body(variable(), body([atom("seen", [variable()])]));
    let program = Program::of(rules().into_iter().chain([
        Show::All.into(),
        show.into(),
        objective(),
        projection(),
    ]));
    let input = prepare_program_relational(Arc::new(program), answers()).unwrap();
    let models = lazy_models(input.program());
    assert_eq!(family(&models), expected());
    let observations = models
        .iter()
        .map(|model| {
            assert!(
                model
                    .atoms()
                    .iter()
                    .all(|atom| !input.metadata().output().includes(atom))
            );
            input
                .metadata()
                .observations()
                .evaluate(
                    model,
                    zetesis_themelios::observation::Limits::default(),
                    &Cancellation::default(),
                )
                .unwrap()
                .into_symbols()
                .into_iter()
                .collect::<BTreeSet<_>>()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observations,
        [BTreeSet::new(), [1.into(), 2.into()].into()].into()
    );
}

#[test]
fn show_failure_keeps_its_unfiltered_statement_identity() {
    let show = Statement::from(Show::Term(variable()));
    let program = Arc::new(Program::of([objective(), show.clone()]));
    let (index, original) = program
        .statements()
        .enumerate()
        .find(|(_, statement)| statement.get() == &show)
        .unwrap();
    assert!(
        program
            .statements()
            .take(index)
            .any(|statement| matches!(statement.get(), Statement::Optimize(_)))
    );
    let error = prepare_program_relational(Arc::clone(&program), answers()).unwrap_err();
    assert!(
        matches!(error.cause(), FormulaFailure::Observation { error }
        if matches!(error.kind(), zetesis_themelios::observation::ErrorKind::Unsupported(
            zetesis_themelios::observation::Feature::UnsafeVariable)))
    );
    let ProgramSubject::Statement(subject) = error.subject().unwrap() else {
        panic!("the unbound show term retains its own original carrier");
    };
    assert!(std::ptr::eq(subject, original));
    let site = error.site().unwrap();
    assert_eq!(site.statement_id().unwrap().index(), index);
    assert!(site.location().is_none());
    assert!(error.diagnostics().is_empty());
}

#[test]
fn expanded_templates_retain_original_evidence() {
    let positive = Statement::from(Rule::fact(atom("p", [interval()])));
    let mut negative = atom("p", [1.into()]);
    negative.sign = Sign::Negative;
    let negative = Statement::from(Rule::fact(negative));
    let program = Arc::new(Program::of([
        positive.clone(),
        negative.clone(),
        objective(),
    ]));
    let input = prepare_program_relational(Arc::clone(&program), answers()).unwrap();
    let originals = program.statements().collect::<Vec<_>>();
    assert!(std::ptr::eq(input.original_program(), program.as_ref()));
    let mut positive_templates = 0;
    let mut coherence = 0;
    for (template, sites) in input
        .program()
        .templates()
        .iter()
        .zip(input.template_sites())
    {
        let subjects = sites
            .iter()
            .map(|site| {
                assert!(site.location().is_none());
                originals[site.statement_id().unwrap().index()].get()
            })
            .collect::<BTreeSet<_>>();
        if template.head().is_none() {
            assert_eq!(subjects, [&positive, &negative].into());
            coherence += 1;
        } else if subjects == [&positive].into() {
            positive_templates += 1;
        } else {
            assert_eq!(subjects, [&negative].into());
        }
    }
    assert_eq!(
        input.template_sites().len(),
        input.program().templates().len()
    );
    assert_eq!(positive_templates, 2);
    assert_eq!(coherence, 1);
}

#[test]
fn excluded_input_remains_structurally_bounded() {
    let program = Arc::new(Program::of([objective()]));
    let mut options = answers();
    options.admission.max_nodes = 1;
    let error = prepare_program_relational(program, options).unwrap_err();
    assert!(matches!(
        error.cause(),
        FormulaFailure::Logical {
            error: ProgramFailureKind::Limit(_),
            ..
        }
    ));
}

#[test]
fn preparation_preserves_control_reasons() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let expired = Cancellation::with_deadline(Instant::now()).unwrap();
    let program = Arc::new(Program::of(rules()));
    for (cancellation, reason) in [(cancelled, Stop::Cancelled), (expired, Stop::Deadline)] {
        let mut options = answers();
        options.cancellation = Some(cancellation);
        let error = prepare_program_relational(Arc::clone(&program), options).unwrap_err();
        assert_eq!(error.interruption(), Some(reason));
        let FormulaFailure::Program {
            program: retained, ..
        } = error
        else {
            panic!("a stopped preparation retains its owner");
        };
        assert!(Arc::ptr_eq(&retained, &program));
    }
    let retry = prepare_program_relational(Arc::clone(&program), answers()).unwrap();
    assert_eq!(family(&lazy_models(retry.program())), expected());
}

#[test]
fn bounded_choice_is_an_explicit_relational_refusal() {
    let choice = Choice::new(
        Some(Guard {
            relation: Some(Relation::Le),
            term: 1.into(),
        }),
        [ChoiceElement::new(atom("p", []).into(), Condition::empty())],
        None,
    );
    let program = Arc::new(Program::of([Rule::fact(choice)]));
    let error = prepare_program_relational(Arc::clone(&program), answers()).unwrap_err();
    assert!(matches!(
        error.cause(),
        FormulaFailure::Expansion(ExpansionFailure::Admission(
            zetesis_themelios::AdmissionFailure::Profile {
                feature: zetesis_themelios::ProfileFeature::BoundedChoice,
                ..
            }
        ))
    ));
    assert!(std::ptr::eq(
        error.original_program().unwrap(),
        program.as_ref()
    ));
    assert!(matches!(
        error.subject(),
        Some(ProgramSubject::Statement(_))
    ));
}

#[test]
fn observation_work_shares_the_expansion_allowance() {
    let constant = Statement::from(Const::new(
        Name::new("n").unwrap(),
        Term::from(2) + Term::from(3),
        None,
    ));
    let value = Term::Symbolic(symbol("n", []));
    let fact = Statement::from(Rule::fact(atom("p", [value.clone()])));
    let plain = prepare_program_relational(
        Arc::new(Program::of([constant.clone(), fact.clone()])),
        answers(),
    )
    .unwrap();
    let program = Arc::new(Program::of([constant, fact, Show::Term(value).into()]));
    let complete = prepare_program_relational(Arc::clone(&program), answers()).unwrap();
    let limit = complete.expansion_usage().term_work - 1;
    assert!(limit >= plain.expansion_usage().term_work);
    let mut options = answers();
    options.expansion.max_term_work = limit;
    let error = prepare_program_relational(program, options).unwrap_err();
    assert!(
        matches!(error.cause(), FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::TermWork, observed, ..
    }) if *observed > limit as u128)
    );
}
