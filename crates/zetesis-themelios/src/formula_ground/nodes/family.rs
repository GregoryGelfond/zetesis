//! Canonicalize a borrowed threshold family before publishing one shared row.

use super::{END, Index, equal, row_hash, threshold_hash};
use crate::formula::ceiling;
use crate::{FormulaFailure, FormulaResource, ProgramSite};
use std::hash::BuildHasher;
use zetesis_ferraris::{AdmissionError, FormulaNodes, NodeView, Threshold};

#[derive(Clone, Copy)]
pub(in crate::formula_ground) struct Entry {
    pub(in crate::formula_ground) bound: i128,
    pub(in crate::formula_ground) node: usize,
    pub(in crate::formula_ground) inserted: bool,
    hash: u64,
}

impl Entry {
    pub(in crate::formula_ground) const fn new(bound: i128) -> Self {
        Self {
            bound,
            node: 0,
            inserted: false,
            hash: 0,
        }
    }
}

/// All scratch is caller-owned and metered. `pending` has one cell per entry;
/// the initially empty open-addressing table has more cells than entries.
pub(in crate::formula_ground) struct Family<'a> {
    pub(in crate::formula_ground) conditions: &'a [usize],
    pub(in crate::formula_ground) weights: &'a [u64],
    pub(in crate::formula_ground) entries: &'a mut [Entry],
    pub(in crate::formula_ground) pending: &'a mut [i128],
    pub(in crate::formula_ground) table: &'a mut [Option<usize>],
    /// Successful complete row comparisons, keyed by the existing owner's row
    /// ID. More cells than entries suffice: each unique bound finds at most one.
    pub(in crate::formula_ground) matched_rows: &'a mut [Option<usize>],
}

/// Look up every bound before mutation, including within-batch duplicates. Full
/// row equality resolves collisions against the authoritative index. New bounds
/// are appended together, then their already-reserved index entries publish with
/// no fallible work remaining. A refusal preserves both graph and index lengths.
///
/// Retained growth is O(E + G), with one row for all absent bounds. Row hashing
/// costs O(E), then O(G) expected probes; complete collision checks retain their
/// charged worst-case cost. Any pending raw-prefix numeric validation is charged
/// before the append that establishes and caches its validity.
pub(in crate::formula_ground) fn intern<S: BuildHasher>(
    index: &mut Index<S>,
    nodes: &mut FormulaNodes,
    mut family: Family<'_>,
    bounds: [(FormulaResource, usize); 2],
    location: ProgramSite,
    mut work: impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<(), FormulaFailure> {
    let added = prepare(index, nodes, &mut family, location, &mut work)?;
    let Family {
        conditions,
        weights,
        entries,
        pending,
        ..
    } = family;
    let first = nodes.view().len();
    if added == 0 {
        return Ok(());
    }
    ceiling(
        bounds[0].0,
        first as u128 + added as u128,
        bounds[0].1 as u128,
        location,
    )?;
    ceiling(
        bounds[1].0,
        nodes.parts().occurrences() as u128 + conditions.len() as u128 * added as u128,
        bounds[1].1 as u128,
        location,
    )?;
    let growth = if added > index.links.capacity() - index.links.len() {
        index.links.len()
    } else {
        0
    } + if added > index.buckets.capacity() - index.buckets.len() {
        index.buckets.len()
    } else {
        0
    };
    // Validate and copy both columns once; publish each descriptor/node and
    // index link/bucket once. Charge every final scan before graph publication.
    let publication = conditions.len() as u128 * 3
        + added as u128 * 4
        + entries.len() as u128
        + growth as u128
        + 1
        + nodes.parts().pending_numeric_validation_work();
    for _ in 0..publication {
        work()?;
    }
    index
        .links
        .try_reserve(added)
        .map_err(|error| FormulaFailure::MetadataAllocation { error, location })?;
    index
        .buckets
        .try_reserve(added)
        .map_err(|error| FormulaFailure::MetadataAllocation { error, location })?;
    let mut transaction = nodes.transaction();
    let appended = transaction
        .push_threshold_family_range(
            conditions,
            weights,
            &pending[..added],
            bounds[0].1,
            bounds[1].1,
        )
        .map_err(|error| FormulaFailure::Theory { error, location })?;
    debug_assert_eq!(appended, first..first + added);
    transaction.commit();
    for entry in entries {
        if entry.inserted {
            let previous = index.buckets.get(&entry.hash).copied().unwrap_or(END);
            debug_assert_eq!(index.links.len(), entry.node);
            index.links.push(previous);
            index.buckets.insert(entry.hash, entry.node);
        }
    }
    Ok(())
}

fn prepare<S: BuildHasher>(
    index: &Index<S>,
    nodes: &FormulaNodes,
    family: &mut Family<'_>,
    location: ProgramSite,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<usize, FormulaFailure> {
    let conditions = family.conditions;
    let weights = family.weights;
    let entries = &mut *family.entries;
    let pending = &mut *family.pending;
    let table = &mut *family.table;
    let matched_rows = &mut *family.matched_rows;
    assert_eq!(entries.len(), pending.len());
    assert!(table.len() > entries.len());
    assert!(matched_rows.len() > entries.len());
    if conditions.len() != weights.len() {
        return Err(FormulaFailure::Theory {
            error: AdmissionError::ThresholdShape,
            location,
        });
    }
    let row_hash = row_hash(&index.hash, conditions, weights, work)?;
    let first = nodes.view().len();
    let mut added = 0;
    for ordinal in 0..entries.len() {
        work()?;
        let bound = entries[ordinal].bound;
        let hash = threshold_hash(&index.hash, row_hash, bound, work)?;
        let mut slot = usize::try_from(hash % u64::try_from(table.len()).expect("table fits u64"))
            .expect("slot fits table");
        let duplicate = loop {
            work()?;
            match table[slot] {
                Some(previous) if entries[previous].bound == bound => break Some(previous),
                Some(_) => slot = (slot + 1) % table.len(),
                None => break None,
            }
        };
        if let Some(previous) = duplicate {
            entries[ordinal] = Entry {
                bound,
                node: entries[previous].node,
                inserted: false,
                hash,
            };
            continue;
        }
        let node = NodeView::Threshold(Threshold {
            conditions,
            weights,
            bound,
        });
        let mut cursor = index.buckets.get(&hash).copied().unwrap_or(END);
        while cursor != END {
            work()?;
            if matches_existing(nodes, cursor, node, matched_rows, location, work)? {
                break;
            }
            cursor = index.links[cursor];
        }
        let inserted = cursor == END;
        if inserted {
            cursor = first.checked_add(added).ok_or(FormulaFailure::Theory {
                error: AdmissionError::Limit,
                location,
            })?;
            pending[added] = bound;
            added += 1;
        }
        entries[ordinal] = Entry {
            bound,
            node: cursor,
            inserted,
            hash,
        };
        table[slot] = Some(ordinal);
    }
    Ok(added)
}

fn matches_existing(
    nodes: &FormulaNodes,
    existing: usize,
    requested: NodeView<'_>,
    matched_rows: &mut [Option<usize>],
    location: ProgramSite,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<bool, FormulaFailure> {
    let old = nodes
        .view()
        .node(existing)
        .map_err(|error| FormulaFailure::Theory { error, location })?;
    let (NodeView::Threshold(left), NodeView::Threshold(right)) = (old, requested) else {
        return Ok(false);
    };
    if left.bound != right.bound {
        return Ok(false);
    }
    let descriptor = nodes.parts().nodes()[existing]
        .threshold_descriptor()
        .expect("threshold node");
    let row = nodes.parts().thresholds()[descriptor].row;
    let mut slot = row % matched_rows.len();
    loop {
        work()?;
        match matched_rows[slot] {
            Some(previous) if previous == row => return Ok(true),
            Some(_) => slot = (slot + 1) % matched_rows.len(),
            None => break,
        }
    }
    if !equal(old, requested, work)? {
        return Ok(false);
    }
    matched_rows[slot] = Some(row);
    Ok(true)
}

#[cfg(test)]
mod tests;
