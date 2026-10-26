use super::*;

mod observer_contract_tests;
mod contract_tests;

/// Independent choices whose objective bound indexes its 400 elements, more
/// work than the rest of the exact search performs.
const HEAVY_OBJECTIVE: &str = "{a;b}. #minimize{1,X:a,X=1..200; 1,X:b,X=1..200}.";

/// Limits whose work ceiling is exactly what an exact search of `theory`
/// charges: that search finds every original model, and no bound's charge
/// fits beside it.
fn exact_search_limits(theory: &zetesis_ferraris::Theory) -> zetesis_stable::Limits {
    let mut exact = StableModels::new(
        theory,
        zetesis_stable::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    for model in exact.by_ref() {
        model.unwrap();
    }
    assert!(exact.exhausted());
    let mut limits = zetesis_stable::Limits::default();
    limits.search.max_work = exact.statistics().search.work;
    limits
}

mod prepared_scoring;
mod choice_replacement;
