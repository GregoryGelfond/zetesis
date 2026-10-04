//! Terminal definitions indexed by head signature and parsed origin.
//!
//! Correspondence stays certified by whole-rule matching; the index only
//! nominates candidates, so a shared origin never establishes a match. Each
//! index holds borrowed keys and definition positions in leased scratch, never
//! canonical payload. Building costs one step per definition, parsed origin and
//! head plus a bulk charge for sorting, n·⌈log₂(n + 1)⌉ steps charged before
//! the sort as a model of its comparisons rather than a count of them; each
//! lookup costs one step per probe.

use std::cmp::Ordering;

use themelios_base::span::Location;
use themelios_program::program::{Arguments, Statement};
use themelios_program::provenance::WithProvenance;
use zetesis_core::catalog::PredicateRef;

use super::{
    matching, reads,
    workspace::{Context, Scratch},
};
use crate::{FormulaFailure, ProgramSite};

pub(super) struct Defined<'d> {
    pub all: &'d [&'d WithProvenance<Statement>],
    /// Name and arity of each ordinary single-tuple head, ascending.
    signatures: Scratch<(&'d str, usize)>,
    /// Each parsed origin with the position of the definition carrying it,
    /// ascending; the definitions of one origin keep their program order.
    origins: Scratch<(Location, usize)>,
}

impl<'d> Defined<'d> {
    pub fn index(
        all: &'d [&'d WithProvenance<Statement>],
        context: &mut Context<'_, '_>,
    ) -> Result<Self, FormulaFailure> {
        let mut signatures = Scratch::new(context)?;
        signatures.reserve(all.len(), context)?;
        let mut count = 0_usize;
        for definition in all {
            context.work()?;
            count += crate::extended::parsed_origins(definition).len();
        }
        let mut origins = Scratch::new(context)?;
        origins.reserve(count, context)?;
        for (position, definition) in all.iter().enumerate() {
            context.work()?;
            if let Some(head) = matching::head(reads::source(definition))
                && let Arguments::Single(arguments) = &head.arguments
            {
                signatures.push((head.name.as_str(), arguments.len()), context)?;
            }
            for origin in crate::extended::parsed_origins(definition) {
                origins.push((origin, position), context)?;
            }
        }
        sort(&mut signatures.values, context)?;
        sort(&mut origins.values, context)?;
        Ok(Self {
            all,
            signatures,
            origins,
        })
    }

    /// Whether a definition's head has the predicate's name and arity, as
    /// `matching::signature` compares them with the sign ignored.
    pub fn defines(
        &self,
        predicate: PredicateRef<'_>,
        context: &mut Context<'_, '_>,
    ) -> Result<bool, FormulaFailure> {
        let key = (predicate.name(), predicate.arity());
        let (mut low, mut high) = (0, self.signatures.values.len());
        while low < high {
            context.work()?;
            let middle = low + (high - low) / 2;
            match self.signatures.values[middle].cmp(&key) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(true),
            }
        }
        Ok(false)
    }

    /// The index range of definitions carrying the site's parsed coordinate.
    /// Constructed sites have no parsed hint; whole-rule matching remains the
    /// independent correspondence check for every site.
    pub fn origin(
        &self,
        origin: ProgramSite,
        context: &mut Context<'_, '_>,
    ) -> Result<std::ops::Range<usize>, FormulaFailure> {
        let Some(origin) = origin.location() else {
            return Ok(0..0);
        };
        let entries = &self.origins.values;
        let start = partition(entries, context, |entry| entry.0 < origin)?;
        let end = partition(entries, context, |entry| entry.0 <= origin)?;
        Ok(start..end)
    }

    /// The definition position of the origin entry at `index`.
    pub fn position(&self, index: usize) -> usize {
        self.origins.values[index].1
    }

    /// Number of origin entries, for per-origin cursors.
    pub fn origin_entries(&self) -> usize {
        self.origins.values.len()
    }
}

/// Sort `values`, charging n·⌈log₂(n + 1)⌉ steps first as a model of the
/// comparisons.
pub(super) fn sort<T: Ord>(
    values: &mut [T],
    context: &mut Context<'_, '_>,
) -> Result<(), FormulaFailure> {
    let count = values.len() as u128;
    let depth = u128::from(usize::BITS - values.len().leading_zeros());
    context.charge(count * depth)?;
    values.sort_unstable();
    Ok(())
}

/// The first index whose value is not `before`, one step per probe.
pub(super) fn partition<T>(
    values: &[T],
    context: &mut Context<'_, '_>,
    before: impl Fn(&T) -> bool,
) -> Result<usize, FormulaFailure> {
    let (mut low, mut high) = (0, values.len());
    while low < high {
        context.work()?;
        let middle = low + (high - low) / 2;
        if before(&values[middle]) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Ok(low)
}
