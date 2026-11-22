//! Workers intern staged rows' head keys; the coordinator publishes each head once.
//!
//! A support plan's head projects fixed relation columns of the staged rows
//! (`RowHead`). Two staged rows with equal projected values denote the same head
//! atom, so after the first of them is admitted and retained the rest change
//! nothing: admission finds the existing atom and retention is idempotent. Each
//! worker therefore interns a row's key, the projected columns' equality
//! identifiers, into one shared [`ContentInterner`] through `&self`, and stages
//! only the provisional coordinate next to the row's occurrence. Once every task
//! settles, the coordinator freezes that one generation (on the grounding pool,
//! so never wider than `--threads`) and resolves each coordinate with
//! [`Generation::finalize`] in staged order: the first occurrence of an identity
//! takes the ordinary `RowHead` path, later ones are skipped. Atom discovery and
//! support order are those of the serial projection.
//!
//! Provisional coordinates live only in a batch's key buffer and die with the
//! publication; final identities index only the publication's local seen set.
//! Neither reaches a catalog column, a solver variable or a device buffer.
//!
//! Work is deterministic at every worker count: a worker charges a fixed amount
//! per key, independent of which task interned the content first, and the
//! freeze's own counts depend only on the generation's content.
//! `proofs/Zetesis/HeadKeyPublication.lean` states the skip law.

use zetesis_core::TemplateTerm;
use zetesis_core::atom_registry::RowColumn;
use zetesis_core::intern::{ContentInterner, Generation, Handle, Pending, Words, Work};

use super::{
    Computation, Context, Counters, FormulaFailure, GroundingWork, StorageLease, WitnessPlan,
    WitnessRow,
};
use crate::formula_ir::HeadIr;
use crate::formula_support::heads::row_column;

/// The shared interner of one support publication's head keys.
pub(crate) struct HeadKeys {
    interner: ContentInterner<Words>,
}

impl HeadKeys {
    pub(crate) fn new() -> Self {
        Self {
            interner: ContentInterner::new(Words),
        }
    }

    /// End the generation after every task settled, on the grounding pool,
    /// and charge its deterministic work. `None` when the interner or the seen
    /// set's reservation refused: every row then takes the ordinary path.
    pub(crate) fn seen(
        &mut self,
        pool: Option<&zetesis_algebra::threads::Pool>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Seen>, FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let mut work = Work::default();
        let generation = self.interner.freeze(pool, &mut work);
        counters.charge_work(u128::from(work.total()), limits, location)?;
        let Ok(generation) = generation else {
            return Ok(None);
        };
        let words = self.interner.len().div_ceil(64);
        let mut marks = Vec::new();
        if marks.try_reserve_exact(words).is_err() {
            return Ok(None);
        }
        marks.resize(words, 0);
        let bytes = self.interner.bytes() + (marks.capacity() * size_of::<u64>()) as u128;
        let bytes = usize::try_from(bytes).map_err(|_| FormulaFailure::Limit {
            resource: crate::FormulaResource::SupportBytes,
            observed: bytes,
            limit: limits.max_support_bytes as u128,
            location,
        })?;
        let mut lease = computation.lease();
        lease.observe(bytes, location)?;
        computation.storage_observed(&lease, 0, bytes, limits, counters, location)?;
        Ok(Some(Seen {
            generation,
            marks,
            _lease: lease,
        }))
    }
}

/// One worker's key projection, prepared at its first staged row.
pub(super) struct Keyer<'keys> {
    keys: &'keys HeadKeys,
    slots: Slots,
    scratch: Vec<u32>,
}

enum Slots {
    /// Before the first staged row.
    Unprepared,
    /// The head's projected columns.
    Keyed(Vec<RowColumn>),
    /// This head cannot be keyed: its rows take the ordinary path.
    Unkeyed,
}

impl<'keys> Keyer<'keys> {
    pub(super) fn new(keys: &'keys HeadKeys) -> Self {
        Self {
            keys,
            slots: Slots::Unprepared,
            scratch: Vec::new(),
        }
    }

    /// The row's provisional head coordinate, or `None` when this row (and so
    /// its batch) must take the ordinary path.
    pub(super) fn key(
        &mut self,
        plan: &WitnessPlan<'_, '_>,
        row: &WitnessRow<'_, '_>,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Pending>, FormulaFailure> {
        let Context {
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
            ..
        } = context;
        if matches!(self.slots, Slots::Unprepared) {
            self.slots =
                slots(plan, row, limits, counters, location)?.map_or(Slots::Unkeyed, Slots::Keyed);
        }
        let Slots::Keyed(slots) = &self.slots else {
            return Ok(None);
        };
        counters.charge_work(slots.len() as u128 + 2, limits, location)?;
        self.scratch.clear();
        if self.scratch.try_reserve(slots.len()).is_err() {
            return Ok(None);
        }
        for slot in slots {
            let (Some(matched), Some(source)) = (
                row.rows.get(slot.input),
                plan.sources
                    .slice()
                    .get(slot.input)
                    .and_then(|resolution| resolution.rows()),
            ) else {
                return Ok(None);
            };
            // Equality identifiers are meaningful within one relation only.
            if !matched.belongs_to(&source.relation) {
                return Ok(None);
            }
            let Some(id) = matched.equality_id(slot.column) else {
                return Ok(None);
            };
            self.scratch.push(id);
        }
        let mut work = Work::default();
        Ok(match self.keys.interner.intern(&self.scratch, &mut work) {
            Ok(Handle::Pending(pending)) => Some(pending),
            // The interner is fresh per publication: nothing is final yet.
            Ok(Handle::Final(_)) | Err(_) => None,
        })
    }
}

/// The relation column of each head variable, in head argument order, as
/// `RowHead` projects them; constants contribute nothing to the key.
fn slots(
    plan: &WitnessPlan<'_, '_>,
    row: &WitnessRow<'_, '_>,
    limits: &crate::FormulaLimits,
    counters: &mut Counters,
    location: crate::ProgramSite,
) -> Result<Option<Vec<RowColumn>>, FormulaFailure> {
    let HeadIr::Normal(Some(pattern)) = plan.base.rule.head else {
        return Ok(None);
    };
    let Some(components) = row.support.components() else {
        return Ok(None);
    };
    let pattern = pattern.get(components, limits, counters, location)?;
    let terms = pattern.terms();
    let mut slots = Vec::new();
    if slots.try_reserve(terms.len()).is_err() {
        return Ok(None);
    }
    for index in 0..terms.len() {
        counters.work(limits, location)?;
        match terms.at(index) {
            Some(TemplateTerm::Constant(_)) => {}
            Some(TemplateTerm::Variable(variable)) => {
                match row_column(variable, row, limits, counters, location)? {
                    Some(column) => slots.push(column),
                    None => return Ok(None),
                }
            }
            None => return Ok(None),
        }
    }
    Ok(Some(slots))
}

/// The coordinator's view of one frozen publication: whether a staged row is
/// the first occurrence of its head.
pub(crate) struct Seen {
    generation: Generation,
    marks: Vec<u64>,
    /// The frozen interner and this set, observed while publication runs.
    _lease: StorageLease,
}

impl Seen {
    /// Mark `pending`'s identity; `true` if this is its first occurrence.
    pub(super) fn first(&mut self, pending: Pending) -> bool {
        let index = self.generation.resolve(pending).index() - self.generation.base();
        let (word, bit) = (index / 64, 1u64 << (index % 64));
        let first = self.marks[word] & bit == 0;
        self.marks[word] |= bit;
        first
    }
}
