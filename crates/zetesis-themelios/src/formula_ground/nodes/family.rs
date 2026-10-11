//! Canonicalize a borrowed numeric family before publishing one shared row.

use super::{END, Index, Row, Rule, equal, numeric};
use crate::formula::ceiling;
use crate::{FormulaFailure, FormulaResource, ProgramSite};
use std::hash::BuildHasher;
use zetesis_ferraris::{
    AdmissionError, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaTransaction, NodeView,
    NumericDomain,
};

mod pending;
pub(in crate::formula_ground) use pending::Pending;

#[derive(Clone, Copy)]
pub(in crate::formula_ground) struct Entry {
    pub(in crate::formula_ground) rule: Rule,
    pub(in crate::formula_ground) node: usize,
    pub(in crate::formula_ground) inserted: bool,
    hash: u64,
}

impl Entry {
    pub(in crate::formula_ground) const fn new(rule: Rule) -> Self {
        Self {
            rule,
            node: 0,
            inserted: false,
            hash: 0,
        }
    }
}

/// All scratch is caller-owned and metered. `pending` has one cell per entry;
/// the initially empty open-addressing table has more cells than entries.
pub(in crate::formula_ground) struct Family<'a> {
    pub(in crate::formula_ground) row: Row<'a>,
    pub(in crate::formula_ground) entries: &'a mut [Entry],
    pub(in crate::formula_ground) pending: Pending<'a>,
    pub(in crate::formula_ground) table: &'a mut [Option<usize>],
    /// Successful complete row comparisons, keyed by the existing owner's row
    /// ID. More cells than entries suffice: each unique rule finds at most one.
    pub(in crate::formula_ground) matched_rows: &'a mut [Option<usize>],
}

/// Look up every rule before mutation, including within-batch duplicates. Full
/// row equality resolves collisions against the authoritative index. New rules
/// are appended together, then their already-reserved index entries publish with
/// no fallible work remaining. A refusal preserves both graph and index lengths.
///
/// Retained growth is O(E + G), with one row for all absent rules. Row hashing
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
        row,
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
        nodes.parts().occurrences() as u128 + row.conditions.len() as u128 * added as u128,
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
    let publication = row.conditions.len() as u128 * 3
        + added as u128 * 4
        + entries.len() as u128
        + growth as u128
        + 1
        + if matches!(row.domain, NumericDomain::Ordinal { .. }) {
            added as u128
        } else {
            0
        }
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
    let appended = pending
        .append(&mut transaction, row, added, bounds[0].1, bounds[1].1)
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
    let row = family.row;
    let entries = &mut *family.entries;
    let pending = &mut family.pending;
    let table = &mut *family.table;
    let matched_rows = &mut *family.matched_rows;
    assert_eq!(entries.len(), pending.len());
    assert!(table.len() > entries.len());
    assert!(matched_rows.len() > entries.len());
    if row.conditions.len() != row.values.len() {
        return Err(FormulaFailure::Theory {
            error: if row.domain == NumericDomain::Weights {
                AdmissionError::ThresholdShape
            } else {
                AdmissionError::NumericShape
            },
            location,
        });
    }
    let row_hash = row.hash(&index.hash, work)?;
    let first = nodes.view().len();
    let mut added = 0;
    for ordinal in 0..entries.len() {
        work()?;
        let rule = entries[ordinal].rule;
        let hash = numeric::hash(&index.hash, row_hash, rule, work)?;
        let mut slot = usize::try_from(hash % u64::try_from(table.len()).expect("table fits u64"))
            .expect("slot fits table");
        let duplicate = loop {
            work()?;
            match table[slot] {
                Some(previous) if entries[previous].rule == rule => break Some(previous),
                Some(_) => slot = (slot + 1) % table.len(),
                None => break None,
            }
        };
        if let Some(previous) = duplicate {
            entries[ordinal] = Entry {
                rule,
                node: entries[previous].node,
                inserted: false,
                hash,
            };
            continue;
        }
        let node = row
            .view(&rule)
            .map_err(|error| FormulaFailure::Theory { error, location })?;
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
            pending
                .set(added, rule)
                .map_err(|error| FormulaFailure::Theory { error, location })?;
            added += 1;
        }
        entries[ordinal] = Entry {
            rule,
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
    let (Some((_, left)), Some((_, right))) = (Row::read(old), Row::read(requested)) else {
        return Ok(false);
    };
    if left != right {
        return Ok(false);
    }
    let stored = nodes.parts().nodes()[existing];
    let row = if let Some(descriptor) = stored.threshold_descriptor() {
        nodes.parts().thresholds()[descriptor].row
    } else {
        let descriptor = stored.extremum_cut_descriptor().expect("numeric node");
        nodes.parts().extremum_cuts()[descriptor].row
    };
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
