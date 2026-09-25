//! Metadata limits refuse whole policies at every incomplete input prefix.
//!
//! The canonical program is borrowed unchanged across attempts. A complete policy
//! must preserve nested values and conditions; a refusal must retain its original
//! location and the exact independently selected resource.

use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_themelios::base::source::{Source, SourceId};
use zetesis_themelios::base::span::Location;
use zetesis_themelios::logical::program::Program;
use zetesis_themelios::logical::symbol::{Name, Sign, Symbol};
use zetesis_themelios::observation::{AdmissionLimits, ErrorKind, Limits, Resource};
use zetesis_themelios::{
    FormulaFailure, MetadataError, MetadataLimits, MetadataResource, SourceMetadata,
};

const TEXT: &str = "#const k=2. #show -pair(f(X),(Y,|X-k|)):data(X,Y),X+1<=Y<=X+2,not hidden(X),not not enabled(Y). #show data/2. #defined data/2. data(1,2).";

fn input() -> (Source, Program, Location) {
    let source = Source::new(SourceId::new(175), TEXT.into()).unwrap();
    let parsed = zetesis_themelios::syntax::parse::parse(
        &source,
        zetesis_themelios::syntax::dialect::Dialect::Clingo,
    );
    assert!(parsed.diagnostics().is_empty());
    let raised = zetesis_themelios::logical::raise::raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    (source, raised.program().clone(), location)
}

#[test]
fn bounded_metadata_preserves_nested_conditions() {
    let (_, program, location) = input();
    let policy = SourceMetadata::compile(&program, MetadataLimits::default(), location).unwrap();
    let atom = |name, values: Vec<Value>| {
        Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
    };
    let data = atom("data", vec![Value::Number(1), Value::Number(2)]);
    let enabled = atom("enabled", vec![Value::Number(2)]);
    let hidden = atom("hidden", vec![Value::Number(1)]);
    let expected = Symbol::Function {
        name: Name::new("pair").unwrap(),
        sign: Sign::Negative,
        arguments: vec![
            Symbol::Function {
                name: Name::new("f").unwrap(),
                sign: Sign::Positive,
                arguments: vec![Symbol::Number(1)],
            },
            Symbol::Tuple(vec![Symbol::Number(2), Symbol::Number(1)]),
        ],
    };
    for (model, expected) in [
        (
            Model::new([data.clone(), enabled.clone()]).unwrap(),
            vec![expected],
        ),
        (Model::new([data.clone()]).unwrap(), vec![]),
        (Model::new([data, enabled, hidden]).unwrap(), vec![]),
    ] {
        let observations = policy
            .observations()
            .evaluate(&model, Limits::default(), &Cancellation::default())
            .unwrap();
        assert_eq!(observations.symbols(), expected);
    }
}

fn metadata_limit(resource: MetadataResource, limit: usize) -> MetadataLimits {
    let mut limits = MetadataLimits::default();
    match resource {
        MetadataResource::Statements => limits.max_statements = limit,
        MetadataResource::Nodes => limits.max_nodes = limit,
        MetadataResource::Depth => limits.max_depth = limit,
        MetadataResource::TextBytes => limits.max_text_bytes = limit,
        MetadataResource::Origins => limits.max_origins = limit,
    }
    limits
}

#[test]
fn every_incomplete_input_budget_refuses_the_policy() {
    let (source, program, location) = input();
    let expected = SourceMetadata::compile(&program, MetadataLimits::default(), location).unwrap();
    for resource in [
        MetadataResource::Statements,
        MetadataResource::Nodes,
        MetadataResource::Depth,
        MetadataResource::TextBytes,
        MetadataResource::Origins,
    ] {
        let mut complete = false;
        // This is a finite fixture exploration ceiling, not a production limit.
        for limit in 0..=512 {
            match SourceMetadata::compile(&program, metadata_limit(resource, limit), location) {
                Ok(actual) => {
                    assert!(limit > 0);
                    assert_eq!(actual, expected);
                    complete = true;
                    break;
                }
                Err(MetadataError::Limit {
                    resource: actual,
                    limit: actual_limit,
                    observed,
                    location,
                }) => {
                    assert_eq!(actual, resource);
                    assert_eq!(actual_limit, limit);
                    assert!(observed > limit as u128);
                    assert_eq!(location.source, source.id());
                    assert!(!source.slice(location.span).unwrap().is_empty());
                }
                Err(error) => panic!("{resource:?}/{limit}: {error}"),
            }
        }
        assert!(
            complete,
            "fixture exceeds its finite exploration: {resource:?}"
        );
    }
}

fn observation_limit(resource: Resource, limit: u32) -> AdmissionLimits {
    let mut limits = AdmissionLimits::default();
    match resource {
        Resource::Directives => limits.max_directives = limit,
        Resource::Nodes => limits.max_nodes = limit,
        Resource::Depth => limits.max_depth = limit,
        Resource::Bytes => limits.max_bytes = limit,
        Resource::Variables => limits.max_variables = limit,
        Resource::BodyElements => limits.max_body_elements = limit,
        Resource::Arity => limits.max_arity = limit,
        Resource::Origins => limits.max_origins = limit,
        _ => panic!("not a template admission resource"),
    }
    limits
}

#[test]
fn every_incomplete_template_budget_refuses_the_policy() {
    let (source, program, location) = input();
    let expected = SourceMetadata::compile(&program, MetadataLimits::default(), location).unwrap();
    for resource in [
        Resource::Directives,
        Resource::Nodes,
        Resource::Depth,
        Resource::Bytes,
        Resource::Variables,
        Resource::BodyElements,
        Resource::Arity,
        Resource::Origins,
    ] {
        let mut complete = false;
        for limit in 0..=512 {
            let limits = MetadataLimits {
                observations: observation_limit(resource, limit),
                ..MetadataLimits::default()
            };
            match SourceMetadata::compile(&program, limits, location) {
                Ok(actual) => {
                    assert!(limit > 0);
                    assert_eq!(actual, expected);
                    complete = true;
                    break;
                }
                Err(MetadataError::Compilation(FormulaFailure::Observation { error })) => {
                    assert!(
                        matches!(
                            error.kind(),
                            ErrorKind::Limit { resource: actual, limit: actual_limit, observed }
                                if *actual == resource && *actual_limit == u128::from(limit)
                                    && *observed > u128::from(limit)
                        ),
                        "{resource:?}/{limit}: {error}"
                    );
                    let origin = error.location().unwrap();
                    assert_eq!(origin.source, source.id());
                    assert!(!source.slice(origin.span).unwrap().is_empty());
                    assert_eq!(error.statistics().work, 0, "no evaluation was started");
                }
                Err(error) => panic!("{resource:?}/{limit}: {error}"),
            }
        }
        assert!(
            complete,
            "fixture exceeds its finite exploration: {resource:?}"
        );
    }
}
