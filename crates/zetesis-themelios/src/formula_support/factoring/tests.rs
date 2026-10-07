use super::*;
use crate::formula_support::testing;
use crate::{ExpansionLimits, FormulaLimits, FormulaResource};

#[test]
fn continuation_owner_accounts_for_exact_storage() {
    testing::with_completed_source(
        "d(1;2). q(N) :- d(_), N=#count{1:d(1)}.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules
                .iter()
                .find(|rule| rule.bindings.is_some())
                .unwrap();
            let location = rule.location;
            let limits = FormulaLimits::default();
            let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
            let mut join =
                Join::rule(rule, support, computation, &limits, &mut budget, counters).unwrap();
            let before = counters.workspace_bytes();
            let observer = computation.lease();
            let base = limits.max_support_bytes
                - computation.allowance(&observer, &limits, location).unwrap();
            let admitted = base + size_of::<Continuations<'_, '_, '_>>();
            let short = FormulaLimits {
                max_support_bytes: admitted - 1,
                ..limits
            };
            assert!(matches!(
                Continuations::new(&mut join, rule, Context::new(computation, &short, counters, location)),
                Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. })
                    if observed == admitted as u128 && limit == (admitted - 1) as u128
            ));
            assert_eq!(counters.workspace_bytes(), before);
            let exact = FormulaLimits {
                max_support_bytes: admitted,
                ..limits
            };
            let rows = Continuations::new(
                &mut join,
                rule,
                Context::new(computation, &exact, counters, location),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                counters.workspace_bytes(),
                before + size_of::<Continuations<'_, '_, '_>>()
            );
            drop(rows);
            assert_eq!(counters.workspace_bytes(), before);
        },
    );
}
