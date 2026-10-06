//! Shared admission precedes table work and preserves refused prefix receipts.

use super::*;
use crate::{ConstraintAllowance, ConstraintCheckLimits};
use zetesis_cpu::Cancellation;

fn select(
    support: &Support<'_>,
    counters: &mut Counters,
    limits: &FormulaLimits,
) -> Result<(), FormulaFailure> {
    let flat = pattern(false);
    let pattern = PositivePattern::Flat((&flat).into());
    let rows = support.resolve(pattern.atom(), limits, counters, location())?;
    let binding = [Some(Value::Number(1)), None];
    let permitted = [Value::Number(8), Value::Number(15)];
    let permitted = permitted.each_ref().map(TermRef::from);
    let selected = support
        .select_domains_at(
            rows,
            pattern,
            binding.as_slice().into(),
            FiniteDomains {
                values: &permitted,
                variables: &[(1, 0..2)],
            },
            GroundingWork::new(limits, counters, location()),
        )?
        .expect("certified domains use tables under Indexed strategy");
    assert_eq!(selected.selection.rows().collect::<Vec<_>>(), [8, 15]);
    Ok(())
}

fn with_support(warm: bool, run: impl FnOnce(&Support<'_>)) {
    let catalog = catalog();
    let limits = FormulaLimits::default();
    let mut setup = Counters::default();
    let relations = catalog.snapshot(&limits, &mut setup, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &setup, location()).unwrap();
    if warm {
        select(&support, &mut setup, &limits).unwrap();
    }
    run(&support);
}

fn refuses_with_its_prefix(warm: bool) {
    let mut complete = 0;
    let mut receipt = crate::GroundingWork::default();
    with_support(warm, |support| {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let mut counters = Counters::resume(
            crate::formula_support::Accounting::default(),
            profile.work(),
        );
        profile
            .phase(GroundingPhase::RuleInstantiation, None, || {
                select(support, &mut counters, &FormulaLimits::default())
            })
            .unwrap();
        complete = counters.accounting.work;
        receipt = observer.0.get();
    });
    let prepared = receipt.table_prepare_work.unwrap();
    let queried = receipt.table_query_work.unwrap();
    assert_eq!(prepared == 0, warm);
    assert!(queried > 1);
    assert!(warm || prepared > 1);
    // All non-table query charges precede preparation or reused selection.
    // Leave exactly one positive table tick; another checker's work reaches
    // the shared receipt but never this check's own ceiling.
    let prefix = complete - prepared - queried;
    let spent = 101;
    let max_work = prefix + 1;
    let allowance = ConstraintAllowance::new(ConstraintCheckLimits::default());
    let cancellation = Cancellation::default();
    let limits = FormulaLimits {
        max_work,
        ..FormulaLimits::default()
    };
    let mut other = Counters::with_allowance(allowance.clone(), &cancellation);
    other
        .charge_work(u128::from(spent), &FormulaLimits::default(), location())
        .unwrap();
    with_support(warm, |support| {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let mut counters = Counters::with_allowance(allowance.clone(), &cancellation);
        counters.observed = profile.work();
        let failure = profile
            .phase(GroundingPhase::RuleInstantiation, None, || {
                select(support, &mut counters, &limits)
            })
            .unwrap_err();
        assert!(matches!(failure, FormulaFailure::Limit {
            resource: FormulaResource::Work, observed, limit, ..
        } if observed > limit && limit == u128::from(max_work)));
        let receipt = observer.0.get();
        assert_eq!(receipt.table_prepare_work, Some(u64::from(!warm)));
        assert_eq!(receipt.table_query_work, Some(u64::from(warm)));
        assert!(u128::from(receipt.support_peak_bytes.unwrap()) > support.live_bytes() as u128);
        assert_eq!(counters.accounting.work, prefix + 1);
        assert_eq!(allowance.statistics().work, spent + max_work);
        assert_eq!(
            allowance.statistics().work,
            spent + counters.accounting.work
        );
        assert_eq!(
            support.tables.get().unwrap().indices.borrow().len(),
            usize::from(warm)
        );
    });
}

#[test]
fn a_refusal_during_table_preparation_keeps_its_prefix() {
    refuses_with_its_prefix(false);
}

#[test]
fn a_refusal_during_reused_selection_keeps_its_prefix() {
    refuses_with_its_prefix(true);
}
