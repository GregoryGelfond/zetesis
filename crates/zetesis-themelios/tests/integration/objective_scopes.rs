//! Scoped objectives observe the original model through shared formula operations.

use crate::support::priority_contracts;
use crate::support::source_cases;
use zetesis_reference_support as reference;
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = include_str!("../fixtures/objective-scopes.jsonl");

#[test]
fn scoped_objectives_preserve_full_scored_answers() {
    assert_eq!(CASES.lines().count(), 39);
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires clingo: scoped objectives match fresh raw clingo"]
fn scoped_objectives_match_fresh_raw_clingo() {
    priority_contracts::fresh(CASES);
}

#[test]
fn scoped_objectives_keep_the_original_reduct_subject() {
    for case in source_cases::cases(CASES) {
        let program = case
            .source
            .split(":~")
            .next()
            .unwrap()
            .split("#minimize")
            .next()
            .unwrap()
            .split("#maximize")
            .next()
            .unwrap();
        let original = reference::admit(program, &FormulaLimits::default()).unwrap();
        let observed = reference::admit(&case.source, &FormulaLimits::default()).unwrap();
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
fn scoped_local_variables_cannot_bind_objective_fields() {
    for source in [
        "p(1).:~#count{X:p(X)}>0.[X]",
        "d(1).q(1).:~q(X):d(X).[1@X]",
        "d(1).q(1).:~q(X):d(X).[1,X]",
    ] {
        let error = reference::admit(source, &FormulaLimits::default()).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn ignored_scoped_rows_retain_no_query_nodes() {
    for field in ["symbol", "1@symbol", "#inf", "1@#sup"] {
        let source = format!("{{a}}.:~#count{{1:a}}>0.[{field}]");
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        let input = reference::admit(&source, &limits).unwrap();
        assert!(input.objectives().templates().is_empty());
        let records = reference::exhaustive(&input);
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|(_, costs)| costs.is_none()));
    }
}

#[test]
fn scoped_numeric_zero_retains_its_query() {
    let mut limits = FormulaLimits::default();
    limits.objective.max_condition_nodes = 0;
    let source = "{a}.:~#count{1:a}>0.[0]";
    assert!(matches!(
        reference::admit(source, &limits).unwrap_err(),
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::Limit {
                resource: zetesis_objective::AdmissionResource::ConditionNodes,
                ..
            },
            ..
        }
    ));
    let input = reference::admit(source, &FormulaLimits::default()).unwrap();
    assert_eq!(input.objectives().templates().len(), 1);
    assert!(
        reference::exhaustive(&input)
            .iter()
            .all(|(_, costs)| *costs == Some(vec![0]))
    );
}

#[test]
fn scoped_validation_precedes_numeric_selection() {
    for field in ["symbol", "1@symbol", "0"] {
        let source = format!("d(0).{{a}}.:~d(X),#count{{1:a}}>1/X.[{field}]");
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        let error = reference::admit(&source, &limits).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(zetesis_themelios::ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
    }
}

#[test]
fn scoped_validation_precedes_activity_exclusion() {
    for body in ["d(X),not a,#count{}>1/X", "d(X),#count{}>1/X,not a"] {
        for field in ["1", "symbol", "1@symbol", "0"] {
            let source = format!("d(0).a.:~{body}.[{field}]");
            let mut limits = FormulaLimits::default();
            limits.objective.max_condition_nodes = 0;
            let error = reference::admit(&source, &limits).unwrap_err();
            assert!(
                matches!(
                    error,
                    FormulaFailure::Expansion(
                        zetesis_themelios::ExpansionFailure::Evaluation { .. }
                    )
                ),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn absent_scoped_rows_retain_no_query_nodes() {
    for field in ["1", "symbol", "1@symbol", "0"] {
        let source = format!("d(1).a.:~d(X),not a,#count{{}}>1/X.[{field}]");
        let mut limits = FormulaLimits::default();
        limits.objective.max_condition_nodes = 0;
        let input = reference::admit(&source, &limits).unwrap();
        assert!(input.objectives().templates().is_empty());
        limits.max_objective_formula_nodes = 0;
        assert!(matches!(
            reference::admit(&source, &limits).unwrap_err(),
            FormulaFailure::Limit {
                resource: FormulaResource::ObjectiveFormulaNodes,
                ..
            }
        ));
    }
}

#[test]
fn scoped_formula_limits_do_not_change_the_theory_cap() {
    let base = "{a}.";
    let original = reference::admit(base, &FormulaLimits::default()).unwrap();
    let mut limits = FormulaLimits::default();
    limits.theory.max_atoms = original.atoms().len();
    limits.theory.max_nodes = original.theory().nodes().len();
    let source = format!("{base}:~N=#sum{{1:a;2:a;4:a;8:a}}.[N]");
    let input = reference::admit(&source, &limits).unwrap();
    assert_eq!(input.atoms(), original.atoms());
    assert_eq!(input.theory().nodes(), original.theory().nodes());
    assert_eq!(reference::exhaustive(&input).len(), 2);
}

#[test]
fn scoped_formula_limits_are_inclusive() {
    let source = "d(1;2).{q(1);q(2)}.:~q(X):d(X).[1]";
    let expected =
        reference::exhaustive(&reference::admit(source, &FormulaLimits::default()).unwrap());
    for resource in [
        FormulaResource::ObjectiveFormulaAtoms,
        FormulaResource::ObjectiveFormulaNodes,
        FormulaResource::Work,
        FormulaResource::Substitutions,
    ] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::ObjectiveFormulaAtoms => {
                    limits.max_objective_formula_atoms = maximum;
                }
                FormulaResource::ObjectiveFormulaNodes => {
                    limits.max_objective_formula_nodes = maximum;
                }
                FormulaResource::Work => limits.max_work = maximum as u64,
                FormulaResource::Substitutions => limits.max_substitutions = maximum as u64,
                _ => unreachable!("selected scratch resources"),
            }
            reference::admit(source, &limits)
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
        assert_eq!(reference::exhaustive(&attempt(upper).unwrap()), expected);
        let failure = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(failure, FormulaFailure::Limit { resource: actual, observed, limit, .. }
            if actual == resource && observed == upper as u128 && limit == (upper - 1) as u128),
            "{resource:?}: {failure}"
        );
        assert!(!failure.diagnostics().is_empty());
    }
}

#[test]
fn scoped_aggregate_nodes_keep_typed_refusals() {
    let source = "{a;b}.:~N=#count{1:a;2:b}.[N@N]";
    let limits = FormulaLimits {
        max_objective_formula_nodes: 5,
        ..FormulaLimits::default()
    };
    let error = reference::admit(source, &limits).unwrap_err();
    assert!(matches!(error, FormulaFailure::Aggregate { error, .. }
        if error.kind() == zetesis_ferraris::AggregateErrorKind::NodeLimit));
    assert!(!error.diagnostics().is_empty());
}

#[test]
fn scoped_binders_retain_source_shape_limits() {
    let source = ":~X=1.[X,X]";
    for resource in [
        zetesis_objective::AdmissionResource::TupleWidth,
        zetesis_objective::AdmissionResource::Filters,
        zetesis_objective::AdmissionResource::Variables,
    ] {
        let mut limits = FormulaLimits::default();
        match resource {
            zetesis_objective::AdmissionResource::TupleWidth => {
                limits.objective.max_tuple_width = 0;
            }
            zetesis_objective::AdmissionResource::Filters => limits.objective.max_filters = 0,
            zetesis_objective::AdmissionResource::Variables => {
                limits.objective.max_variables_per_template = 0;
            }
            _ => unreachable!("selected source shape limits"),
        }
        let error = reference::admit(source, &limits).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Objective { error: zetesis_objective::AdmissionError::Limit {
            resource: actual, actual: 1, limit: 0, .. }, .. } if actual == resource),
            "{resource:?}: {error}"
        );
    }
}

#[test]
fn scoped_declarations_retain_original_analysis_nodes() {
    use themelios_program::program::Statement;
    let source = "{a}. :~#count{1:a}>0.[1]";
    let input = reference::admit(source, &FormulaLimits::default()).unwrap();
    let statements: Vec<_> = input.analyzed_program().statements().collect();
    assert_eq!(statements.len(), 2);
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.get(), Statement::Rule(_)))
            .count(),
        1
    );
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.get(), Statement::WeakConstraint(_)))
            .count(),
        1
    );
    assert_eq!(input.objective_declarations().len(), 1);
    assert!(
        input
            .source()
            .expect("source input")
            .slice(
                input.objective_declarations()[0]
                    .location()
                    .expect("parsed source")
                    .span
            )
            .unwrap()
            .starts_with(":~")
    );
    assert!(input.objective_origins().iter().flatten().any(|origin| {
        input
            .source()
            .expect("source input")
            .slice(origin.location().expect("parsed source").span)
            .unwrap()
            == "#count{1:a}>0"
    }));
}
