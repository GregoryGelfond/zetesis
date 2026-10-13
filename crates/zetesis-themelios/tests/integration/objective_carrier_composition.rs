//! Independent observers preserve previously established source carriers.

use crate::support::limits;
use crate::support::priority_contracts;
use zetesis_test_support::thresholds::first_success;

use zetesis_reference_support as reference;
use zetesis_themelios::{FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &str = include_str!("../fixtures/objective-carrier-composition.jsonl");

#[test]
fn independent_observers_preserve_precise_carriers() {
    priority_contracts::check(CASES);
}

#[test]
#[ignore = "requires clingo: composed carriers match fresh raw clingo"]
fn composed_carriers_match_fresh_raw_clingo() {
    priority_contracts::fresh(CASES);
}

#[test]
fn composed_carrier_limits_are_inclusive() {
    let source =
        "n(N):-N=#max{1;word}.c(X):-n(X).p(N):-n(N),N=1.#minimize{N:n(N);N@2:c(N);1@3:p(N)}.";
    let expected =
        reference::exhaustive(&reference::admit(source, &FormulaLimits::default()).unwrap());
    for resource in [
        FormulaResource::ObjectivePresenceEntries,
        FormulaResource::Work,
    ] {
        let attempt = |maximum| reference::admit(source, &limits::formula(resource, maximum));
        let upper = first_success(limits::formula_default(resource), |middle| {
            attempt(middle).is_ok()
        });
        let input = attempt(upper).unwrap();
        // The fixed symbolic maximum excludes n(1), hence every p(N).
        // Carrier preparation still runs under both inclusive ceilings even
        // though no numeric objective row survives source activity.
        assert!(input.objectives().priorities().is_empty());
        assert_eq!(reference::exhaustive(&input), expected);
        let failure = attempt(upper - 1).unwrap_err();
        assert!(
            matches!(failure,
                FormulaFailure::Limit { resource: actual, observed, limit, .. }
                if actual == resource && observed == u128::from(upper) && limit == u128::from(upper - 1)
            ),
            "{resource:?}: {failure}"
        );
        assert!(!failure.diagnostics().is_empty());
    }
}
