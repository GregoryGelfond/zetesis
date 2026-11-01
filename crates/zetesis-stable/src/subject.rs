//! Class-indexed applications and their ordered joint closure.
//!
//! An application may make partial progress. Only an unchanged complete late
//! pass after eager agreement establishes the joint fixed point. This private
//! protocol is not a membership extension point; concrete owners determine
//! which subjects a candidate or frozen query may read.

use zetesis_algebra::{
    Class,
    region::{Narrowing, Region},
};
use zetesis_ferraris::{NarrowingAttempt, NarrowingScratch};

use crate::budget::{Budget, Quota};
use crate::{Incomplete, RegionCounts};

/// Position in one regional closure's separately owned source allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    First,
    Continue,
}

/// Local progress, distinct from agreement of the complete subject family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubjectStep {
    Unchanged,
    Changed,
    Refuted,
}

impl SubjectStep {
    /// The outcome of a single subject that has completed its own closure.
    pub(crate) fn narrowing(self) -> Narrowing {
        match self {
            Self::Unchanged => Narrowing::Fixed { changed: false },
            Self::Changed => Narrowing::Fixed { changed: true },
            Self::Refuted => Narrowing::Refuted,
        }
    }
}

/// A per-application search receipt, including any prefix before a failure.
/// Separately owned source work never enters these counts.
pub(crate) struct SubjectAttempt {
    pub(crate) result: Result<SubjectStep, Incomplete>,
    pub(crate) counts: RegionCounts,
}

impl SubjectAttempt {
    pub(crate) fn failed(error: Incomplete) -> Self {
        Self {
            result: Err(error),
            counts: RegionCounts::default(),
        }
    }

    /// Selection metadata remains with the concrete adapter. This conversion
    /// neither reruns the narrower nor adds a second account owner.
    pub(crate) fn from_narrowing(attempt: &NarrowingAttempt<Incomplete>) -> Self {
        let charges = &attempt.statistics;
        Self {
            result: attempt.result.map(|result| match result {
                Narrowing::Refuted => SubjectStep::Refuted,
                Narrowing::Fixed { changed: false } => SubjectStep::Unchanged,
                Narrowing::Fixed { changed: true } => SubjectStep::Changed,
            }),
            counts: RegionCounts {
                propagations: charges.propagations,
                held: charges.held,
                cut: charges.cut,
                work: charges.work,
                support_work: charges.support_work,
                founded_work: charges.founded_work,
                support_explanations: charges.support_explanations,
                ..RegionCounts::default()
            },
        }
    }

    /// Publish the admitted prefix before inspecting the application result.
    pub(crate) fn account(self, counts: &mut RegionCounts) -> Result<SubjectStep, Incomplete> {
        counts.add(self.counts)?;
        self.result
    }
}

/// Borrow existing control and the cumulative receipt without owning either.
/// Late publication can preflight its exact delta before polling or mutation.
pub(crate) struct SubjectContext<'call, 'control, Q: Quota> {
    pub(crate) budget: &'call mut Budget<'control, Q>,
    pub(crate) counts: &'call RegionCounts,
}

pub(crate) trait Subject<C: Class> {
    type Knowledge;

    fn narrow<Q: Quota>(
        &mut self,
        region: &mut Region,
        knowledge: &mut Self::Knowledge,
        scratch: &mut NarrowingScratch,
        pass: Pass,
        context: &mut SubjectContext<'_, '_, Q>,
    ) -> SubjectAttempt;
}

/// Apply the declared eager order and then one late consequence at a time.
///
/// Eager applications finish their own closure. Only a later eager change can
/// invalidate an earlier closure, so the first application's change alone
/// does not repeat a round. A late change immediately restarts eager work and
/// continues the same source allowance. Each change decides a fresh atom;
/// refusals abandon the partial closure rather than claiming agreement.
/// The iterator factory borrows the existing owners and allocates no registry.
pub(crate) fn joint<C, S, Q, I>(
    mut eager: impl FnMut() -> I,
    late: &mut [S],
    region: &mut Region,
    knowledge: &mut S::Knowledge,
    scratch: &mut NarrowingScratch,
    budget: &mut Budget<'_, Q>,
    counts: &mut RegionCounts,
) -> Result<Narrowing, Incomplete>
where
    C: Class,
    S: Subject<C>,
    Q: Quota,
    I: Iterator<Item = S>,
{
    let mut changed = false;
    let mut pass = Pass::First;
    'closure: loop {
        let mut revisit = false;
        for (index, mut subject) in eager().enumerate() {
            let attempt = subject.narrow(
                region,
                knowledge,
                scratch,
                pass,
                &mut SubjectContext { budget, counts },
            );
            match attempt.account(counts)? {
                SubjectStep::Refuted => return Ok(Narrowing::Refuted),
                SubjectStep::Changed => {
                    changed = true;
                    revisit |= index != 0;
                }
                SubjectStep::Unchanged => {}
            }
        }
        if revisit {
            continue;
        }
        for subject in &mut *late {
            let attempt = subject.narrow(
                region,
                knowledge,
                scratch,
                pass,
                &mut SubjectContext { budget, counts },
            );
            match attempt.account(counts)? {
                SubjectStep::Refuted => return Ok(Narrowing::Refuted),
                SubjectStep::Changed => {
                    changed = true;
                    pass = Pass::Continue;
                    continue 'closure;
                }
                SubjectStep::Unchanged => {}
            }
        }
        return Ok(Narrowing::Fixed { changed });
    }
}

#[cfg(test)]
mod tests;
