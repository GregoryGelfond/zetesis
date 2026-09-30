//! Selected complete rows preserve source grounding, evidence and finite refusals.

use std::cell::Cell;

use themelios_base::span::Location;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, GroundingObserver, GroundingOptions, GroundingOutcome, GroundingPhase,
    GroundingWork, JoinStrategy, prepare_formula,
};

#[derive(Default)]
struct Observation {
    work: Cell<GroundingWork>,
    failed: Cell<bool>,
}
impl GroundingObserver for Observation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<Location>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.work.set(self.work.get().checked_sum(work));
        if outcome == GroundingOutcome::Failed {
            self.failed.set(true);
        }
    }
}

fn ground(
    source: &str,
    strategy: JoinStrategy,
    limits: &FormulaLimits,
    observer: &Observation,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        *limits,
    )?
    .with_grounding_options(GroundingOptions { joins: strategy })
    .ground_with_observer(Some(observer))
}

fn equal_theory(source: &str) -> GroundingWork {
    let indexed_work = Observation::default();
    let table_work = Observation::default();
    let indexed = ground(
        source,
        JoinStrategy::Indexed,
        &FormulaLimits::default(),
        &indexed_work,
    )
    .unwrap();
    let table = ground(
        source,
        JoinStrategy::Table,
        &FormulaLimits::default(),
        &table_work,
    )
    .unwrap();
    assert_eq!(table.atoms(), indexed.atoms());
    assert_eq!(table.theory().nodes(), indexed.theory().nodes());
    assert_eq!(table.theory().roots(), indexed.theory().roots());
    assert_eq!(table.formula_origins(), indexed.formula_origins());
    assert_eq!(
        table.objectives().templates().iter().collect::<Vec<_>>(),
        indexed.objectives().templates().iter().collect::<Vec<_>>()
    );
    assert_eq!(
        table.objectives().priorities(),
        indexed.objectives().priorities()
    );
    assert_eq!(table.objective_origins(), indexed.objective_origins());
    assert_eq!(
        table.objective_declarations(),
        indexed.objective_declarations()
    );
    assert_eq!(indexed_work.work.get().table_probes, Some(0));
    let work = table_work.work.get();
    assert!(
        work.table_probes.unwrap() > 0,
        "the complete theory comparison must execute table queries"
    );
    assert!(work.table_rows.unwrap() > 0);
    work
}

const SHARED: &str =
    "p(1;2;3).q(1,1,a).q(1,2,a).q(2,2,a).q(3,3,b).{r(X)}:-p(X),q(X,X,a).s(X):-p(X),q(X,X,a).";

#[test]
fn reused_tables_preserve_the_emitted_theory() {
    let work = equal_theory(SHARED);
    assert!(work.table_preparations.unwrap() > 0);
    assert!(work.table_reuses.unwrap() > 0);
    assert!(work.table_prepare_work.unwrap() > 0);
    assert!(work.table_query_work.unwrap() > 0);
    assert!(work.table_index_bytes.unwrap() > 0);
}

#[test]
fn typed_signed_rows_preserve_the_emitted_theory() {
    equal_theory(
        "d(1;\"1\";f(1);#inf;#sup). -q(1,1). -q(\"1\",\"1\"). -q(f(1),f(1)). -q(#inf,#sup). {r(X)}:-d(X),-q(X,X).",
    );
}

#[test]
fn local_join_queries_preserve_the_emitted_theory() {
    equal_theory(
        "d(1..3).p(1,1).p(1,2).p(2,2).{a(X)}:-d(X). ok(X):-d(X),1<=#count{Y:p(X,Y),a(Y)}.",
    );
}

#[test]
fn empty_positive_extensions_create_no_arithmetic_error() {
    for source in [
        "a(0;1).b(1;2).p(X):-a(X),b(X),1/X>0.",
        "a(1;2147483647).b(1;2).p(X):-a(X),b(X),X+1>0.",
        "a(0;1).b(1;2).p(X):-a(X),1/X>0,b(X).",
    ] {
        equal_theory(source);
    }
}

#[test]
fn table_rows_exclude_what_a_false_comparison_excludes() {
    // X<0 is defined and false on every row, so the division and the
    // aggregate assignment are never reached under either strategy, and the
    // two strategies ground the same theory.
    for source in [
        "d(0;1).e(0;1). :-d(X),e(X),X<0,1/X>0.",
        "d(0;1).e(0;1). :-d(X),1/X>0,e(X),X<0.",
        "a.b.d(1). :-d(X),N=#sum{2147483647:a;1:b},X<0.",
    ] {
        equal_theory(source);
    }
}

#[test]
fn table_strategy_preserves_deferred_head_errors() {
    equal_theory("d(0;1).e(0;1).p(1/X):-d(X),e(X),X>0.");
}

fn minimum(resource: FormulaResource) -> u64 {
    let compile = |maximum| {
        let mut limits = FormulaLimits::default();
        match resource {
            FormulaResource::Work => limits.max_work = maximum,
            FormulaResource::SupportBytes => {
                limits.max_support_bytes = usize::try_from(maximum).unwrap();
            }
            _ => unreachable!("selected finite resource"),
        }
        ground(
            SHARED,
            JoinStrategy::Table,
            &limits,
            &Observation::default(),
        )
    };
    let mut low = 0;
    let mut high = match resource {
        FormulaResource::Work => FormulaLimits::default().max_work,
        FormulaResource::SupportBytes => {
            u64::try_from(FormulaLimits::default().max_support_bytes).unwrap()
        }
        _ => unreachable!("selected finite resource"),
    };
    assert!(compile(high).is_ok());
    while low < high {
        let mid = low + (high - low) / 2;
        match compile(mid) {
            Ok(_) => high = mid,
            Err(FormulaFailure::Limit {
                resource: found,
                observed,
                limit,
                ..
            }) => {
                assert_eq!(found, resource);
                assert_eq!(limit, u128::from(mid));
                assert!(observed > limit);
                low = mid + 1;
            }
            other => panic!("unexpected resource result: {other:?}"),
        }
    }
    assert!(compile(low).is_ok());
    assert!(
        matches!(compile(low - 1), Err(FormulaFailure::Limit { resource: found, .. }) if found == resource)
    );
    low
}

#[test]
fn cumulative_table_work_has_an_exact_ceiling() {
    assert!(minimum(FormulaResource::Work) > 0);
}

#[test]
fn simultaneous_table_storage_has_an_exact_ceiling() {
    assert!(minimum(FormulaResource::SupportBytes) > 0);
}

#[test]
fn structural_patterns_retain_explicit_indexed_matching() {
    let work = equal_theory("d(1..2).r(f(1)).r(f(2)).{a(X)}:-d(X),r(f(X)).");
    assert!(work.table_inapplicable_probes.unwrap() > 0);
    assert!(work.indexed_probes.unwrap() > 0);
}

#[test]
fn nullary_table_rows_preserve_the_emitted_theory() {
    equal_theory("q.{p}:-q.");
}

#[test]
fn objective_queries_preserve_exact_templates() {
    equal_theory("d(1..3).{a(X)}:-d(X).#minimize{X@X,X:a(X),d(X)}.");
}
