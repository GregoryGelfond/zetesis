//! Extend selected base rows by the certified one-layer positive definitions.

mod join;

use std::fmt;
use zetesis_core::atom_interner::{AtomInterner, ClosedCatalog};
use zetesis_core::{Model, ModelFailure, UnificationError};
use zetesis_cpu::{Cancellation, Stop};

use super::{TerminalFormula, storage::Work};
use crate::FormulaFailure;
use crate::formula_support::{Counters, components};

/// Work and substitutions of one account of reconstruction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReconstructionCharges {
    /// Charged work.
    pub work: u64,
    /// Complete substitutions.
    pub substitutions: u64,
}

impl ReconstructionCharges {
    const fn saturating_sub(self, other: Self) -> Self {
        Self {
            work: self.work.saturating_sub(other.work),
            substitutions: self.substitutions.saturating_sub(other.substitutions),
        }
    }
    const fn saturating_add(self, other: Self) -> Self {
        Self {
            work: self.work.saturating_add(other.work),
            substitutions: self.substitutions.saturating_add(other.substitutions),
        }
    }
    fn max(self, other: Self) -> Self {
        Self {
            work: self.work.max(other.work),
            substitutions: self.substitutions.max(other.substitutions),
        }
    }
}

/// Reconstruction history for one session. Each call is bounded by the
/// per-answer `allowance`, fixed when the cursor is created; the totals are
/// reported only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReconstructionStatistics {
    /// Calls begun, including a refused call.
    pub attempts: u64,
    /// Complete interpretations published.
    pub completed: u64,
    /// Accepted source and reconstruction work; includes admission history
    /// and every call, for reporting.
    pub work: u64,
    /// Accepted source and reconstruction substitutions; includes admission
    /// and every call, for reporting.
    pub substitutions: u64,
    /// Charged by source admission; every call starts after it.
    pub admission: ReconstructionCharges,
    /// What each call may charge: the headroom the formula ceilings left
    /// after admission.
    pub allowance: ReconstructionCharges,
    /// The latest call's charge: on a refused session, what the refused call
    /// accepted; otherwise the answer delivered last.
    pub latest: ReconstructionCharges,
    /// The largest charge of any call, refused calls included, by component:
    /// how close an answer came to the allowance.
    pub peak: ReconstructionCharges,
}

/// A reconstruction refused without publishing a partial interpretation.
#[derive(Debug)]
pub enum ReconstructionError {
    /// The supplied interpretation belongs to a different base occurrence owner.
    ForeignInput,
    /// A prior call already refused; this reconstruction history is fused.
    Failed,
    /// Located source, work, storage, allocation or cancellation refusal.
    Source(Box<FormulaFailure>),
    /// The selected output could not be published.
    Model(ModelFailure<Box<FormulaFailure>>),
    /// A certified flat pattern did not match its retained frame shape.
    Unification(UnificationError),
    /// A certified head could not be instantiated by its complete body binding.
    Head(zetesis_core::InstantiationError),
}

impl ReconstructionError {
    /// State a work or substitution refusal of one call against its allowance:
    /// both the limit and the observed count less admission's charge.
    fn relative_to(self, admission: ReconstructionCharges) -> Self {
        let relative = |failure: Box<FormulaFailure>| match *failure {
            FormulaFailure::Limit {
                resource,
                limit,
                observed,
                location,
            } if matches!(
                resource,
                crate::FormulaResource::Work | crate::FormulaResource::Substitutions
            ) =>
            {
                let base = u128::from(if resource == crate::FormulaResource::Work {
                    admission.work
                } else {
                    admission.substitutions
                });
                Box::new(FormulaFailure::Limit {
                    resource,
                    limit: limit.saturating_sub(base),
                    observed: observed.saturating_sub(base),
                    location,
                })
            }
            other => Box::new(other),
        };
        match self {
            Self::Source(error) => Self::Source(relative(error)),
            Self::Model(ModelFailure::Stopped(error)) => {
                Self::Model(ModelFailure::Stopped(relative(error)))
            }
            other => other,
        }
    }

    fn retain_input(self, owner: &crate::formula_owner::Owner) -> Self {
        match self {
            Self::Source(error) => Self::Source(Box::new(owner.retain_failure(*error))),
            Self::Model(ModelFailure::Stopped(error)) => Self::Model(ModelFailure::Stopped(
                Box::new(owner.retain_failure(*error)),
            )),
            other => other,
        }
    }

    /// Cancellation or deadline, distinct from resource or structural refusal.
    #[must_use]
    pub fn stop(&self) -> Option<Stop> {
        match self {
            Self::Source(error) | Self::Model(ModelFailure::Stopped(error)) => error.interruption(),
            Self::ForeignInput
            | Self::Failed
            | Self::Model(ModelFailure::Model(_))
            | Self::Unification(_)
            | Self::Head(_) => None,
        }
    }
}
impl From<FormulaFailure> for ReconstructionError {
    fn from(error: FormulaFailure) -> Self {
        Self::Source(Box::new(error))
    }
}
impl fmt::Display for ReconstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignInput => {
                f.write_str("answer reconstruction received a different base program")
            }
            Self::Failed => f.write_str("answer reconstruction stopped after an earlier refusal"),
            Self::Source(error) => error.fmt(f),
            Self::Model(error) => error.fmt(f),
            Self::Unification(error) => error.fmt(f),
            Self::Head(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ReconstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignInput | Self::Failed => None,
            Self::Source(error) => Some(error.as_ref()),
            Self::Model(error) => Some(error),
            Self::Unification(error) => Some(error),
            Self::Head(error) => Some(error),
        }
    }
}

/// Independent cumulative extension cursor over an immutable admitted owner.
/// Scratch and selected derived rows are private to each call. Nothing from a
/// previous answer becomes true in a subsequent one through storage reuse.
pub struct TerminalReconstruction<'a> {
    owner: &'a TerminalFormula,
    admission: ReconstructionCharges,
    allowance: ReconstructionCharges,
    /// Admission and every call so far, for reporting.
    total: ReconstructionCharges,
    latest: ReconstructionCharges,
    peak: ReconstructionCharges,
    attempts: u64,
    completed: u64,
    failed: bool,
}

impl<'a> TerminalReconstruction<'a> {
    pub(super) fn new(owner: &'a TerminalFormula) -> Result<Self, ReconstructionError> {
        let prepared = &owner.0.extension;
        if prepared.closed.components.is_none() {
            return Err(owner
                .0
                .source
                .retain_failure(components::missing(prepared.location))
                .into());
        }
        let admission = ReconstructionCharges {
            work: prepared.baseline.work,
            substitutions: prepared.baseline.substitutions,
        };
        // The sizing: each answer may use the headroom admission left.
        let ceilings = ReconstructionCharges {
            work: prepared.limits.max_work,
            substitutions: prepared.limits.max_substitutions,
        };
        Ok(Self {
            owner,
            admission,
            allowance: ceilings.saturating_sub(admission),
            total: admission,
            latest: ReconstructionCharges::default(),
            peak: ReconstructionCharges::default(),
            attempts: 0,
            completed: 0,
            failed: false,
        })
    }

    /// Charges accepted through the latest completed or refused call.
    #[must_use]
    pub fn statistics(&self) -> ReconstructionStatistics {
        ReconstructionStatistics {
            attempts: self.attempts,
            completed: self.completed,
            work: self.total.work,
            substitutions: self.total.substitutions,
            admission: self.admission,
            allowance: self.allowance,
            latest: self.latest,
            peak: self.peak,
        }
    }

    /// Extend exactly this base interpretation by all certified definitions.
    /// The caller must separately establish that the input is a base answer set.
    /// The partition theorem then establishes membership for the returned model.
    /// Each call starts from admission's history and may charge the cursor's
    /// `allowance` of work and substitutions, whatever earlier answers used;
    /// a work or substitution refusal reports the allowance as its limit and
    /// the call's own charge as observed. Other ceilings are checked within
    /// the call as during admission. No full possible-support relation is
    /// enumerated during reconstruction.
    ///
    /// # Errors
    /// Refuses a foreign owner, cancellation, a resource boundary or invalid
    /// retained structure. Any refusal fuses this cursor. Earlier returned models
    /// remain valid and their capacities are owned by the consuming session.
    /// Logical source failures retain the original canonical program.
    pub fn reconstruct(
        &mut self,
        model: &Model,
        cancellation: &Cancellation,
    ) -> Result<Model, ReconstructionError> {
        if self.failed {
            return Err(ReconstructionError::Failed);
        }
        self.failed = true;
        self.attempts = self.attempts.checked_add(1).ok_or_else(|| {
            self.owner
                .0
                .source
                .retain_failure(super::storage::overflow(self.owner.0.extension.location))
        })?;
        let owner = self.owner;
        let prepared = &owner.0.extension;
        // Each answer starts from admission's history under its own allowance.
        let limits = crate::FormulaLimits {
            max_work: self.admission.work.saturating_add(self.allowance.work),
            max_substitutions: self
                .admission
                .substitutions
                .saturating_add(self.allowance.substitutions),
            ..prepared.limits
        };
        let mut accounting = prepared.baseline.start();
        let result = accounting.with_cancellation(cancellation, |counters| {
            extend(owner, model, counters, &limits)
        });
        self.latest = ReconstructionCharges {
            work: accounting.work,
            substitutions: accounting.substitutions,
        }
        .saturating_sub(self.admission);
        self.peak = self.peak.max(self.latest);
        self.total = self.total.saturating_add(self.latest);
        if result.is_ok() {
            self.completed += 1; // bounded by the checked attempts count
            self.failed = false;
        }
        result.map_err(|error| {
            error
                .relative_to(self.admission)
                .retain_input(&owner.0.source)
        })
    }
}

fn extend(
    owner: &TerminalFormula,
    model: &Model,
    counters: &mut Counters,
    limits: &crate::FormulaLimits,
) -> Result<Model, ReconstructionError> {
    let prepared = &owner.0.extension;
    counters.work(limits, prepared.location)?;
    if !model.catalog().same_owner(owner.base_atom_catalog()) {
        return Err(ReconstructionError::ForeignInput);
    }
    // The descendant writer counts the shared closed allocations. The separate
    // closed handle, component metadata, base output maps and input selection
    // remain external; no payload is charged twice merely because it is shared.
    let external = prepared.metadata_bytes
        + prepared.closed.metadata_bytes()
        + size_of::<ClosedCatalog>() as u128
        + model.selection_bytes()
        + size_of::<TerminalReconstruction<'_>>() as u128;
    let mut work = Work {
        limits,
        counters,
        location: prepared.location,
        external,
    };
    let mut writer = work.writer(&prepared.closed.storage)?;
    let components = prepared
        .closed
        .components
        .as_ref()
        .ok_or_else(|| components::missing(prepared.location))?
        .bind_with(prepared.closed.storage.vocabulary_read(), || work.permit())
        .map_err(|error| components::failure(error, limits, external, prepared.location))?;
    let mut atoms = model.atoms().iter();
    for _ in 0..atoms.len() {
        work.permit()?;
        let atom = atoms.next().expect("checked immutable model length");
        let limits = work.limits;
        let location = work.location;
        work.atoms(&mut writer, |writer, bound, counters| {
            writer
                .entry_atom_with(atom, bound, || counters.work(limits, location))?
                .insert_with(bound, || counters.work(limits, location))
        })?;
    }
    for rule in &prepared.deferred {
        work.location = rule.location;
        work.permit()?;
        join::derive(rule, components, model, &mut writer, &mut work)?;
    }
    work.location = prepared.location;
    publish(&mut writer, &mut work)
}

fn publish(writer: &mut AtomInterner, work: &mut Work<'_>) -> Result<Model, ReconstructionError> {
    let limits = work.limits;
    let location = work.location;
    work.atoms(writer, |writer, bound, counters| {
        writer.commit_with(bound, || counters.work(limits, location))
    })?;
    let order = work.atoms(writer, |writer, bound, counters| {
        writer.ordered_ids_with(bound, || counters.work(limits, location))
    })?;
    let mut order_lease = work.counters.lease();
    order_lease.observe(
        size_of_val(&order) + order.capacity() * size_of::<usize>(),
        location,
    )?;
    work.observe(writer.storage_bytes())?;
    let catalog = work.atoms(writer, |writer, bound, counters| {
        writer.publish_selection_with(&order, bound, || counters.work(limits, location))
    })?;
    let mut output_lease = work.counters.lease();
    output_lease.observe(
        usize::try_from(catalog.publication_bytes())
            .map_err(|_| super::storage::overflow(location))?,
        location,
    )?;
    drop(order);
    drop(order_lease);
    let remaining = work.remaining(writer.storage_bytes())?;
    let result =
        Model::publish_ordered_catalog_with(catalog, remaining, || work.permit().map_err(Box::new));
    let peak = match &result {
        Ok(model) => model.selection_bytes(),
        Err(error) => error.peak_bytes(),
    };
    let observed = work.observe(writer.storage_bytes() + peak);
    let model = result.map_err(|error| ReconstructionError::Model(error.into_parts().0))?;
    observed?;
    work.permit()?;
    Ok(model)
}
