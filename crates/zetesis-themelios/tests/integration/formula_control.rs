//! Canonical preparation separates input inspection, purpose and runtime control.

use std::{cell::Cell, collections::BTreeSet, sync::Arc, time::Instant};

use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_reference_support::exhaustive;
use zetesis_themelios::logical::{
    Name, Symbol, Term,
    program::{
        Atom, Body, Choice, ChoiceElement, Condition, Direction, Guard, Optimize, OptimizeElement,
        Program, Project, Relation, Rule, Show, Statement, weight,
    },
    symbol::VarName,
};
use zetesis_themelios::{
    ExpansionFailure, ExpansionResource, FormulaFailure, FormulaPurpose, GroundingObserver,
    GroundingPhase, ProgramAdmissionOptions, ProgramFormulaOptions, ProgramSite, ProgramSubject,
    prepare_program_formula_with, validate_program_formula,
};

fn atom(name: &str) -> Atom {
    Atom::constant(Name::new(name).unwrap())
}

fn choice() -> Rule {
    let bound = || Guard {
        relation: Some(Relation::Le),
        term: 1.into(),
    };
    Rule::fact(Choice::new(
        Some(bound()),
        ["p", "q"].map(|name| ChoiceElement::new(atom(name).into(), Condition::empty())),
        Some(bound()),
    ))
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
        Atom::new(
            Name::new("selected").unwrap(),
            [Term::from(1) / Term::from(0)],
        ),
        Body::empty(),
    )
    .into()
}

fn answers() -> ProgramFormulaOptions {
    ProgramFormulaOptions {
        purpose: FormulaPurpose::AnswerSets,
        ..ProgramFormulaOptions::default()
    }
}

#[test]
fn validation_does_not_expand_finite_facts() {
    let interval = Term::Interval {
        lower: Box::new(1.into()),
        upper: Box::new(i32::MAX.into()),
    };
    let program = Arc::new(Program::of([Rule::fact(Atom::new(
        Name::new("p").unwrap(),
        [interval],
    ))]));
    validate_program_formula(&program, ProgramAdmissionOptions::default()).unwrap();
    let error =
        prepare_program_formula_with(program, ProgramFormulaOptions::default()).unwrap_err();
    assert!(matches!(
        error.cause(),
        FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::Templates,
            ..
        })
    ));
    assert_eq!(error.interruption(), None);
}

#[test]
fn excluded_declarations_are_not_evaluated() {
    for directive in [objective(), projection()] {
        let program = Arc::new(Program::of([choice().into(), directive.clone()]));
        let ordinary =
            prepare_program_formula_with(Arc::clone(&program), ProgramFormulaOptions::default())
                .and_then(zetesis_themelios::PreparedFormula::ground)
                .unwrap_err();
        let ProgramSubject::Statement(subject) = ordinary.subject().unwrap() else {
            panic!("the undefined directive retains its original subject");
        };
        assert_eq!(subject.get(), &directive);
        let input = prepare_program_formula_with(Arc::clone(&program), answers())
            .unwrap()
            .ground()
            .unwrap();
        assert!(std::ptr::eq(input.original_program(), program.as_ref()));
        assert!(!input.projection().is_explicit());
        assert_eq!(
            exhaustive(&input),
            ["p", "q"]
                .map(|name| (BTreeSet::from([name.to_owned()]), None))
                .into()
        );
    }
}

#[test]
fn excluded_declarations_do_not_block_hybrid() {
    let program = Arc::new(Program::of([choice().into(), objective(), projection()]));
    let mut options = answers();
    options.formula.objective.max_templates = 0;
    options.expansion.max_metadata_statements = 0;
    let input = prepare_program_formula_with(program, options)
        .unwrap()
        .ground_hybrid()
        .unwrap();
    assert!(!input.projection().is_explicit());
}

#[test]
fn answer_sets_preserve_show() {
    let program = Arc::new(Program::of([
        choice().into(),
        objective(),
        projection(),
        Show::Term(7.into()).into(),
    ]));
    let input = prepare_program_formula_with(program, answers())
        .unwrap()
        .ground()
        .unwrap();
    let model = Model::from_positions(input.atom_catalog(), []).unwrap();
    let shown = input
        .metadata()
        .observations()
        .evaluate(
            &model,
            zetesis_themelios::observation::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(shown.symbols(), &[Symbol::Number(7)]);
}

#[test]
fn excluded_objective_keeps_later_failure_identity() {
    let show = Statement::from(Show::Term(Term::variable(VarName::new("X").unwrap())));
    let program = Arc::new(Program::of([choice().into(), objective(), show.clone()]));
    let (index, original) = program
        .statements()
        .enumerate()
        .find(|(_, carrier)| carrier.get() == &show)
        .unwrap();
    // An excluded declaration must precede the refused one in canonical order,
    // otherwise a compiler renumbering a reduced program would pass this test.
    assert!(
        program
            .statements()
            .take(index)
            .any(|carrier| matches!(carrier.get(), Statement::Optimize(_)))
    );
    let error = prepare_program_formula_with(Arc::clone(&program), answers()).unwrap_err();
    assert!(
        matches!(error.cause(), FormulaFailure::Observation { error }
        if matches!(error.kind(), zetesis_themelios::observation::ErrorKind::Unsupported(
            zetesis_themelios::observation::Feature::UnsafeVariable
        )))
    );
    let FormulaFailure::Program {
        program: retained, ..
    } = &error
    else {
        panic!("the refusal retains its original canonical program");
    };
    assert!(Arc::ptr_eq(retained, &program));
    let ProgramSubject::Statement(subject) = error.subject().unwrap() else {
        panic!("the unbound show term refuses its original declaration");
    };
    assert!(std::ptr::eq(subject, original));
    let site = error.site().unwrap();
    assert_eq!(site.statement_id().unwrap().index(), index);
    assert!(site.location().is_none());
}

fn controlled(cancellation: Cancellation) -> ProgramFormulaOptions {
    ProgramFormulaOptions {
        cancellation: Some(cancellation),
        ..ProgramFormulaOptions::default()
    }
}

fn assert_stopped(error: &FormulaFailure, program: &Arc<Program>, reason: Stop) {
    assert_eq!(error.interruption(), Some(reason));
    let FormulaFailure::Program {
        program: retained, ..
    } = error
    else {
        panic!("a stopped canonical operation retains its original owner");
    };
    assert!(Arc::ptr_eq(retained, program));
    assert!(error.diagnostics().is_empty());
}

#[test]
fn preparation_preserves_control_reason() {
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let expired = Cancellation::with_deadline(Instant::now()).unwrap();
    for (cancellation, reason) in [(cancelled, Stop::Cancelled), (expired, Stop::Deadline)] {
        let program = Arc::new(Program::of([choice()]));
        let error = prepare_program_formula_with(Arc::clone(&program), controlled(cancellation))
            .unwrap_err();
        assert_stopped(&error, &program, reason);
    }
}

#[test]
fn control_persists_into_every_materialization_request() {
    let program = Arc::new(Program::of([choice()]));
    let cancellation = Cancellation::default();
    let prepare = || {
        prepare_program_formula_with(Arc::clone(&program), controlled(cancellation.clone()))
            .unwrap()
    };
    let eager = prepare();
    let hybrid = prepare();
    let adaptive = prepare();
    cancellation.cancel();
    for error in [
        eager.ground().unwrap_err(),
        hybrid.ground_hybrid().unwrap_err(),
        adaptive.ground_adaptive().unwrap_err(),
    ] {
        assert_stopped(&error, &program, Stop::Cancelled);
    }
}

struct CancelRule {
    cancellation: Cancellation,
    site: Cell<Option<ProgramSite>>,
}
impl GroundingObserver for CancelRule {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_enter(&self, phase: GroundingPhase, site: Option<ProgramSite>) {
        if phase == GroundingPhase::RuleInstantiation {
            self.site.set(site);
            self.cancellation.cancel();
        }
    }
}

#[test]
fn rule_instantiation_observes_shared_cancellation() {
    let program = Arc::new(Program::of([choice()]));
    let cancellation = Cancellation::default();
    let prepared =
        prepare_program_formula_with(Arc::clone(&program), controlled(cancellation.clone()))
            .unwrap();
    let observer = CancelRule {
        cancellation,
        site: Cell::new(None),
    };
    let error = prepared.ground_with_observer(Some(&observer)).unwrap_err();
    assert_stopped(&error, &program, Stop::Cancelled);
    let site = observer
        .site
        .get()
        .expect("the rule instantiation phase ran");
    assert_eq!(error.site(), Some(site));
    assert!(matches!(
        error.subject(),
        Some(ProgramSubject::Statement(_))
    ));
}
