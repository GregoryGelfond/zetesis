//! Conditional alternatives stay inside their local quantifier in weak bodies.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;
#[path = "support/priority_contracts.rs"]
mod priority_contracts;

use themelios_program::program::{Arguments, BodyElement, LiteralInner, Statement};
use zetesis_themelios::{AnalysisBasis, FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = include_str!("fixtures/objective-pools.jsonl");

#[test]
fn conditional_pools_preserve_complete_scored_families() {
    assert_eq!(CASES.lines().count(), 12);
    assert_eq!(
        source_cases::cases(CASES)
            .iter()
            .map(|case| case.records.len())
            .sum::<usize>(),
        91
    );
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for raw priority reporting"]
fn conditional_pools_match_fresh_clingo() {
    priority_contracts::fresh(CASES);
}

#[test]
fn analysis_projection_preserves_the_original_subject() {
    for case in source_cases::cases(CASES) {
        let original = source_records::admit(
            case.source.split(":~").next().unwrap(),
            &FormulaLimits::default(),
        )
        .unwrap();
        let observed = source_records::admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            observed.analysis_basis(),
            AnalysisBasis::DependencyProjection
        );
        assert_eq!(original.atoms(), observed.atoms(), "{}", case.name);
        assert_eq!(
            original.theory().nodes(),
            observed.theory().nodes(),
            "{}",
            case.name
        );
        assert_eq!(
            original.theory().roots(),
            observed.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            original.formula_origins(),
            observed.formula_origins(),
            "{}",
            case.name
        );
    }
}

#[test]
fn analysis_projection_retains_complete_weak_fields() {
    use themelios_base::source::{Source, SourceId};
    use themelios_program::raise::raise;
    use themelios_syntax::{dialect::Dialect, parse::parse};
    for case in source_cases::cases(CASES) {
        let source = Source::new(SourceId::new(1), case.source.clone()).unwrap();
        let parsed = parse(&source, Dialect::Clingo);
        let raised = raise(&parsed);
        let original: Vec<_> = raised
            .program()
            .statements()
            .filter_map(|node| match node.get() {
                Statement::WeakConstraint(weak) => Some(weak),
                _ => None,
            })
            .collect();
        let input = source_records::admit(&case.source, &FormulaLimits::default()).unwrap();
        let projected: Vec<_> = input
            .analyzed_program()
            .statements()
            .filter_map(|node| match node.get() {
                Statement::WeakConstraint(weak) => Some(weak),
                _ => None,
            })
            .collect();
        for original in &original {
            assert!(
                projected
                    .iter()
                    .any(|projected| original.weight() == projected.weight()
                        && original.terms().eq(projected.terms())),
                "{}",
                case.name
            );
        }
        for projected in projected {
            assert!(
                original
                    .iter()
                    .any(|original| original.weight() == projected.weight()
                        && original.terms().eq(projected.terms())),
                "{}",
                case.name
            );
            for element in projected.body().get().elements() {
                if let BodyElement::Conditional(conditional) = element.get()
                    && let LiteralInner::Atom(atom) = &conditional.literal.inner
                {
                    assert!(matches!(atom.get().arguments, Arguments::Single(_)));
                }
            }
        }
    }
}

#[test]
fn projected_analysis_keeps_parsed_objective_evidence() {
    let source = "d(1;2).p(1).p(3).:~p(X;X+1):d(X).[1]";
    let input = source_records::admit(source, &FormulaLimits::default()).unwrap();
    let declarations = input.objective_declarations();
    assert_eq!(declarations.len(), 1);
    assert_eq!(
        input.source().slice(declarations[0].span).unwrap(),
        ":~p(X;X+1):d(X).[1]"
    );
    assert!(
        input
            .objective_origins()
            .iter()
            .flatten()
            .any(|location| { input.source().slice(location.span).unwrap() == "p(X;X+1):d(X)" })
    );
}

#[test]
fn pooled_local_variables_cannot_bind_outer_fields() {
    for fields in ["X", "1@X", "1,X"] {
        let source = format!("d(1;2).p(1).p(3).:~p(X;X+1):d(X).[{fields}]");
        let error = source_records::admit(&source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn ignored_pooled_rows_still_validate_required_arithmetic() {
    for fields in ["1", "symbol", "0", "1@symbol"] {
        let source = format!("d(0).p(0).:~p(X;1/X):d(X).[{fields}]");
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        let error = source_records::admit(&source, &limits).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Evaluation { .. })
            ),
            "{error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn projected_analysis_has_an_inclusive_node_limit() {
    let source = "d(1;2).p(1).p(3).:~p(X;X+1):d(X).[1]";
    let attempt = |maximum| {
        source_records::admit(
            source,
            &FormulaLimits {
                max_analysis_nodes: maximum,
                ..FormulaLimits::default()
            },
        )
    };
    let (mut lower, mut upper) = (0, 65536);
    assert!(attempt(upper).is_ok());
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if attempt(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    assert_eq!(
        source_records::exhaustive(&attempt(upper).unwrap()),
        source_cases::cases(CASES)[0].records
    );
    let failure = attempt(upper - 1).unwrap_err();
    assert!(
        matches!(failure, FormulaFailure::Limit {
        resource: FormulaResource::AnalysisNodes, observed, limit, ..
    } if observed == upper as u128 && limit == (upper - 1) as u128),
        "{failure}"
    );
    assert!(!failure.diagnostics().is_empty());
}

#[test]
fn finite_objective_occurrences_preserve_complete_scored_families() {
    for (source, expanded) in [
        (
            "{p}.:~#count{1:p}=(0;1).[(1;2)]",
            "{p}.:~#count{1:p}=0.[1]:~#count{1:p}=1.[1]:~#count{1:p}=0.[2]:~#count{1:p}=1.[2]",
        ),
        (
            "{p}.:~#count{1:p}=(0..1)+0.[1]",
            "{p}.:~#count{1:p}=0.[1]:~#count{1:p}=1.[1]",
        ),
        (
            "d(1;2).p(1).:~d(X),p(X;X+1).[X@X,X]",
            "d(1;2).p(1).:~d(X),p(X).[X@X,X]:~d(X),p(X+1).[X@X,X]",
        ),
        (
            "p(1).:~X=(1;2),p(X).[X@X,X]",
            "p(1).:~X=1,p(X).[X@X,X]:~X=2,p(X).[X@X,X]",
        ),
        (
            "d(1;2).e(1;2;3).p(1).p(3).:~p(X):d(X),e(X;X+1).[1]",
            "d(1;2).e(1;2;3).p(1).p(3).:~p(X):d(X),e(X);p(X):d(X),e(X+1).[1]",
        ),
        (
            "p(1).:~#count{(1;2):p(1)}>0.[1]",
            "p(1).:~#count{1:p(1);2:p(1)}>0.[1]",
        ),
        (
            "p(1).:~#count{1:p(1;2)}>0.[1]",
            "p(1).:~#count{1:p(1);1:p(2)}>0.[1]",
        ),
        (
            "p(1).#minimize{1:p(1;2)}.",
            "p(1).#minimize{1:p(1);1:p(2)}.",
        ),
        ("p(1).:~p(1).[(1;2)]", "p(1).:~p(1).[1]:~p(1).[2]"),
        ("{p}.:~p.[1@(1;2)]", "{p}.:~p.[1@1]:~p.[1@2]"),
        ("{p}.:~p.[(1;1),f((2;3))]", "{p}.:~p.[1,f(2)]:~p.[1,f(3)]"),
        ("{p}.:~p.[1,f(1..2)]", "{p}.:~p.[1,f(1)]:~p.[1,f(2)]"),
        ("{p}.:~p.[1..2]", "{p}.:~p.[1]:~p.[2]"),
        (
            "{p(f(1));p(f(2))}.:~p(f((1;2))).[(1;2)]",
            "{p(f(1));p(f(2))}.:~p(f(1)).[1]:~p(f(1)).[2]:~p(f(2)).[1]:~p(f(2)).[2]",
        ),
    ] {
        let original = source_records::admit(source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let expanded = source_records::admit(expanded, &FormulaLimits::default()).unwrap();
        let actual = source_records::exhaustive(&original);
        assert!(!actual.is_empty(), "nonempty finite family: {source}");
        assert_eq!(actual, source_records::exhaustive(&expanded), "{source}");
        assert_eq!(original.source().text(), source);
    }
}

#[test]
fn local_pool_alternatives_do_not_borrow_another_alternatives_binder() {
    let source = "d(1;2).p(1).p(3).:~p(X):d(X;X+1).[1]";
    let error = source_records::admit(source, &FormulaLimits::default()).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::UnboundArgumentInput { .. }),
        "{error}"
    );
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn projected_source_payload_has_independent_limits() {
    use zetesis_themelios::{
        AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, admit_formula,
    };
    let source = "d(1;2).p(1).p(3).:~p(X;X+1):d(X).[1]";
    for resource in [
        ExpansionResource::TermWork,
        ExpansionResource::ScalarBytes,
        ExpansionResource::Values,
        ExpansionResource::Origins,
    ] {
        let attempt = |maximum| {
            let mut limits = ExpansionLimits::default();
            match resource {
                ExpansionResource::TermWork => limits.max_term_work = maximum,
                ExpansionResource::ScalarBytes => limits.max_scalar_bytes = maximum,
                ExpansionResource::Values => limits.max_values = maximum,
                ExpansionResource::Origins => limits.max_origin_locations = maximum,
                _ => unreachable!("selected source resources"),
            }
            admit_formula(
                source.into(),
                AdmissionOptions::default(),
                limits,
                FormulaLimits::default(),
            )
        };
        let (mut lower, mut upper) = (0, 65536);
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        assert_eq!(
            source_records::exhaustive(&attempt(upper).unwrap()),
            source_cases::cases(CASES)[0].records
        );
        let failure = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(failure, FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: actual, location, ..
        }) if actual == resource && !location.span.is_empty()),
            "{failure}"
        );
    }
}
