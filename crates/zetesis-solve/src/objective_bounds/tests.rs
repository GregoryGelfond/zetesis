use super::*;

mod observer_contract_tests;
mod contract_tests;

/// The clause search of `theory`'s stable models.
fn by_clauses(
    theory: &zetesis_ferraris::Theory,
    limits: zetesis_sat::Limits,
    cancellation: zetesis_sat::Cancellation,
) -> Result<zetesis_sat::StableModels, zetesis_sat::Incomplete> {
    zetesis_sat::StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Clauses,
        limits,
        cancellation,
    )
}
