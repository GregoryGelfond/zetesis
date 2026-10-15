//! Cadence tests use the settled production entry, not the legacy test control.
use super::{Accounting, OBSERVATIONS, StopPrecedence, observe_every_charge};
use crate::formula_support::Counters;
use crate::grounding_observer::Work;
use crate::test_support::location;
use crate::{
    ConstraintCheckCause, ConstraintCheckFailure, FormulaFailure, FormulaLimits, FormulaResource,
    ProgramSite, ReconstructionError,
};
use zetesis_core::{AtomIndexError, ModelError, ModelFailure};
use zetesis_cpu::{Cancellation, Stop};

fn run<T, E: StopPrecedence>(
    account: &mut Accounting,
    cancellation: &Cancellation,
    action: impl FnOnce(&mut Counters) -> Result<T, E>,
) -> Result<T, E> {
    account.controlled(Some(cancellation), Work::default(), location(), action)
}
fn refusal(site: ProgramSite) -> FormulaFailure {
    FormulaFailure::Limit {
        resource: FormulaResource::SupportBytes,
        limit: 0,
        observed: 1,
        location: site,
    }
}

#[test]
fn control_handoff_reuses_the_workspace() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut account = Accounting::with_allowance(allowance.clone());
    account.work = 7;
    account.substitutions = 1;
    let pending = account.allowance.as_mut().unwrap();
    pending.work(7);
    pending.substitution();
    let mut lease = account.workspace.lease();
    lease.observe(32, location()).unwrap();

    let mut active = account.take_for_control();
    // Both handles identify the existing workspace. The placeholder owns no
    // charge history, so its retirement cannot publish or duplicate receipts.
    assert!(active.workspace.owns(&lease));
    assert!(account.workspace.owns(&lease));
    assert_eq!((account.work, account.substitutions), (0, 0));
    assert!(account.allowance.is_none());
    assert!(account.generated_values.is_none());
    drop(account);
    assert_eq!(allowance.statistics().work, 0);
    assert_eq!(allowance.statistics().substitutions, 0);
    assert_eq!(active.workspace.bytes(), 32);
    assert_eq!((active.work, active.substitutions), (7, 1));
    active.settle();
    assert_eq!(allowance.statistics().work, 7);
    assert_eq!(allowance.statistics().substitutions, 1);
    active.settle();
    assert_eq!(allowance.statistics().work, 7);
    assert_eq!(allowance.statistics().substitutions, 1);
    drop(lease);
    assert_eq!(active.workspace.bytes(), 0);
}

#[test]
fn refused_control_restores_live_workspace_and_charges() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut account = Accounting::with_allowance(allowance.clone());
    let mut lease = account.workspace.lease();
    lease.observe(32, location()).unwrap();
    let limits = FormulaLimits {
        max_work: 3,
        ..Default::default()
    };
    let result = run(&mut account, &Cancellation::default(), |counters| {
        assert!(counters.accounting.workspace.owns(&lease));
        counters.charge_work(2, &limits, location())?;
        counters.substitution(&limits, location())?;
        counters.charge_work(2, &limits, location())
    });
    assert!(matches!(
        result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            observed: 4,
            limit: 3,
            ..
        })
    ));
    assert!(account.workspace.owns(&lease));
    assert_eq!(account.workspace.bytes(), 32);
    assert_eq!((account.work, account.substitutions), (2, 1));
    assert_eq!(allowance.statistics().work, 2);
    assert_eq!(allowance.statistics().substitutions, 1);

    run(&mut account, &Cancellation::default(), |counters| {
        assert!(counters.accounting.workspace.owns(&lease));
        counters.work(&limits, location())
    })
    .unwrap();
    assert_eq!((account.work, account.substitutions), (3, 1));
    assert_eq!(allowance.statistics().work, 3);
    assert_eq!(allowance.statistics().substitutions, 1);
    drop(lease);
    assert_eq!(account.workspace.bytes(), 0);
}

#[test]
fn charged_ranges_observe_the_boundary_and_preserve_counts() {
    for initial in [0, 1, 7, 255, 256, 257] {
        for amount in [0_u64, 1, 255, 256, 257, 1024] {
            let mut account = Accounting {
                work: initial,
                ..Default::default()
            };
            OBSERVATIONS.set(0);
            run(&mut account, &Cancellation::default(), |counters| {
                counters.charge_work(u128::from(amount), &FormulaLimits::default(), location())
            })
            .unwrap();
            assert_eq!(account.work, initial + amount);
            assert_eq!(
                OBSERVATIONS.get(),
                2 + u64::from(zetesis_cpu::poll_due(initial, amount))
            );
        }
    }
}

#[test]
fn a_pending_stop_admits_at_most_255_more_units() {
    for initial in 0..256 {
        let cancellation = Cancellation::default();
        let mut account = Accounting {
            work: initial,
            ..Default::default()
        };
        let result = run(&mut account, &cancellation, |counters| {
            cancellation.cancel();
            for _ in 0..256 {
                counters.work(&FormulaLimits::default(), location())?;
            }
            Ok::<_, FormulaFailure>(())
        });
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        let expected = if initial == 0 { 0 } else { 256 - initial };
        assert_eq!(account.work - initial, expected);
        assert!(expected <= 255);
    }
}

#[test]
fn refusing_work_observes_between_boundaries() {
    let cancellation = Cancellation::default();
    let mut account = Accounting {
        work: 7,
        ..Default::default()
    };
    let limits = FormulaLimits {
        max_work: 7,
        ..Default::default()
    };
    let result = run(&mut account, &cancellation, |counters| {
        cancellation.cancel();
        let failure = counters.work(&limits, location());
        assert!(matches!(
            failure,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        Ok::<_, FormulaFailure>(())
    });
    assert!(result.is_err());
    assert_eq!(account.work, 7);
}

#[test]
fn absorbed_work_is_retained_before_observing_a_stop() {
    let cancellation = Cancellation::default();
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut account = Accounting::with_allowance(allowance.clone());
    let result = run(&mut account, &cancellation, |counters| {
        cancellation.cancel();
        counters.absorb(257, &FormulaLimits::default(), location())
    });
    assert!(matches!(result, Err(FormulaFailure::Interrupted { .. })));
    assert_eq!(account.work, 257);
    assert_eq!(allowance.statistics().work, 257);
}

#[test]
fn explicit_poll_prevents_starting_unhooked_work() {
    let cancellation = Cancellation::default();
    let mut entered = false;
    let result = run(&mut Accounting::default(), &cancellation, |counters| {
        counters.work(&FormulaLimits::default(), location())?;
        cancellation.cancel();
        counters.poll(location())?;
        entered = true;
        Ok::<_, FormulaFailure>(())
    });
    assert!(result.is_err());
    assert!(!entered);
}

#[test]
fn late_completion_stop_has_each_entry_error_shape() {
    macro_rules! check {
        ($error:ty, $pattern:pat) => {{
            let cancellation = Cancellation::default();
            let mut account = Accounting {
                work: 7,
                ..Default::default()
            };
            let result: Result<(), $error> = run(&mut account, &cancellation, |_| {
                cancellation.cancel();
                Ok(())
            });
            assert!(matches!(result, Err($pattern)));
            assert_eq!(account.work, 7);
        }};
    }
    check!(
        FormulaFailure,
        FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        }
    );
    check!(ReconstructionError, ReconstructionError::Source(_));
    check!(
        ConstraintCheckCause,
        ConstraintCheckCause::Stopped(Stop::Cancelled)
    );
    check!(
        ConstraintCheckFailure,
        ConstraintCheckFailure {
            cause: ConstraintCheckCause::Stopped(Stop::Cancelled),
            ..
        }
    );
}

#[test]
fn pending_entry_stop_does_not_enter_any_error_family() {
    fn check<E: StopPrecedence>() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let mut entered = false;
        let result: Result<(), E> = run(&mut Accounting::default(), &cancellation, |_| {
            entered = true;
            Ok(())
        });
        assert!(result.is_err());
        assert!(!entered);
    }
    check::<FormulaFailure>();
    check::<ReconstructionError>();
    check::<ConstraintCheckCause>();
    check::<ConstraintCheckFailure>();
}

#[test]
fn resource_replacement_keeps_site_and_program_envelope() {
    let original = crate::formula_support::testing::prepare("p.");
    // The canonical analyzed program is enough to identify this retained input;
    // no debug rendering of randomized metadata participates in equality.
    let program = std::sync::Arc::new(original.program.analyzed.full_program().clone());
    let site = ProgramSite::program();
    let cancellation = Cancellation::default();
    let error = FormulaFailure::Program {
        program: program.clone(),
        error: Box::new(refusal(site)),
    };
    let result: Result<(), FormulaFailure> = run(&mut Accounting::default(), &cancellation, |_| {
        cancellation.cancel();
        Err(error)
    });
    let error = result.unwrap_err();
    assert!(std::ptr::eq(
        error.original_program().unwrap(),
        program.as_ref()
    ));
    assert_eq!(error.site(), Some(site));
    assert_eq!(error.interruption(), Some(Stop::Cancelled));
}

#[test]
fn nested_resource_replacement_keeps_its_error_path() {
    let site = ProgramSite::program();
    let cancellation = Cancellation::default();
    let result: Result<(), ReconstructionError> =
        run(&mut Accounting::default(), &cancellation, |_| {
            cancellation.cancel();
            Err(ReconstructionError::Model(ModelFailure::Stopped(Box::new(
                refusal(site),
            ))))
        });
    assert!(
        matches!(result, Err(ReconstructionError::Model(ModelFailure::Stopped(error)))
        if error.site() == Some(site) && error.interruption() == Some(Stop::Cancelled))
    );
    let cancellation = Cancellation::default();
    let result: Result<(), ConstraintCheckCause> =
        run(&mut Accounting::default(), &cancellation, |_| {
            cancellation.cancel();
            Err(ConstraintCheckCause::Index(AtomIndexError::Stopped(
                Box::new(refusal(site)),
            )))
        });
    assert!(
        matches!(result, Err(ConstraintCheckCause::Index(AtomIndexError::Stopped(error)))
        if error.site() == Some(site) && error.interruption() == Some(Stop::Cancelled))
    );
}

#[test]
fn model_catalog_resources_settle_but_shape_failures_survive() {
    for (error, resource) in [
        (
            ModelError::Catalog(zetesis_core::catalog::Error::Allocation),
            true,
        ),
        (
            ModelError::Catalog(zetesis_core::catalog::Error::Shape),
            false,
        ),
        (
            ModelError::Position {
                position: 1,
                atoms: 0,
            },
            false,
        ),
    ] {
        let cancellation = Cancellation::default();
        let result: Result<(), ReconstructionError> =
            run(&mut Accounting::default(), &cancellation, |_| {
                cancellation.cancel();
                Err(ReconstructionError::Model(ModelFailure::Model(error)))
            });
        let error = result.unwrap_err();
        assert_eq!(error.stop(), resource.then_some(Stop::Cancelled));
    }
}

#[test]
fn nonresource_failures_keep_their_original_meaning() {
    let errors = [
        FormulaFailure::SourceActivity {
            location: location(),
        },
        FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location: location(),
        },
        FormulaFailure::UnadmittedTerm {
            location: location(),
        },
    ];
    for error in errors {
        let cancellation = Cancellation::default();
        let result: Result<(), FormulaFailure> =
            run(&mut Accounting::default(), &cancellation, |_| {
                cancellation.cancel();
                Err(error)
            });
        assert_eq!(result.unwrap_err().interruption(), None);
    }
}

#[test]
fn settled_unwind_restores_the_shared_receipt() {
    let allowance = crate::ConstraintAllowance::new(crate::ConstraintCheckLimits::default());
    let mut account = Accounting::with_allowance(allowance.clone());
    let mut lease = account.workspace.lease();
    lease.observe(32, location()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), FormulaFailure> =
            run(&mut account, &Cancellation::default(), |counters| {
                assert!(counters.accounting.workspace.owns(&lease));
                counters.charge_work(257, &FormulaLimits::default(), location())?;
                counters.substitution(&FormulaLimits::default(), location())?;
                panic!("after accepted work");
            });
    }));
    assert!(result.is_err());
    assert!(account.workspace.owns(&lease));
    assert_eq!(account.workspace.bytes(), 32);
    assert_eq!((account.work, account.substitutions), (257, 1));
    assert_eq!(allowance.statistics().work, 257);
    assert_eq!(allowance.statistics().substitutions, 1);
    run(&mut account, &Cancellation::default(), |counters| {
        assert!(counters.accounting.workspace.owns(&lease));
        counters.work(&FormulaLimits::default(), location())
    })
    .unwrap();
    assert_eq!(account.work, 258);
    assert_eq!(allowance.statistics().work, 258);
    assert_eq!(allowance.statistics().substitutions, 1);
    drop(lease);
    assert_eq!(account.workspace.bytes(), 0);
}

#[cfg(debug_assertions)]
#[test]
fn escaped_control_is_rejected_when_observed() {
    let mut escaped = run(
        &mut Accounting::default(),
        &Cancellation::default(),
        |counters| Ok::<_, FormulaFailure>(std::mem::take(counters)),
    )
    .unwrap();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| escaped.poll(location())));
    assert!(result.is_err());
    escaped.detach_test_control();
}

#[test]
fn legacy_observation_changes_no_completed_receipt() {
    fn completed() -> (u64, u64) {
        let mut account = Accounting::default();
        run(&mut account, &Cancellation::default(), |counters| {
            for amount in [1, 255, 0, 257, 2] {
                counters.charge_work(amount, &FormulaLimits::default(), location())?;
                counters.substitution(&FormulaLimits::default(), location())?;
            }
            Ok::<_, FormulaFailure>(())
        })
        .unwrap();
        (account.work, account.substitutions)
    }
    assert_eq!(completed(), observe_every_charge(completed));
}

#[test]
fn terminal_baseline_tail_refusal_is_settled() {
    let cancellation = Cancellation::default();
    let mut account = Accounting {
        work: 7,
        ..Default::default()
    };
    let result: Result<(), FormulaFailure> = run(&mut account, &cancellation, |counters| {
        let baseline = counters.accounting.flat_baseline().unwrap();
        assert_eq!(baseline.work, 7);
        cancellation.cancel();
        Err(refusal(ProgramSite::program()))
    });
    assert_eq!(result.unwrap_err().interruption(), Some(Stop::Cancelled));
    assert_eq!(account.work, 7);
}

fn table_failure(error: zetesis_cpu::table::MeteredFailure<FormulaFailure>) -> FormulaFailure {
    match error.cause {
        zetesis_cpu::table::MeteredCause::Stopped(error) => error,
        zetesis_cpu::table::MeteredCause::Table(cause) => FormulaFailure::SupportTable {
            error: zetesis_cpu::table::Failure {
                cause,
                work: error.work,
                peak_bytes: error.peak_bytes,
            },
            location: location(),
        },
    }
}

#[test]
fn table_zero_callbacks_do_not_observe_grounding_control() {
    use zetesis_core::{Atom, Predicate, Value, relation};
    use zetesis_cpu::table::{Limits, Table};
    let predicate = Predicate::new("p", 1).unwrap();
    let atoms: Vec<_> = (0..300)
        .map(|n| Atom::new(predicate.clone(), vec![Value::Number(n)]).unwrap())
        .collect();
    let relation =
        relation::Relation::from_atoms(&predicate, &atoms, relation::Limits::default()).unwrap();
    let mut account = Accounting {
        work: 7,
        ..Default::default()
    };
    let mut zeros = 0;
    let mut positive = 0;
    let mut due = 0;
    OBSERVATIONS.set(0);
    run(&mut account, &Cancellation::default(), |counters| {
        let table = Table::prepare_with(&relation, &[0], Limits::default(), |amount| {
            if amount == 0 {
                zeros += 1;
            } else {
                positive += 1;
            }
            due += u64::from(zetesis_cpu::poll_due(
                counters.accounting.work,
                amount as u64,
            ));
            counters.charge_work(amount as u128, &FormulaLimits::default(), location())
        })
        .map_err(table_failure)?;
        assert_eq!(counters.accounting.work, 7 + table.statistics().work);
        Ok::<_, FormulaFailure>(())
    })
    .unwrap();
    assert!(account.work > 256 && zeros > 0 && positive > due);
    assert_eq!(OBSERVATIONS.get(), 2 + due);
}

#[test]
fn table_capacity_refusal_under_pending_stop_settles_at_exit() {
    use zetesis_core::{Predicate, relation};
    use zetesis_cpu::table::{Cause, Limits, MeteredCause, Table};
    let predicate = Predicate::new("p", 0).unwrap();
    let relation =
        relation::Relation::from_atoms(&predicate, &[], relation::Limits::default()).unwrap();
    let cancellation = Cancellation::default();
    let mut account = Accounting {
        work: 7,
        ..Default::default()
    };
    let result: Result<(), FormulaFailure> = run(&mut account, &cancellation, |counters| {
        let Err(error) = Table::prepare_with(
            &relation,
            &[],
            Limits {
                max_bytes: 0,
                ..Limits::default()
            },
            |amount| {
                assert_eq!(amount, 0);
                cancellation.cancel();
                counters.charge_work(amount as u128, &FormulaLimits::default(), location())
            },
        ) else {
            panic!("the table header cannot fit zero bytes");
        };
        assert!(matches!(
            error.cause,
            MeteredCause::Table(Cause::Limit { .. })
        ));
        Err(table_failure(error))
    });
    assert_eq!(result.unwrap_err().interruption(), Some(Stop::Cancelled));
    assert_eq!(account.work, 7);
}

#[derive(Default)]
struct Receipts(std::cell::Cell<crate::GroundingWork>);
impl crate::GroundingObserver for Receipts {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: crate::GroundingPhase,
        _: Option<ProgramSite>,
        _: crate::GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        self.0.set(self.0.get().checked_sum(work));
    }
}

#[test]
fn eager_grounding_keeps_completed_theory_and_receipts() {
    let source = include_str!("../../../tests/fixtures/expression-keys/diagonal.lp");
    let execute = |receipts: &Receipts| {
        let mut preparation = crate::formula_support::testing::prepare(source);
        preparation.budget = preparation
            .budget
            .with_cancellation(Some(Cancellation::default()));
        crate::formula_ground::ground(preparation, Some(receipts), None).unwrap()
    };
    let old_receipts = Receipts::default();
    let reference = observe_every_charge(|| execute(&old_receipts));
    let receipts = Receipts::default();
    let actual = execute(&receipts);
    assert_eq!(
        crate::test_support::compiled_atoms(&actual),
        crate::test_support::compiled_atoms(&reference)
    );
    assert_eq!(actual.theory.nodes(), reference.theory.nodes());
    assert_eq!(actual.theory.operands(), reference.theory.operands());
    assert_eq!(actual.theory.roots(), reference.theory.roots());
    assert_eq!(actual.origins, reference.origins);
    assert_eq!(actual.warnings, reference.warnings);
    assert_eq!(receipts.0.get(), old_receipts.0.get());
}

#[test]
fn nested_formula_resource_families_settle_at_their_own_site() {
    use zetesis_core::{TemplateCatalogFailure, catalog};
    let site = ProgramSite::program();
    let allocation = || Vec::<u8>::new().try_reserve(usize::MAX).unwrap_err();
    let expansion = || crate::ExpansionFailure::Allocation {
        error: allocation(),
        location: site,
    };
    let failures = [
        FormulaFailure::TemplateCatalog {
            error: TemplateCatalogFailure::Storage(catalog::Error::Allocation),
            location: site,
        },
        FormulaFailure::TermAssignment {
            error: catalog::AssignmentError::Storage(catalog::Error::IdExhausted),
            location: site,
        },
        FormulaFailure::ObjectiveCondition {
            error: zetesis_objective::ConditionError::Storage(catalog::Error::Overflow),
            location: site,
        },
        FormulaFailure::AtomCatalog {
            error: catalog::Error::Storage {
                required: 1,
                limit: 0,
            },
            location: site,
        },
        FormulaFailure::MetadataAllocation {
            error: allocation(),
            location: site,
        },
        FormulaFailure::AtomAllocation {
            error: allocation(),
            location: site,
        },
        FormulaFailure::SupportTable {
            error: zetesis_cpu::table::Failure {
                cause: zetesis_cpu::table::Cause::Allocation,
                work: 7,
                peak_bytes: 0,
            },
            location: site,
        },
        FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Overflow,
            location: site,
        },
        FormulaFailure::Logical {
            error: crate::ProgramFailureKind::Core(zetesis_core::AdmissionError::Canonical {
                error: catalog::Error::Allocation,
                origin: zetesis_core::AdmissionOrigin::Program,
            }),
            part: None,
            location: site,
        },
        FormulaFailure::Expansion(expansion()),
        FormulaFailure::Include(Box::new(
            crate::BundleAdmissionError::Expansion(expansion()),
        )),
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::ConditionStorage {
                template: 0,
                error: zetesis_objective::ConditionError::Storage(catalog::Error::Allocation),
            },
            location: site,
        },
        FormulaFailure::Theory {
            error: zetesis_ferraris::AdmissionError::Allocation,
            location: site,
        },
    ];
    for failure in failures {
        let cancellation = Cancellation::default();
        let result: Result<(), FormulaFailure> =
            run(&mut Accounting::default(), &cancellation, |_| {
                cancellation.cancel();
                Err(failure)
            });
        let settled = result.unwrap_err();
        assert_eq!(settled.interruption(), Some(Stop::Cancelled));
        assert_eq!(settled.site(), Some(site));
    }
}

#[test]
fn nested_formula_shape_families_survive_a_pending_stop() {
    use zetesis_core::{TemplateCatalogFailure, catalog};
    let site = location();
    let failures = [
        FormulaFailure::TemplateCatalog {
            error: TemplateCatalogFailure::Incomplete,
            location: site,
        },
        FormulaFailure::TermAssignment {
            error: catalog::AssignmentError::Read(catalog::ReadError::ForeignCatalog),
            location: site,
        },
        FormulaFailure::ObjectiveCondition {
            error: zetesis_objective::ConditionError::Reference {
                node: 0,
                operand: 0,
            },
            location: site,
        },
        FormulaFailure::AtomCatalog {
            error: catalog::Error::Shape,
            location: site,
        },
        FormulaFailure::SupportTable {
            error: zetesis_cpu::table::Failure {
                cause: zetesis_cpu::table::Cause::Scope,
                work: 0,
                peak_bytes: 0,
            },
            location: site,
        },
        FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location: site,
        },
        FormulaFailure::Logical {
            error: crate::ProgramFailureKind::Core(
                zetesis_core::AdmissionError::NonDenseVariable {
                    template: 0,
                    expected: 0,
                    actual: 2,
                },
            ),
            part: None,
            location: site,
        },
        FormulaFailure::Objective {
            error: zetesis_objective::AdmissionError::MissingPriority { template: 0 },
            location: site,
        },
        FormulaFailure::Theory {
            error: zetesis_ferraris::AdmissionError::Root,
            location: site,
        },
    ];
    for failure in failures {
        let cancellation = Cancellation::default();
        let expected = std::mem::discriminant(&failure);
        let result: Result<(), FormulaFailure> =
            run(&mut Accounting::default(), &cancellation, |_| {
                cancellation.cancel();
                Err(failure)
            });
        let settled = result.unwrap_err();
        assert_eq!(std::mem::discriminant(&settled), expected);
        assert_eq!(settled.interruption(), None);
        assert_eq!(settled.site(), Some(site));
    }
}

#[test]
fn checker_failure_settlement_preserves_its_receipt() {
    let cancellation = Cancellation::default();
    let mut account = Accounting::default();
    let result: Result<(), ConstraintCheckFailure> = run(&mut account, &cancellation, |counters| {
        counters
            .charge_work(7, &FormulaLimits::default(), location())
            .unwrap();
        cancellation.cancel();
        Err(ConstraintCheckFailure {
            cause: ConstraintCheckCause::Source(Box::new(refusal(ProgramSite::program()))),
            statistics: crate::ConstraintCheckStatistics {
                work: 7,
                substitutions: 0,
                scalar_bytes: 13,
            },
        })
    });
    let error = result.unwrap_err();
    assert_eq!(error.stop(), Some(Stop::Cancelled));
    assert_eq!(error.statistics.work, 7);
    assert_eq!(error.statistics.scalar_bytes, 13);
    assert!(
        matches!(error.cause, ConstraintCheckCause::Source(error) if error.site() == Some(ProgramSite::program()))
    );
    assert_eq!(account.work, 7);
}
