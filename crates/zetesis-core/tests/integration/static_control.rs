//! Controlled eager grounding preserves complete rows and typed refusal prefixes.
use zetesis_core::{Filter, GroundProgram, StaticFailure, StaticLimits, Template, Term};
use zetesis_test_support::programs::{number, pattern, program};

fn fixture() -> zetesis_core::Program {
    program(vec![
        Template::new(
            Some(pattern("d", vec![number(0)])),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("d", vec![number(1)])),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("p", vec![Term::Variable(0)])),
            vec![
                pattern("d", vec![Term::Variable(0)]),
                pattern("d", vec![Term::Variable(0)]),
            ],
            vec![pattern("q", vec![Term::Variable(0)])],
            vec![],
            vec![Filter::Eq(Term::Variable(0), Term::Variable(0))],
        ),
    ])
}

#[test]
fn controlled_compilation_preserves_the_complete_graph() {
    let source = fixture();
    let expected = GroundProgram::compile(&source, StaticLimits::default()).unwrap();
    let mut calls = 0;
    let actual = GroundProgram::compile_with(&source, StaticLimits::default(), || {
        calls += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert!(calls > actual.atom_count() + actual.rules().len());
    assert!(actual.program().same_instance(&source));
    assert!(actual.atoms().iter().eq(expected.atoms().iter()));
    assert_eq!(actual.rules(), expected.rules());
    assert_eq!(actual.gate_atom_ids(), expected.gate_atom_ids());
}

#[test]
fn every_control_refusal_leaves_the_source_reusable() {
    let source = fixture();
    let mut calls = 0;
    let expected = GroundProgram::compile_with(&source, StaticLimits::default(), || {
        calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    for cutoff in 0..calls {
        let mut accepted = 0;
        let error = GroundProgram::compile_with(&source, StaticLimits::default(), || {
            if accepted == cutoff {
                return Err(cutoff);
            }
            accepted += 1;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error, StaticFailure::Stopped(cutoff));
        assert_eq!(accepted, cutoff);
    }
    let retried = GroundProgram::compile(&source, StaticLimits::default()).unwrap();
    assert!(retried.atoms().iter().eq(expected.atoms().iter()));
    assert_eq!(retried.rules(), expected.rules());
}

#[test]
fn rejected_substitutions_still_poll_control() {
    let mut rules = vec![
        Template::new(
            Some(pattern("d", vec![number(0)])),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        Template::new(
            Some(pattern("d", vec![number(1)])),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
    ];
    rules.push(Template::new(
        Some(pattern("p", vec![])),
        (0..12)
            .map(|slot| pattern("d", vec![Term::Variable(slot)]))
            .collect(),
        vec![],
        vec![],
        vec![Filter::Neq(Term::Variable(0), Term::Variable(0))],
    ));
    let source = program(rules);
    let mut remaining = 2000;
    let result = GroundProgram::compile_with(
        &source,
        StaticLimits {
            max_substitutions: usize::MAX,
            ..StaticLimits::default()
        },
        || {
            if remaining == 0 {
                return Err("stop");
            }
            remaining -= 1;
            Ok(())
        },
    );
    assert!(matches!(result, Err(StaticFailure::Stopped("stop"))));
    assert_eq!(remaining, 0);
}

#[test]
fn controlled_compilation_preserves_static_refusals() {
    let source = fixture();
    let limits = StaticLimits {
        max_atoms: 0,
        ..StaticLimits::default()
    };
    let expected = GroundProgram::compile(&source, limits).unwrap_err();
    let actual = GroundProgram::compile_with(&source, limits, || Ok::<_, ()>(())).unwrap_err();
    assert_eq!(actual, StaticFailure::Static(expected));
}
