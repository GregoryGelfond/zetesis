//! Source priority carriers are distinct from realized aggregate values.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use source_records::{admit, exhaustive};
use std::collections::BTreeSet;

use proptest::prelude::*;
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = r#"{"name":"optional_count","source":"{a}.n(N):-N=#count{1:a}.#minimize{1@N:n(N)}.","priorities":[1,0],"records":[[["n(0)"],[0,1]],[["a","n(1)"],[1,0]]]}
{"name":"shared_sum","source":"{a}.n(N):-N=#sum{2:a;3:a}.#minimize{1@N:n(N)}.","priorities":[5,3,2,0],"records":[[["n(0)"],[0,0,0,1]],[["a","n(5)"],[1,0,0,0]]]}
{"name":"same_row","source":"{a}.n(N):-N=#sum{2:a;3:a}.#minimize{N@N:n(N)}.","priorities":[5,3,2,0],"records":[[["n(0)"],[0,0,0,0]],[["a","n(5)"],[5,0,0,0]]]}
{"name":"shared_count","source":"{a}.n(N):-N=#count{1,k:a;1,l:a}.#minimize{1@N:n(N)}.","priorities":[2,1,0],"records":[[["n(0)"],[0,0,1]],[["a","n(2)"],[1,0,0]]]}
{"name":"required_sum","source":"a.{b}.n(N):-N=#sum{2:a;3:b}.#minimize{1@N:n(N)}.","priorities":[5,2],"records":[[["a","n(2)"],[0,1]],[["a","b","n(5)"],[1,0]]]}
{"name":"required_alias","source":"a.{b}.n(N):-N=#sum{2:a;2:b;3:b}.#minimize{1@N:n(N)}.","priorities":[5,2],"records":[[["a","n(2)"],[0,1]],[["a","b","n(5)"],[1,0]]]}
{"name":"mixed_maximum","source":"{a}.n(N):-N=#max{2:a;foo:a}.#minimize{1@N:n(N)}.","priorities":[2],"records":[[["n(#inf)"],[0]],[["a","n(foo)"],[0]]]}
{"name":"shared_minimum","source":"{a}.n(N):-N=#min{2:a;3:a}.#minimize{1@N:n(N)}.","priorities":[3,2],"records":[[["n(#sup)"],[0,0]],[["a","n(2)"],[0,1]]]}
{"name":"changing_maximum","source":"a.{b}.n(N):-N=#max{2:a;3:b}.#maximize{1@N:n(N)}.","priorities":[3,2],"records":[[["a","n(2)"],[0,-1]],[["a","b","n(3)"],[-1,0]]]}
{"name":"forwarded_count","source":"{a}.n(N):-N=#count{1:a}.rank(P):-n(P).:~rank(P).[1@P]","priorities":[1,0],"records":[[["n(0)","rank(0)"],[0,1]],[["a","n(1)","rank(1)"],[1,0]]]}
{"name":"excluded_choices","source":"{a}.:-a.n(N):-N=#sum{2:a;3:a}.#minimize{1@N:n(N)}.","priorities":[5,3,2,0],"records":[[["n(0)"],[0,0,0,1]]]}
{"name":"signed_sum","source":"a.{b}.n(N):-N=#sum{2:a;-3:b}.#minimize{1@N:n(N)}.","priorities":[2,-1],"records":[[["a","n(2)"],[1,0]],[["a","b","n(-1)"],[0,1]]]}
{"name":"sum_plus","source":"a.{b}.n(N):-N=#sum+{2:a;-3:b;4:b}.#minimize{1@N:n(N)}.","priorities":[6,2],"records":[[["a","n(2)"],[0,1]],[["a","b","n(6)"],[1,0]]]}
{"name":"shared_product","source":"{a}.n(N):-N=#count{1:a}.m(P):-P=#sum{2:a}.#minimize{1@(N+P),k:n(N),m(P)}.","priorities":[3,2,1,0],"records":[[["n(0)","m(0)"],[0,0,0,1]],[["a","n(1)","m(2)"],[1,0,0,0]]]}
{"name":"independent_product","source":"{a;b}.n(N):-N=#count{1:a}.m(P):-P=#sum{2:b}.#minimize{1@(N+P),k:n(N),m(P)}.","priorities":[3,2,1,0],"records":[[["n(0)","m(0)"],[0,0,0,1]],[["a","n(1)","m(0)"],[0,0,1,0]],[["b","n(0)","m(2)"],[0,1,0,0]],[["a","b","n(1)","m(2)"],[1,0,0,0]]]}
{"name":"aliased_product","source":"{a}.n(N):-N=#count{1:a}.alias(P):-n(P).#minimize{1@(N+P),k:n(N),alias(P)}.","priorities":[2,1,0],"records":[[["n(0)","alias(0)"],[0,0,1]],[["a","n(1)","alias(1)"],[1,0,0]]]}
{"name":"numeric_field_product","source":"{a}.n(N):-N=#max{1:a;word:a}.m(P):-P=#count{1:a}.#minimize{N@P,k:n(N),m(P)}.","priorities":[1,0],"records":[[["n(#inf)","m(0)"],[0,0]],[["a","n(word)","m(1)"],[0,0]]]}
{"name":"shared_certificate","source":"{a}.n(N):-N=#count{1:a}.alias(P):-n(P).#minimize{1@N,k:n(N);2@P,l:alias(P)}.","priorities":[1,0],"records":[[["n(0)","alias(0)"],[0,3]],[["a","n(1)","alias(1)"],[3,0]]]}
{"name":"aliased_objective_key","source":"{a}.n(N):-N=#count{1:a}.alias(P):-n(P).#minimize{1@N,k:n(N);1@P,k:alias(P)}.","priorities":[1,0],"records":[[["n(0)","alias(0)"],[0,1]],[["a","n(1)","alias(1)"],[1,0]]]}
{"name":"source_permutation","source":"m(P):-P=#sum{2:a}.n(N):-N=#count{1:a}.{a}.#minimize{1@(P+N),k:m(P),n(N)}.","priorities":[3,2,1,0],"records":[[["n(0)","m(0)"],[0,0,0,1]],[["a","n(1)","m(2)"],[1,0,0,0]]]}
"#;

#[test]
fn source_carriers_preserve_complete_scored_answers() {
    for (case, row) in source_cases::cases(CASES.trim())
        .into_iter()
        .zip(CASES.lines())
    {
        let expected: serde_json::Value = serde_json::from_str(row).unwrap();
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            expected["priorities"],
            "{}",
            case.name
        );
    }
}

#[test]
#[ignore = "requires an independently installed clingo"]
fn original_sources_match_complete_clingo_records() {
    for case in source_cases::cases(CASES.trim()) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn source_measures_match_complete_key_subsets(
        function in 0..5_usize,
        weights in prop::array::uniform3(-2..4_i32),
    ) {
        let name = ["count", "sum", "sum+", "min", "max"][function];
        let source = format!("r.{{a}}.n(N):-N=#{name}{{{},k0:r;{},k1:a;{},k2:a}}.#minimize{{1@N:n(N)}}.", weights[0], weights[1], weights[2]);
        // Enumerate key subsets, independently of the production subset-sum
        // reducer. The required first key is present in every source subset.
        let measure = |mask: usize| {
            let selected: Vec<_> = weights.iter().enumerate()
                .filter(|(index, _)| *index == 0 || mask & (1 << (index - 1)) != 0)
                .map(|(_, &weight)| weight).collect();
            match function {
                0 => i32::try_from(selected.len()).unwrap(),
                1 => selected.iter().sum(),
                2 => selected.iter().filter(|&&weight| weight > 0).sum(),
                3 => *selected.iter().min().unwrap(),
                4 => *selected.iter().max().unwrap(),
                _ => unreachable!(),
            }
        };
        let priorities: Vec<_> = (0..4).map(measure).collect::<BTreeSet<_>>().into_iter().rev().collect();
        let expected = [false, true].into_iter().map(|chosen| {
            // Both optional keys share a: only subsets 0 and 3 realize.
            let value = measure(if chosen { 3 } else { 0 });
            let mut atoms = BTreeSet::from(["r".into(), format!("n({value})")]);
            if chosen { atoms.insert("a".into()); }
            let costs = priorities.iter().map(|&priority| i64::from(priority == value)).collect();
            (atoms, Some(costs))
        }).collect();
        let input = admit(&source, &FormulaLimits::default()).unwrap();
        prop_assert_eq!(input.objectives().priorities(), priorities);
        prop_assert_eq!(exhaustive(&input), expected);
    }
}

#[test]
fn source_carriers_preserve_original_equalities() {
    for case in source_cases::cases(CASES.trim()) {
        let Some((theory, _)) = case.source.split_once("#minimize") else {
            continue;
        };
        let source = admit(theory, &FormulaLimits::default()).unwrap();
        let observed = admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(observed.atoms(), source.atoms(), "{}", case.name);
        assert_eq!(
            observed.theory().nodes(),
            source.theory().nodes(),
            "{}",
            case.name
        );
        assert_eq!(
            observed.theory().roots(),
            source.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            observed.formula_origins(),
            source.formula_origins(),
            "{}",
            case.name
        );
    }
}

#[test]
fn mixed_source_rows_retain_warnings_and_complete_costs() {
    let source = "{a}.n(N):-N=#sum{2:a;3:a}.#minimize{1@(1/(N-2)):n(N)}.";
    let input = admit(source, &FormulaLimits::default()).unwrap();
    let expected: source_records::Records = BTreeSet::from([
        (BTreeSet::from(["n(0)".into()]), Some(vec![0, 1])),
        (
            BTreeSet::from(["a".into(), "n(5)".into()]),
            Some(vec![0, 1]),
        ),
    ]);
    assert_eq!(exhaustive(&input), expected);
    assert_eq!(input.objectives().priorities(), [1, 0]);
    let [warning] = input.warnings() else {
        panic!("one omitted source-carrier instance");
    };
    assert!(
        input
            .source()
            .slice(warning.location().span)
            .unwrap()
            .contains("#minimize")
    );
    let guarded = admit(
        "{a}.n(N):-N=#sum{2:a;3:a}.#minimize{1@(1/(N-2)):n(N),N!=2}.",
        &FormulaLimits::default(),
    )
    .unwrap();
    assert!(guarded.warnings().is_empty());
    assert_eq!(exhaustive(&input), exhaustive(&guarded));
}

#[test]
fn source_carriers_bound_retained_values() {
    // Request + two cone predicates + one transported predicate + two possible
    // tuple keys + two weights + four source values reach this reservation.
    const ENTRIES: usize = 12;
    let source = "{a}.n(N):-N=#sum{2:a;3:a}.#minimize{1@N:n(N)}.";
    let mut limits = FormulaLimits {
        max_objective_presence_entries: ENTRIES - 1,
        ..FormulaLimits::default()
    };
    let error = admit(source, &limits).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::ObjectivePresenceEntries, observed, .. } if observed == ENTRIES as u128)
    );
    assert!(!error.diagnostics().is_empty());
    limits.max_objective_presence_entries = ENTRIES;
    assert!(admit(source, &limits).is_ok());
}

#[test]
fn source_products_share_the_certificate_budget() {
    const ENTRIES: usize = 14;
    let source = "{a}.n(N):-N=#count{1:a}.m(P):-P=#sum{2:a}.#minimize{1@(N+P):n(N),m(P)}.";
    let mut limits = FormulaLimits {
        max_objective_presence_entries: ENTRIES - 1,
        ..FormulaLimits::default()
    };
    let error = admit(source, &limits).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::ObjectivePresenceEntries, observed, .. } if observed == ENTRIES as u128),
        "{error}"
    );
    limits.max_objective_presence_entries = ENTRIES;
    assert!(admit(source, &limits).is_ok());
}

#[test]
fn unrealized_values_consume_assignment_capacity() {
    const SOURCE_VALUES: usize = 4;
    let source = "{a}.n(N):-N=#sum{2:a;3:a}.#minimize{1@N:n(N)}.";
    let mut limits = FormulaLimits {
        max_assignment_values: SOURCE_VALUES - 1,
        ..FormulaLimits::default()
    };
    let error = admit(source, &limits).unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::AssignmentValues, observed, .. } if observed == SOURCE_VALUES as u128)
    );
    limits.max_assignment_values = SOURCE_VALUES;
    assert!(admit(source, &limits).is_ok());
}

#[test]
fn source_products_bound_specialized_objectives() {
    const SOURCE_ROWS: usize = 4;
    let source = "{a}.n(N):-N=#count{1:a}.m(P):-P=#sum{2:a}.#minimize{1@(N+P):n(N),m(P)}.";
    let mut limits = FormulaLimits::default();
    limits.objective.max_templates = SOURCE_ROWS - 1;
    let error = admit(source, &limits).unwrap_err();
    let FormulaFailure::Limit {
        resource: FormulaResource::ObjectiveElements,
        observed,
        location,
        ..
    } = &error
    else {
        panic!("{error}");
    };
    assert_eq!(*observed, SOURCE_ROWS as u128);
    let start = usize::try_from(location.span.start().get()).unwrap();
    let end = usize::try_from(location.span.end().get()).unwrap();
    assert_eq!(&source[start..end], "#minimize{1@(N+P):n(N),m(P)}.");
    limits.objective.max_templates = SOURCE_ROWS;
    assert!(admit(source, &limits).is_ok());
}

#[test]
fn selected_carriers_preserve_complete_scored_answers() {
    let rows = include_str!("fixtures/objective-filtered-carriers.jsonl");
    let cases = source_cases::cases(rows);
    assert_eq!(cases.len(), 12);
    for (case, row) in cases.into_iter().zip(rows.lines()) {
        let expected: serde_json::Value = serde_json::from_str(row).unwrap();
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(exhaustive(&input), case.records, "{}", case.name);
        assert_eq!(
            serde_json::to_value(input.objectives().priorities()).unwrap(),
            expected["priorities"],
            "{}",
            case.name
        );
    }
}

#[test]
fn selected_carriers_preserve_original_equalities() {
    for case in source_cases::cases(include_str!("fixtures/objective-filtered-carriers.jsonl")) {
        let program = case.source.split("#minimize").next().unwrap();
        let original = admit(program, &FormulaLimits::default()).unwrap();
        let selected = admit(&case.source, &FormulaLimits::default()).unwrap();
        assert_eq!(original.atoms(), selected.atoms(), "{}", case.name);
        assert_eq!(
            original.theory().nodes(),
            selected.theory().nodes(),
            "{}",
            case.name
        );
        assert_eq!(
            original.theory().roots(),
            selected.theory().roots(),
            "{}",
            case.name
        );
        assert_eq!(
            original.formula_origins(),
            selected.formula_origins(),
            "{}",
            case.name
        );
    }
}

#[test]
#[ignore = "requires independent clingo for 12 original filtered-carrier sources"]
fn selected_source_carriers_match_fresh_clingo() {
    for case in source_cases::cases(include_str!("fixtures/objective-filtered-carriers.jsonl")) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}

#[test]
fn completed_carriers_respect_grounding_work_limits() {
    const SEARCH_BOUND: u64 = 65_536;
    let case = source_cases::cases(CASES.trim())
        .into_iter()
        .find(|case| case.name == "shared_product")
        .unwrap();
    for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::Work => limits.max_work = maximum,
                FormulaResource::Substitutions => limits.max_substitutions = maximum,
                _ => unreachable!("selected work resource"),
            }
            admit(&case.source, &limits)
        };
        let (mut lower, mut upper) = (0, SEARCH_BOUND);
        assert!(attempt(upper).is_ok());
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if attempt(middle).is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        assert_eq!(exhaustive(&attempt(upper).unwrap()), case.records);
        let error = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::Limit { resource: actual, observed, limit, .. } if actual == resource && observed == u128::from(upper) && limit == u128::from(upper - 1))
        );
        assert!(!error.diagnostics().is_empty());
    }
}
