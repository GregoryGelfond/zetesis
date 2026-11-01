//! Concrete readings of the existing candidate and frozen owners.

use zetesis_algebra::{AnswerSetsSought, Countermodels, Failure, Meter};
use zetesis_ferraris::{
    ArithmeticAttempt, CardinalitySystem, Knowledge, Narrower, NarrowingScratch, OriginalSubject,
    ParitySystem, Theory,
};

use super::super::conditions::{Bound, CandidateKnowledge};
use super::arithmetic_counts::ArithmeticKind;
use super::{BudgetQuota, CandidateReading, meter_failure};
use crate::budget::{Budget, Quota};
use crate::region_filter::Check;
use crate::streamed_constraints::ConstraintConsequence;
use crate::subject::{Pass, Subject, SubjectAttempt, SubjectContext, SubjectStep};
use crate::{Cancellation, Incomplete, RegionCounts};
use zetesis_algebra::region::Region;

/// Borrowed candidate owners; knowledge remains in its existing lazy slots.
/// Source state retains its independent worker lifetime and allowance.
pub(super) enum CandidateSubject<'theory, 'check, 'factory, 'state> {
    Original {
        reading: OriginalSubject<'theory>,
        narrower: &'theory Narrower,
    },
    Restriction {
        index: usize,
        theory: &'theory Theory,
        narrower: &'theory Narrower,
    },
    Bound(&'theory Bound),
    Parity(&'theory ParitySystem),
    Cardinality(&'theory CardinalitySystem),
    Streamed {
        theory: &'theory Theory,
        check: &'check mut Check<'factory, 'state>,
        /// Source allowance lifecycle, independent of earlier late progress.
        pass: Pass,
    },
}

impl Subject<AnswerSetsSought> for CandidateSubject<'_, '_, '_, '_> {
    type Knowledge = CandidateKnowledge;

    fn narrow<Q: Quota>(
        &mut self,
        region: &mut Region,
        knowledge: &mut Self::Knowledge,
        scratch: &mut NarrowingScratch,
        _pass: Pass,
        context: &mut SubjectContext<'_, '_, Q>,
    ) -> SubjectAttempt {
        let (reading, narrower, known) = match self {
            Self::Original { reading, narrower } => {
                (*reading, *narrower, knowledge.permanent(0, narrower))
            }
            Self::Restriction {
                index,
                theory,
                narrower,
            } => (
                OriginalSubject::new(theory, None),
                *narrower,
                knowledge.permanent(*index, narrower),
            ),
            Self::Bound(bound) => {
                let (theory, narrower) = bound.index.as_ref();
                (
                    OriginalSubject::new(theory, None),
                    narrower,
                    knowledge.bound(bound),
                )
            }
            Self::Parity(system) => {
                return arithmetic(
                    ArithmeticKind::Parity,
                    context.budget,
                    |cancellation, meter| {
                        system.inspect_reserved(
                            region,
                            knowledge.parity(system),
                            scratch,
                            cancellation,
                            meter,
                        )
                    },
                );
            }
            Self::Cardinality(system) => {
                return arithmetic(
                    ArithmeticKind::Cardinality,
                    context.budget,
                    |cancellation, meter| {
                        system.inspect_reserved(
                            region,
                            knowledge.cardinality(system),
                            scratch,
                            cancellation,
                            meter,
                        )
                    },
                );
            }
            Self::Streamed {
                theory,
                check,
                pass,
            } => {
                return streamed(theory, check, region, pass, context);
            }
        };
        let known = match known {
            Ok(known) => known,
            Err(error) => return SubjectAttempt::failed(error),
        };
        let attempt = BudgetQuota::narrow(context.budget, |cancellation, meter| {
            narrower.propagate_known_reserved(reading, region, known, scratch, cancellation, meter)
        });
        SubjectAttempt::from_narrowing(&attempt)
    }
}

/// The declared late order, retaining absence without allocating a registry.
/// The common driver skips None and restarts eager closure after any change.
pub(super) fn late<'theory, 'check, 'factory, 'state>(
    reading: &CandidateReading<'theory>,
    filter: Option<&'check mut Check<'factory, 'state>>,
) -> [Option<CandidateSubject<'theory, 'check, 'factory, 'state>>; 3] {
    [
        reading
            .arithmetic
            .map(|prepared| CandidateSubject::Parity(prepared.parity())),
        reading
            .arithmetic
            .and_then(|prepared| prepared.cardinality())
            .map(CandidateSubject::Cardinality),
        filter.map(|check| CandidateSubject::Streamed {
            theory: reading.original.theory(),
            check,
            pass: Pass::First,
        }),
    ]
}

/// Translate one arithmetic batch without asserting its local fixed closure.
/// The arithmetic owner already refunds once; this adapter neither resettles
/// it nor converts a typed refusal into a semantic refutation.
fn arithmetic<'call, 'control, Q: Quota>(
    kind: ArithmeticKind,
    budget: &'call mut Budget<'control, Q>,
    run: impl FnOnce(
        &Cancellation,
        &mut Meter<'_, BudgetQuota<'call, 'control, Q>>,
    ) -> ArithmeticAttempt<Failure<Incomplete>>,
) -> SubjectAttempt {
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota { budget };
    let mut meter = Meter::reserved(&mut quota);
    let attempt = run(cancellation, &mut meter);
    let activity = kind.counts(&attempt);
    let mut result =
        SubjectAttempt::from_statistics(attempt.result.map_err(meter_failure), &attempt.statistics);
    result.counts.arithmetic = activity;
    result
}

/// A child copy and its actual search-work receipt, on success or failure.
/// Publish counts before inspecting result. An error exposes no partial child.
pub(in crate::ferraris) struct KnowledgeCopy {
    pub(in crate::ferraris) result: Result<CandidateKnowledge, Incomplete>,
    pub(in crate::ferraris) counts: RegionCounts,
}

/// Reuse one retired child outside the frontier lock under the current budget.
/// No independent allowance is created; one Meter owns both arithmetic copies
/// and the encompassing refund, even if the second copy refuses.
pub(in crate::ferraris) fn copy_knowledge<Q: Quota>(
    parent: &CandidateKnowledge,
    mut retired: Option<CandidateKnowledge>,
    budget: &mut Budget<'_, Q>,
) -> KnowledgeCopy {
    let cancellation = budget.cancellation;
    let mut quota = BudgetQuota { budget };
    let mut meter = Meter::reserved(&mut quota);
    let result = parent.copy_reserved(&mut retired, cancellation, &mut meter);
    meter.refund();
    let counts = RegionCounts {
        work: meter.charged(),
        ..RegionCounts::default()
    };
    KnowledgeCopy {
        result: result
            .map_err(meter_failure)
            .map(|()| retired.expect("successful copy publishes its child")),
        counts,
    }
}

/// The original source owner validates an open atom and accounts its own work
/// before this search publication boundary. The cumulative receipt preflight
/// precedes both existing polls and the one-operation debit. A failed debit or
/// mutation publishes no propagation delta; any debit already made is retained
/// by the search account. No split decision is charged.
fn streamed<Q: Quota>(
    theory: &Theory,
    check: &mut Check<'_, '_>,
    region: &mut Region,
    pass: &mut Pass,
    context: &mut SubjectContext<'_, '_, Q>,
) -> SubjectAttempt {
    let consequence = match check.consequence(theory, region, context.budget.cancellation, *pass) {
        Ok(consequence) => {
            // Only an actual successful source call opens this allowance.
            // Arithmetic/eager restarts neither open it nor renew it.
            *pass = Pass::Continue;
            consequence
        }
        Err(error) => return SubjectAttempt::failed(error),
    };
    let (atom, held) = match consequence {
        ConstraintConsequence::NoConsequence => {
            return SubjectAttempt {
                result: Ok(SubjectStep::Unchanged),
                counts: RegionCounts::default(),
            };
        }
        ConstraintConsequence::Refuted { .. } => {
            return SubjectAttempt {
                result: Ok(SubjectStep::Refuted),
                counts: RegionCounts::default(),
            };
        }
        ConstraintConsequence::Hold { atom, .. } => (atom, true),
        ConstraintConsequence::Cut { atom, .. } => (atom, false),
    };
    let delta = RegionCounts {
        held: u64::from(held),
        cut: u64::from(!held),
        work: 1,
        propagations: 1,
        ..RegionCounts::default()
    };
    let result = (|| {
        let mut checked = *context.counts;
        checked.add(delta)?;
        context.budget.cancellation.poll()?;
        context.budget.tick()?;
        let applied = if held {
            region.hold(atom)
        } else {
            region.cut(atom)
        };
        if !applied {
            return Err(Incomplete::InvalidRegionConsequence);
        }
        Ok(SubjectStep::Changed)
    })();
    match result {
        Ok(step) => SubjectAttempt {
            result: Ok(step),
            counts: delta,
        },
        Err(error) => SubjectAttempt::failed(error),
    }
}

/// A trusted (theory, model-truth) reading, without candidate components.
/// Its existing Choose call returns the preference alongside the local closure;
/// the caller publishes that preference only after accounting the attempt.
pub(super) struct Frozen<'a> {
    narrower: &'a Narrower,
    reading: zetesis_ferraris::FrozenSubject<'a>,
    pub(super) preferred: Option<usize>,
}

impl<'a> Frozen<'a> {
    pub(super) fn new(
        narrower: &'a Narrower,
        reading: zetesis_ferraris::FrozenSubject<'a>,
    ) -> Self {
        Self {
            narrower,
            reading,
            preferred: None,
        }
    }
}

impl Subject<Countermodels> for Frozen<'_> {
    type Knowledge = Knowledge<Countermodels>;

    fn narrow<Q: Quota>(
        &mut self,
        region: &mut Region,
        knowledge: &mut Self::Knowledge,
        scratch: &mut NarrowingScratch,
        _pass: Pass,
        context: &mut SubjectContext<'_, '_, Q>,
    ) -> SubjectAttempt {
        let attempt = BudgetQuota::narrow(context.budget, |cancellation, meter| {
            self.narrower.narrow_frozen_known_reserved(
                self.reading,
                region,
                knowledge,
                scratch,
                cancellation,
                meter,
            )
        });
        self.preferred = attempt.preferred;
        SubjectAttempt::from_narrowing(&attempt)
    }
}

#[cfg(test)]
mod tests;
