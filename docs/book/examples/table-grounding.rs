//! Compare the admitted program from indexed joins and reusable table joins.

// ANCHOR: example
use std::cell::Cell;

use zetesis_themelios::ProgramSite;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    GroundingObserver, GroundingOptions, GroundingOutcome, GroundingPhase, GroundingWork,
    JoinStrategy, prepare_formula,
};

const SOURCE: &str = "
    station(1;2;3).
    route(1,1,rail). route(1,2,rail). route(2,2,rail). route(3,3,road).
    {selected(X)} :- station(X), route(X,X,rail).
    eligible(X) :- station(X), route(X,X,rail).
";

#[derive(Default)]
struct JoinWork(Cell<GroundingWork>);

impl GroundingObserver for JoinWork {
    fn enter(&self) {}
    fn exit(&self) {}

    fn details_enabled(&self) -> bool {
        true
    }

    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(self.0.get().checked_sum(work));
    }
}

fn ground(
    joins: JoinStrategy,
    observer: Option<&dyn GroundingObserver>,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(
        SOURCE.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .with_grounding_options(GroundingOptions { joins })
    .ground_with_observer(observer)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let indexed = ground(JoinStrategy::Indexed, None)?;
    let observed = JoinWork::default();
    let table = ground(JoinStrategy::Table, Some(&observed))?;

    // Compare complete admitted atoms, formulas and their source locations.
    assert_eq!(table.atoms(), indexed.atoms());
    assert_eq!(table.theory().nodes(), indexed.theory().nodes());
    assert_eq!(table.theory().roots(), indexed.theory().roots());
    assert_eq!(table.formula_origins(), indexed.formula_origins());

    // Selecting a strategy is insufficient evidence that its index was used.
    let work = observed.0.get();
    assert!(work.table_preparations.is_some_and(|count| count > 0));
    assert!(work.table_probes.is_some_and(|count| count > 0));
    assert!(work.table_reuses.is_some_and(|count| count > 0));
    assert!(work.table_rows.is_some_and(|count| count > 0));
    Ok(())
}
// ANCHOR_END: example
