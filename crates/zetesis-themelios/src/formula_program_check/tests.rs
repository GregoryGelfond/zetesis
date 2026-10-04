use super::check;
use crate::{
    AdmissionOptions, CompilationFailure, ParsedSource, ProfileFeature, ProgramAdmissionOptions,
    ProgramFailureKind, ProgramResource, ProgramSubject,
};
use themelios_program::program::{Arguments, Atom, Program, Rule, Script, Statement};
use themelios_program::provenance::{Origin, Provenance, TransformTag, WithProvenance};
use themelios_program::raise::raise;
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::Term;

fn program(text: &str) -> Program {
    let source = ParsedSource::new(text.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(source.parsed());
    assert!(raised.diagnostics().is_empty(), "{text}");
    raised.into_program()
}

fn limit(program: &Program, options: ProgramAdmissionOptions, resource: ProgramResource) {
    let error = check(program, options).unwrap_err();
    assert!(matches!(error.kind, ProgramFailureKind::Limit(found) if found.resource == resource));
    let ProgramSubject::Statement(statement) = error.subject else {
        panic!("expected original statement");
    };
    assert!(
        program
            .statements()
            .any(|original| std::ptr::eq(original, statement))
    );
}

#[test]
fn formula_families_are_inspected_without_s0_restrictions() {
    for text in [
        "#const n=2. #defined p/1. p(1..n). #show f(X):p(X). #project p(X):p(X).",
        "{#true:p(1;1)}=2. p(1).",
        "p(X):q(X);r(X):s(X) :- d(X).",
        "1#count{1:p;2:#true:q}2. p :- 1#count{X:q(X)}.",
        "p :- 1{q(X):d(X)}2, r(X):d(X).",
        ":~ p(X), X>0. [X@2,X] #minimize{X@1,X:p(X)}.",
        "p((1,2),f(1;2)) :- 0<1<2.",
    ] {
        check(&program(text), ProgramAdmissionOptions::default()).unwrap();
    }
}

#[test]
fn nested_bodies_share_the_width_limit() {
    for text in ["#show f(1):p,q.", "#project p:p,q.", ":~ p,q. [1@0]"] {
        limit(
            &program(text),
            ProgramAdmissionOptions {
                max_body_elements: 1,
                ..ProgramAdmissionOptions::default()
            },
            ProgramResource::BodyElements,
        );
    }
}

#[test]
fn nested_formula_text_consumes_the_allowance() {
    for text in [
        "p :- 1#count{\"long payload\":q}.",
        "{#true:q(\"long payload\")}.",
        "#minimize{\"long payload\"@1:p}.",
        "#show \"long payload\".",
        "#const c=\"long payload\".",
    ] {
        limit(
            &program(text),
            ProgramAdmissionOptions {
                max_text_bytes: 4,
                ..ProgramAdmissionOptions::default()
            },
            ProgramResource::TextBytes,
        );
    }
}

#[test]
fn provenance_is_bounded_before_retention() {
    let fact = Rule::fact(Atom::constant(Name::new("p").unwrap()));
    for provenance in [
        Provenance::from(Origin::Constructed).with_doc("a long documentation payload"),
        Provenance::from(Origin::Transformed(TransformTag::new(
            "a long transformation tag",
        ))),
    ] {
        let program = Program::of_nodes([WithProvenance::new(
            Statement::Rule(fact.clone()),
            provenance,
        )]);
        limit(
            &program,
            ProgramAdmissionOptions {
                max_text_bytes: 4,
                ..ProgramAdmissionOptions::default()
            },
            ProgramResource::TextBytes,
        );
    }
}

#[test]
fn retained_structure_consumes_the_node_allowance() {
    let provenance = (0..32).fold(Provenance::empty(), |origins, index| {
        origins.merge(Provenance::from(Origin::Transformed(TransformTag::new(
            index.to_string(),
        ))))
    });
    let program = Program::of_nodes([WithProvenance::new(
        Statement::Rule(Rule::fact(Atom::constant(Name::new("p").unwrap()))),
        provenance,
    )]);
    limit(
        &program,
        ProgramAdmissionOptions {
            max_nodes: 20,
            ..ProgramAdmissionOptions::default()
        },
        ProgramResource::Nodes,
    );
    limit(
        &self::program("p. q. r."),
        ProgramAdmissionOptions {
            max_nodes: 20,
            ..ProgramAdmissionOptions::default()
        },
        ProgramResource::Nodes,
    );
}

#[test]
fn symbol_depth_is_bounded_inside_formula_terms() {
    let symbol = (0..64).fold(Symbol::Number(1), |child, _| Symbol::Tuple(vec![child]));
    let program = Program::of([Rule::fact(Atom::new(
        Name::new("p").unwrap(),
        [Term::Symbolic(symbol)],
    ))]);
    limit(
        &program,
        ProgramAdmissionOptions {
            max_depth: 16,
            ..ProgramAdmissionOptions::default()
        },
        ProgramResource::Depth,
    );
}

#[test]
fn empty_pools_are_rejected_before_source_normalization() {
    for atom in [
        Atom::new(Name::new("p").unwrap(), [Term::Pool(vec![])]),
        Atom {
            name: Name::new("p").unwrap(),
            sign: Sign::Positive,
            arguments: Arguments::Pooled(vec![]),
        },
    ] {
        let program = Program::of([Rule::fact(atom)]);
        assert!(matches!(
            check(&program, ProgramAdmissionOptions::default())
                .unwrap_err()
                .kind,
            ProgramFailureKind::Compilation(CompilationFailure::Profile(
                ProfileFeature::Term | ProfileFeature::PooledArguments
            ))
        ));
    }
}

#[test]
fn unsupported_shells_retain_the_actual_statement() {
    let program = Program::of([Statement::Script(Script::new(
        Name::new("python").unwrap(),
        "opaque",
    ))]);
    let error = check(&program, ProgramAdmissionOptions::default()).unwrap_err();
    assert!(matches!(
        error.kind,
        ProgramFailureKind::Compilation(CompilationFailure::Profile(ProfileFeature::Statement))
    ));
    assert!(matches!(error.subject, ProgramSubject::Statement(statement)
        if std::ptr::eq(statement, program.statements().next().unwrap())));
}

#[test]
fn unsupported_parts_retain_the_actual_key() {
    let program = program("#program step(t). p(t).");
    let error = check(&program, ProgramAdmissionOptions::default()).unwrap_err();
    assert!(matches!(error.subject, ProgramSubject::Part(key) if key.name.as_str() == "step"));
}

#[test]
fn constructed_objective_count_is_bounded_before_lowering() {
    let parsed = program("#minimize{1:a;2:b}. :~ c. [3@0]");
    let logical = Program::of(parsed.statements().map(|carrier| carrier.get().clone()));
    let mut limits = crate::FormulaLimits::default();
    limits.objective.max_templates = 3;
    super::check_objectives(&logical, &limits).unwrap();
    limits.objective.max_templates = 2;
    let error = super::check_objectives(&logical, &limits).unwrap_err();
    let crate::FormulaFailure::Limit {
        resource: crate::FormulaResource::ObjectiveElements,
        observed: 3,
        limit: 2,
        location,
    } = error
    else {
        panic!("expected objective-template refusal: {error}");
    };
    assert_eq!(location.location(), None);
    let carrier = logical
        .statements()
        .nth(location.statement_id().unwrap().index())
        .unwrap();
    assert!(matches!(
        carrier.get(),
        Statement::Optimize(_) | Statement::WeakConstraint(_)
    ));
}
