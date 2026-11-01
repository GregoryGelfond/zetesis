//! Concrete readings of the existing candidate and frozen owners.

use zetesis_algebra::{AnswerSetsSought, Countermodels};
use zetesis_ferraris::{Knowledge, Narrower, NarrowingScratch, OriginalSubject, Theory};

use super::super::conditions::{Bound, CandidateKnowledge};
use super::BudgetQuota;
use crate::budget::Quota;
use crate::region_filter::Check;
use crate::streamed_constraints::ConstraintConsequence;
use crate::subject::{Pass, Subject, SubjectAttempt, SubjectContext, SubjectStep};
use crate::{Incomplete, RegionCounts};
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
    Streamed {
        theory: &'theory Theory,
        check: &'check mut Check<'factory, 'state>,
    },
}

impl Subject<AnswerSetsSought> for CandidateSubject<'_, '_, '_, '_> {
    type Knowledge = CandidateKnowledge;

    fn narrow<Q: Quota>(
        &mut self,
        region: &mut Region,
        knowledge: &mut Self::Knowledge,
        scratch: &mut NarrowingScratch,
        pass: Pass,
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
            Self::Streamed { theory, check } => {
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

/// The original source owner validates an open atom and accounts its own work
/// before this search publication boundary. The cumulative receipt preflight
/// precedes both existing polls and the one-operation debit. A failed debit or
/// mutation publishes no propagation delta; any debit already made is retained
/// by the search account. No split decision is charged.
fn streamed<Q: Quota>(
    theory: &Theory,
    check: &mut Check<'_, '_>,
    region: &mut Region,
    pass: Pass,
    context: &mut SubjectContext<'_, '_, Q>,
) -> SubjectAttempt {
    let consequence = match check.consequence(theory, region, context.budget.cancellation, pass) {
        Ok(consequence) => consequence,
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
