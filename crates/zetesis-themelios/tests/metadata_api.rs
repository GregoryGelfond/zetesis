//! Native metadata consumers use canonical vocabulary through the frontend alone.

use zetesis_core::{Atom, Model, Predicate, Sign, Value};
use zetesis_cpu::Cancellation;
use zetesis_themelios::base::source::{Source, SourceId};
use zetesis_themelios::base::span::{ByteOffset, Location, Span};
use zetesis_themelios::logical::program::{Const, Program, Show, Statement};
use zetesis_themelios::logical::provenance::WithProvenance;
use zetesis_themelios::logical::symbol::{Name, Sign as SymbolSign, Signature, Symbol};
use zetesis_themelios::logical::term::{Term, Variable};
use zetesis_themelios::observation::{ErrorKind, Feature, Limits, ObservationProgram};
use zetesis_themelios::{
    AdmissionOptions, AtomSelection, AtomSelectionError, AtomSelectionLimits, ExpansionFailure,
    ExpansionLimits, FormulaFailure, FormulaLimits, MetadataError, MetadataFeature, MetadataLimits,
    MetadataResource, SourceMetadata, admit_formula,
};

fn fallback() -> Location {
    Location {
        source: SourceId::new(73),
        span: Span::empty(ByteOffset::new(0)),
    }
}
fn program(text: &str) -> (Source, Program) {
    let source = Source::new(fallback().source, text.into()).unwrap();
    let parsed = zetesis_themelios::syntax::parse::parse(
        &source,
        zetesis_themelios::syntax::dialect::Dialect::Clingo,
    );
    assert!(parsed.diagnostics().is_empty());
    let raised = zetesis_themelios::logical::raise::raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    (source, raised.program().clone())
}
fn metadata(text: &str) -> SourceMetadata {
    SourceMetadata::compile(&program(text).1, MetadataLimits::default(), fallback()).unwrap()
}
fn model() -> Model {
    Model::new([
        Atom::new(Predicate::new("p", 1).unwrap(), vec![Value::Number(3)]).unwrap(),
        Atom::new(
            Predicate::with_sign("p", 1, Sign::Negative).unwrap(),
            vec![Value::Number(3)],
        )
        .unwrap(),
        Atom::new(Predicate::new("hidden", 0).unwrap(), vec![]).unwrap(),
    ])
    .unwrap()
}

#[test]
fn canonical_source_types_are_nameable() {
    let (source, program) = program("p.");
    let analysis: zetesis_themelios::analysis::Analysis =
        zetesis_themelios::analysis::Analysis::of(&program);
    let options = AdmissionOptions {
        source_id: source.id(),
        ..Default::default()
    };
    assert_eq!(options.source_id, fallback().source);
    let _: &Program = &program;
    let _ = analysis;
}

#[test]
fn explicit_selection_matches_source_policy() {
    let signature = Predicate::with_sign("p", 1, Sign::Negative).unwrap();
    let selection = AtomSelection::from_signatures(
        &[signature.clone(), signature],
        AtomSelectionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        &selection,
        metadata("#show -p/1. #show -p/1.").atom_selection()
    );
    let full = model();
    assert_eq!(
        full.atoms()
            .iter()
            .filter(|atom| selection.includes(*atom))
            .count(),
        1
    );
    assert_eq!(full.atoms().len(), 3);
}

#[test]
fn empty_selection_has_no_atom_channel() {
    let selection = AtomSelection::from_signatures(
        &[],
        AtomSelectionLimits {
            max_signatures: 0,
            max_name_bytes: 0,
            ..AtomSelectionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(selection, AtomSelection::none());
    assert_eq!(&selection, metadata("#show.").atom_selection());
    assert!(model().atoms().iter().all(|atom| !selection.includes(atom)));
}

#[test]
fn all_selection_matches_default_policy() {
    assert_eq!(&AtomSelection::all(), metadata("").atom_selection());
    assert!(
        model()
            .atoms()
            .iter()
            .all(|atom| AtomSelection::all().includes(atom))
    );
}

#[test]
fn selection_limits_count_duplicate_inputs() {
    let signatures = [
        Predicate::new("p", 0).unwrap(),
        Predicate::new("p", 0).unwrap(),
    ];
    assert!(
        AtomSelection::from_signatures(
            &signatures,
            AtomSelectionLimits {
                max_signatures: 2,
                max_name_bytes: 2,
                ..AtomSelectionLimits::default()
            }
        )
        .is_ok()
    );
    assert_eq!(
        AtomSelection::from_signatures(
            &signatures,
            AtomSelectionLimits {
                max_signatures: 1,
                max_name_bytes: 2,
                ..AtomSelectionLimits::default()
            }
        ),
        Err(AtomSelectionError::Signatures {
            limit: 1,
            observed: 2
        })
    );
}

#[test]
fn selection_text_limit_is_inclusive() {
    let signatures = [Predicate::new("name", 0).unwrap()];
    assert!(
        AtomSelection::from_signatures(
            &signatures,
            AtomSelectionLimits {
                max_signatures: 1,
                max_name_bytes: 4,
                ..AtomSelectionLimits::default()
            }
        )
        .is_ok()
    );
    assert_eq!(
        AtomSelection::from_signatures(
            &signatures,
            AtomSelectionLimits {
                max_signatures: 1,
                max_name_bytes: 3,
                ..AtomSelectionLimits::default()
            }
        ),
        Err(AtomSelectionError::NameBytes {
            limit: 3,
            observed: 4
        })
    );
}

#[test]
fn native_metadata_matches_source_admission() {
    let source = "#const c=2+1. #show -p/1. #show f(X,c):p(X). #defined q/1.";
    let (_, shared) = program(source);
    let native = SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap();
    let admitted = admit_formula(
        source.into(),
        AdmissionOptions {
            source_id: fallback().source,
            ..Default::default()
        },
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(&native, admitted.metadata());
    let observations =
        ObservationProgram::compile(&shared, MetadataLimits::default(), fallback()).unwrap();
    assert_eq!(&observations, native.observations());
    assert_eq!(
        native
            .observations()
            .evaluate(&model(), Limits::default(), &Cancellation::default())
            .unwrap()
            .symbols(),
        admitted
            .metadata()
            .observations()
            .evaluate(&model(), Limits::default(), &Cancellation::default())
            .unwrap()
            .symbols()
    );
}

#[test]
fn parsed_metadata_retains_duplicate_origins() {
    let (source, shared) = program("#show p/1. #show p/1.");
    let native = SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap();
    assert_eq!(native.directives().len(), 2);
    for directive in native.directives().iter() {
        assert_eq!(directive.location().source, source.id());
        assert_eq!(source.slice(directive.location().span), Ok("#show p/1."));
    }
}

#[test]
fn constructed_metadata_invents_no_parsed_origin() {
    let shared = Program::of_nodes([
        WithProvenance::constructed(Statement::Show(Show::Signature(Signature {
            name: Name::new("p").unwrap(),
            arity: 1,
            sign: SymbolSign::Negative,
        }))),
        WithProvenance::constructed(Statement::Show(Show::Term(Symbol::Number(7).into()))),
    ]);
    let native = SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap();
    assert!(native.directives().is_empty());
    assert_eq!(
        native.observations().origins().collect::<Vec<_>>(),
        vec![&[][..]]
    );
    assert_eq!(
        native
            .observations()
            .evaluate(&model(), Limits::default(), &Cancellation::default())
            .unwrap()
            .symbols(),
        &[Symbol::Number(7)]
    );
    assert_eq!(native.atom_selection().signatures().len(), 1);
}

#[test]
fn native_observations_enforce_positive_safety() {
    let (_, shared) = program("#show X:not p(X).");
    assert!(
        matches!(SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()), Err(MetadataError::Compilation(FormulaFailure::Observation { error })) if error.kind() == &ErrorKind::Unsupported(Feature::UnsafeVariable))
    );
}

#[test]
fn native_constants_refuse_cycles() {
    let (_, shared) = program("#const a=b. #const b=a.");
    assert!(matches!(
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()),
        Err(MetadataError::Expansion(
            ExpansionFailure::ConstantCycle { .. }
        ))
    ));
}

#[test]
fn native_constants_refuse_duplicate_origins() {
    let (_, shared) = program("#const a=1. #const a=1.");
    assert!(matches!(
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()),
        Err(MetadataError::Expansion(
            ExpansionFailure::DuplicateConstant { .. }
        ))
    ));
}

#[test]
fn native_constants_refuse_unground_values() {
    let shared = Program::of_nodes([WithProvenance::constructed(Statement::Const(Const {
        name: Name::new("a").unwrap(),
        value: Term::Variable(Variable::Anonymous),
        policy: None,
    }))]);
    assert!(matches!(
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()),
        Err(MetadataError::Unsupported {
            feature: MetadataFeature::Term,
            ..
        })
    ));
}

#[test]
fn native_input_limits_precede_compilation() {
    let (_, shared) = program("#show f(1).");
    for (limits, expected) in [
        (
            MetadataLimits {
                max_statements: 0,
                ..Default::default()
            },
            MetadataResource::Statements,
        ),
        (
            MetadataLimits {
                max_nodes: 0,
                ..Default::default()
            },
            MetadataResource::Nodes,
        ),
        (
            MetadataLimits {
                max_text_bytes: 0,
                ..Default::default()
            },
            MetadataResource::TextBytes,
        ),
        (
            MetadataLimits {
                max_origins: 0,
                ..Default::default()
            },
            MetadataResource::Origins,
        ),
        (
            MetadataLimits {
                max_depth: 0,
                ..Default::default()
            },
            MetadataResource::Depth,
        ),
    ] {
        assert!(
            matches!(SourceMetadata::compile(&shared, limits, fallback()), Err(MetadataError::Limit { resource, .. }) if resource == expected)
        );
    }
}

#[test]
fn native_metadata_refuses_parameterized_parts() {
    let (_, shared) = program("#program step(t). #show p/1.");
    assert!(matches!(
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()),
        Err(MetadataError::Unsupported {
            feature: MetadataFeature::ProgramPart,
            ..
        })
    ));
}

#[test]
fn source_free_atom_metadata_retains_no_queries() {
    let metadata = SourceMetadata::for_atoms(AtomSelection::none());
    assert_eq!(metadata.atom_selection(), metadata.output());
    assert!(metadata.observations().is_empty());
    assert!(metadata.directives().is_empty());
}

#[test]
fn native_input_limits_are_inclusive() {
    let (_, shared) = program("#show f(1).");
    let exact = MetadataLimits {
        max_statements: 1,
        max_nodes: 4,
        max_depth: 2,
        max_text_bytes: 1,
        max_origins: 1,
        ..Default::default()
    };
    assert!(SourceMetadata::compile(&shared, exact, fallback()).is_ok());
    assert!(matches!(
        SourceMetadata::compile(
            &shared,
            MetadataLimits {
                max_nodes: 3,
                ..exact
            },
            fallback()
        ),
        Err(MetadataError::Limit {
            resource: MetadataResource::Nodes,
            observed: 4,
            limit: 3,
            ..
        })
    ));
}

#[test]
fn native_depth_cap_precedes_recursive_compilation() {
    let mut symbol = Symbol::Number(1);
    for _ in 0..2_000 {
        symbol = Symbol::Tuple(vec![symbol]);
    }
    let shared = Program::of_nodes([WithProvenance::constructed(Statement::Show(Show::Term(
        symbol.into(),
    )))]);
    assert!(matches!(
        SourceMetadata::compile(
            &shared,
            MetadataLimits {
                max_depth: usize::MAX,
                ..Default::default()
            },
            fallback()
        ),
        Err(MetadataError::Limit {
            resource: MetadataResource::Depth,
            observed: 65,
            limit: 64,
            ..
        })
    ));
}

#[test]
fn metadata_compilation_does_not_admit_rules() {
    let native = metadata("p(X). #show 1.");
    assert_eq!(
        native
            .observations()
            .evaluate(
                &Model::new([]).unwrap(),
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .symbols(),
        &[Symbol::Number(1)]
    );
}

#[test]
fn native_constants_refuse_annotated_policy() {
    let (_, shared) = program("#const a=1. [default]");
    assert!(matches!(
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()),
        Err(MetadataError::Expansion(
            ExpansionFailure::ConstantPolicy { .. }
        ))
    ));
}

#[test]
fn native_constant_work_limit_is_enforced() {
    let (_, shared) = program("#const a=1. #show a.");
    let limits = MetadataLimits {
        expansion: ExpansionLimits {
            max_term_work: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        SourceMetadata::compile(&shared, limits, fallback()),
        Err(MetadataError::Compilation(FormulaFailure::Expansion(
            ExpansionFailure::Limit {
                resource: zetesis_themelios::ExpansionResource::TermWork,
                ..
            }
        )))
    ));
}

#[test]
fn native_directive_limit_counts_originals() {
    let (_, shared) = program("#show p/1. #show p/1.");
    let limits = MetadataLimits {
        expansion: ExpansionLimits {
            max_metadata_statements: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        SourceMetadata::compile(&shared, limits, fallback()),
        Err(MetadataError::Expansion(ExpansionFailure::Limit {
            resource: zetesis_themelios::ExpansionResource::MetadataStatements,
            observed: 2,
            limit: 1,
            ..
        }))
    ));
}

#[test]
fn native_metadata_preserves_structured_expression_values() {
    let native = metadata(
        "#const c=(1+2,\"x\"). #show. #show (c,-X,|X-2|):p(X),X+1=4. #show -p/1. #defined q/1.",
    );
    let evaluated = native
        .observations()
        .evaluate(&model(), Limits::default(), &Cancellation::default())
        .unwrap();
    assert_eq!(
        evaluated.symbols(),
        &[Symbol::Tuple(vec![
            Symbol::Tuple(vec![Symbol::Number(3), Symbol::String("x".into())]),
            Symbol::Number(-3),
            Symbol::Number(1),
        ])],
    );
    let expected = Predicate::with_sign("p", 1, Sign::Negative).unwrap();
    assert!(
        native
            .atom_selection()
            .signatures()
            .iter()
            .eq([zetesis_core::catalog::PredicateRef::from(&expected)])
    );
}

#[test]
fn canonical_strings_with_nulls_are_located_refusals() {
    // The owned symbol vocabulary can represent this string even though the
    // metadata/source bridge cannot. Refuse at the public borrowed-input door.
    let shared = Program::of_nodes([WithProvenance::constructed(Statement::Show(Show::Term(
        Symbol::String("before\0after".into()).into(),
    )))]);
    let error =
        SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap_err();
    assert!(matches!(error, MetadataError::Unsupported {
        feature: MetadataFeature::NullText, location
    } if location == fallback()));
    assert_eq!(error.to_string(), "metadata refuses NullText");
}

#[test]
fn native_metadata_refuses_unsupported_expression_shapes() {
    for source in ["#show (1;2).", "#show 1..2.", "#show @f(1)."] {
        let (original, shared) = program(source);
        let error =
            SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap_err();
        let MetadataError::Unsupported { feature, location } = error else {
            panic!("expected canonical metadata shape refusal: {error}");
        };
        assert_eq!(feature, MetadataFeature::Term, "{source}");
        assert_eq!(original.slice(location.span).unwrap(), source);
        assert_eq!(location.source, original.id());
    }
}

#[test]
fn constructed_tuple_limits_count_the_borrowed_nodes() {
    let shared = Program::of_nodes([WithProvenance::constructed(Statement::Show(Show::Term(
        Symbol::Tuple(vec![Symbol::Number(1), Symbol::String("x".into())]).into(),
    )))]);
    // One directive, its symbolic term, the tuple and its two children.
    let limits = MetadataLimits {
        max_nodes: 5,
        ..MetadataLimits::default()
    };
    let native = SourceMetadata::compile(&shared, limits, fallback()).unwrap();
    assert_eq!(
        native
            .observations()
            .evaluate(
                &Model::default(),
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .symbols(),
        &[Symbol::Tuple(vec![
            Symbol::Number(1),
            Symbol::String("x".into())
        ])],
    );
    let error = SourceMetadata::compile(
        &shared,
        MetadataLimits {
            max_nodes: 4,
            ..limits
        },
        fallback(),
    )
    .unwrap_err();
    assert!(matches!(error, MetadataError::Limit {
        resource: MetadataResource::Nodes, observed: 5, limit: 4, location
    } if location == fallback()));
    assert_eq!(error.to_string(), "metadata Nodes count 5 exceeds 4");
}

#[test]
fn native_metadata_displays_its_underlying_failure() {
    for source in ["#const a=b. #const b=a.", "#show X:not p(X)."] {
        let (_, shared) = program(source);
        let error =
            SourceMetadata::compile(&shared, MetadataLimits::default(), fallback()).unwrap_err();
        let message = match &error {
            MetadataError::Expansion(cause) => cause.to_string(),
            MetadataError::Compilation(cause) => cause.to_string(),
            other => panic!("expected a retained normalization or compilation cause: {other}"),
        };
        assert!(!message.is_empty());
        assert_eq!(error.to_string(), message, "{source}");
    }
}
