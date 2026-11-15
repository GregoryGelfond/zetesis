//! Probes: frozen narrowings from a copy of the closed query root with a
//! guard and a pattern decided. A refuted probe is the premise of
//! `Zetesis.Algebra.entailed_by_refutations`; any other outcome allows the
//! pattern.

use zetesis_algebra::region::SubjectStep;
use zetesis_algebra::words::Words;
use zetesis_algebra::{Cancellation, Countermodels, Quota};

use super::super::{
    ParitySystem,
    budget::{Budget, BudgetError},
    storage::Storage,
};
use super::proposals::Group;
use crate::{FrozenSubject, Knowledge, Narrower, Narrowing, NarrowingScratch, Region};

/// Rows already established, narrowing beside the frozen reading in a probe:
/// sound for the class, as the frozen reading is, so their joint closure
/// refutes what either alone cannot.
pub(super) type Rows<'a> = Option<&'a ParitySystem>;

/// A group's forbidden table: one bit per pattern over its support.
pub(super) type Table = Words<u64, [u64; 4]>;

/// The group a table is probed for, with the patterns already known refuted,
/// which run no probe.
pub(super) struct Target<'a> {
    pub group: &'a Group,
    pub known: &'a Table,
    pub forbidden: &'a mut Table,
}

/// A group's probed table with what it cost and what it found: the probes
/// run and the patterns refuted, the known ones included.
pub(super) struct Probed {
    pub run: u64,
    pub refuted: u64,
}

/// The closed root, its knowledge, and one retired region and knowledge the
/// probes reuse.
pub(super) struct Probe<'a> {
    narrower: &'a Narrower,
    subject: FrozenSubject<'a>,
    root: &'a Region,
    knowledge: &'a Knowledge<Countermodels>,
    region: Region,
    retired: Knowledge<Countermodels>,
}

impl<'a> Probe<'a> {
    pub(super) fn new(
        narrower: &'a Narrower,
        subject: FrozenSubject<'a>,
        root: &'a Region,
        knowledge: &'a Knowledge<Countermodels>,
    ) -> Self {
        Self {
            narrower,
            subject,
            root,
            knowledge,
            region: root.clone(),
            retired: knowledge.clone(),
        }
    }

    /// The retired copies' bytes, held for the preparation's life.
    pub(super) fn retained_bytes(&self) -> u128 {
        self.region.retained_bytes() + self.retired.retained_bytes()
    }

    /// Bytes of this private probe, including both copies and its borrowed
    /// handles. The copied region and knowledge have fixed-size payloads.
    pub(super) fn copy_bytes(root: &Region, knowledge: &Knowledge<Countermodels>) -> u128 {
        (size_of::<Self>() - size_of::<Region>() - size_of::<Knowledge<Countermodels>>()) as u128
            + root.retained_bytes()
            + knowledge.retained_bytes()
    }

    pub(super) fn live_bytes(&self) -> u128 {
        Self::copy_bytes(&self.region, &self.retired)
    }

    /// The region the last probe narrowed.
    pub(super) const fn region(&self) -> &Region {
        &self.region
    }

    /// Narrow a copy of the closed root with `decisions` applied; whether the
    /// copy is refuted. A decision the root already contradicts refutes the
    /// copy without narrowing. Copying charges one operation per retained
    /// word and per decision.
    ///
    /// # Errors
    /// The meter's refusal or a stop, with the narrowing's performed prefix
    /// charged.
    pub(super) fn run<Q: Quota + ?Sized>(
        &mut self,
        decisions: &[(usize, bool)],
        rows: Rows<'_>,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        storage: &Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<bool, BudgetError<Q::Error>> {
        let words = u64::try_from(self.retained_bytes().div_ceil(8)).unwrap_or(u64::MAX);
        let copying = words.saturating_add(decisions.len() as u64);
        let consistent = budget.meter_mut().bounded(copying, || {
            self.region.clone_from(self.root);
            self.retired.clone_from(self.knowledge);
            let consistent = decisions.iter().all(|&(atom, held)| {
                if held {
                    self.region.hold(atom)
                } else {
                    self.region.cut(atom)
                }
            });
            (consistent, copying)
        })?;
        if !consistent {
            return Ok(true);
        }
        // The snapshot is private to this probe. A Boolean refutation reaches
        // no arithmetic check and requires no arithmetic workspace.
        let mut storage = Storage::with_live_bytes(storage.bytes());
        let mut known = None;
        loop {
            let attempt = self.narrower.propagate_frozen_known_reserved(
                self.subject,
                &mut self.region,
                &mut self.retired,
                scratch,
                cancellation,
                budget.meter_mut(),
            );
            if attempt.result? == Narrowing::Refuted {
                return Ok(true);
            }
            let Some(system) = rows else {
                return Ok(false);
            };
            let known = match &mut known {
                Some(prior) => prior,
                None => known.insert(system.probe_knowledge(&self.region, &mut storage, budget)?),
            };
            let (knowledge, buffer) = known;
            let inspected = system.inspect_with_buffer(
                &mut self.region,
                knowledge,
                scratch,
                cancellation,
                Some(buffer),
                budget.meter_mut(),
            );
            match inspected.result? {
                SubjectStep::Refuted => return Ok(true),
                SubjectStep::Changed => {}
                // The rows one at a time are at their fixed point; as a
                // linear system under the decisions they may still contradict.
                SubjectStep::Unchanged => {
                    return Ok(!system.probe_consistent(&self.region, &storage, budget)?);
                }
            }
        }
    }

    /// The forbidden table of `group` under its guard cut: every pattern over
    /// the support whose probe is refuted, and every pattern `known` refuted
    /// already, which runs no probe.
    ///
    /// # Errors
    /// As [`Self::run`].
    pub(super) fn table<Q: Quota + ?Sized>(
        &mut self,
        target: Target<'_>,
        rows: Rows<'_>,
        scratch: &mut NarrowingScratch,
        cancellation: &Cancellation,
        storage: &Storage,
        budget: &mut Budget<'_, '_, Q>,
    ) -> Result<Probed, BudgetError<Q::Error>> {
        let Target {
            group,
            known,
            forbidden,
        } = target;
        *forbidden = Words::<u64, _>::new([0; 4]);
        let (mut run, mut refuted) = (0, 0);
        let support = &group.support.atoms[..group.support.len];
        let mut storage = Storage::with_live_bytes(storage.bytes());
        let mut decisions = storage.reserve_bytes(support.len() + 1, budget)?;
        for pattern in 0..(1_usize << support.len()) {
            if known.contains(pattern) {
                forbidden.insert(pattern);
                refuted += 1;
                continue;
            }
            decisions.clear();
            decisions.push((group.guard, false));
            decisions.extend(
                support
                    .iter()
                    .enumerate()
                    .map(|(position, &atom)| (atom, pattern & (1 << position) != 0)),
            );
            let refuting = self.run(&decisions, rows, scratch, cancellation, &storage, budget)?;
            run += 1;
            if refuting {
                forbidden.insert(pattern);
                refuted += 1;
            }
        }
        Ok(Probed { run, refuted })
    }
}
