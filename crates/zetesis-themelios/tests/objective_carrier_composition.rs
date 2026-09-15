//! Independent observers preserve previously established source carriers.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;
#[path = "support/priority_contracts.rs"]
mod priority_contracts;

use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = include_str!("fixtures/objective-carrier-composition.jsonl");

#[test]
fn independent_observers_preserve_precise_carriers() {
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires independent clingo 5.8.2 for raw priority reporting"]
fn composed_carriers_match_fresh_raw_clingo() {
    priority_contracts::fresh(CASES);
}

#[test]
fn composed_carrier_limits_are_inclusive() {
    let source =
        "n(N):-N=#max{1;word}.c(X):-n(X).p(N):-n(N),N=1.#minimize{N:n(N);N@2:c(N);1@3:p(N)}.";
    let expected = source_records::exhaustive(
        &source_records::admit(source, &FormulaLimits::default()).unwrap(),
    );
    for resource in [
        FormulaResource::ObjectivePresenceEntries,
        FormulaResource::Work,
    ] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            match resource {
                FormulaResource::ObjectivePresenceEntries => {
                    limits.max_objective_presence_entries = maximum;
                }
                FormulaResource::Work => limits.max_work = maximum as u64,
                _ => unreachable!("selected carrier resources"),
            }
            source_records::admit(source, &limits)
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
        let input = attempt(upper).unwrap();
        // The fixed symbolic maximum excludes n(1), hence every p(N).
        // Carrier preparation still runs under both inclusive ceilings even
        // though no numeric objective row survives source activity.
        assert!(input.objectives().priorities().is_empty());
        assert_eq!(source_records::exhaustive(&input), expected);
        let failure = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(failure,
                FormulaFailure::Limit { resource: actual, observed, limit, .. }
                if actual == resource && observed == upper as u128 && limit == (upper - 1) as u128
            ),
            "{resource:?}: {failure}"
        );
        assert!(!failure.diagnostics().is_empty());
    }
}
