//! Integer hulls of complete source columns, kept across support rounds.
//!
//! A column's hull is the closed range of its numeric values. It is declined
//! for an empty or unindexed column, and for one holding a nonnumeric value,
//! exactly as the totality module's `column_bounds` reads it. A query reads the column's
//! distinct values, O(d). Support completion alone keeps the hulls, from one
//! round to the next. Rows are only appended and keep their
//! values, so a kept hull covers the rows it has read in every later snapshot,
//! and a later round reads only its Δ appended rows, O(Δ). A declined hull stays
//! declined. Space is O(1) for each column read.

use std::cell::RefCell;
use std::mem::size_of;

use zetesis_core::ValueNodeRef;

use super::expression_keys::tick;
use super::signature::{Signature, signature};
use super::{RelationRows, Support};
use crate::FormulaFailure;
use crate::ProgramSite;
use crate::formula_support::relations::Postings;
use crate::formula_support::{Computation, Context, StorageLease, reserve};
use crate::scalar_arithmetic::Range;

/// The support owner counts this header; its lease counts the entry vector.
#[derive(Default)]
pub(in crate::formula_support) struct Workspace<'source> {
    cache: RefCell<Option<Cache<'source>>>,
}

/// Hulls kept between rounds of one support completion, holding no snapshot
/// borrow; the next round's queries find each again by relation signature.
#[derive(Default)]
pub(in crate::formula_support) struct Retained(Option<Cache<'static>>);

struct Cache<'source> {
    entries: Vec<Entry<'source>>,
    lease: StorageLease,
}

struct Entry<'source> {
    /// The source in this snapshot, once a query here has named its relation.
    source: Option<&'source RelationRows<'source>>,
    relation: Signature,
    column: usize,
    /// The hull covers the source rows before this position.
    covered: usize,
    hull: Hull,
}

#[derive(Clone, Copy)]
enum Hull {
    Empty,
    Bounded(Range),
    /// A nonnumeric value was read; no later row can repair it.
    Declined,
}

impl Hull {
    fn include(&mut self, value: ValueNodeRef<'_>) {
        *self = match (*self, value) {
            (Self::Empty, ValueNodeRef::Number(value)) => Self::Bounded(Range::point(value)),
            (Self::Bounded(mut range), ValueNodeRef::Number(value)) => {
                range.include(value);
                Self::Bounded(range)
            }
            _ => Self::Declined,
        };
    }

    fn range(self) -> Option<Range> {
        match self {
            Self::Bounded(range) => Some(range),
            Self::Empty | Self::Declined => None,
        }
    }
}

impl<'source> Support<'source> {
    /// Every numeric value in one complete column of this snapshot as one
    /// closed range, or a conservative decline for empty, mixed or unindexed
    /// columns: the same answer the totality module's `column_bounds` gives. No binding
    /// existence follows. A hull kept from an earlier round reads only the
    /// rows appended since.
    pub(in crate::formula_support) fn column_hull(
        &self,
        rows: &'source RelationRows<'source>,
        column: usize,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Range>, FormulaFailure> {
        tick(context)?;
        let Postings::Indexed(postings) = rows.postings(column) else {
            return Ok(None);
        };
        let Some(hulls) = &self.hulls else {
            return Ok(distinct(rows, column, postings, context)?.range());
        };
        if !self.workspace().owns(&context.computation.lease()) {
            return Err(crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(
                    zetesis_core::catalog::ReadError::ForeignCatalog,
                ),
                context.work.location,
            ));
        }
        let mut cache = hulls.cache.borrow_mut();
        if let Some(cache) = cache.as_mut() {
            for entry in &mut cache.entries {
                tick(context)?;
                let same_source = match entry.source {
                    Some(resolved) => std::ptr::eq(resolved, rows),
                    None => entry.relation.names(rows.relation.predicate(), context)?,
                };
                if same_source && entry.column == column {
                    entry.source = Some(rows);
                    extend(entry, rows, context)?;
                    return Ok(entry.hull.range());
                }
            }
        }
        let hull = distinct(rows, column, postings, context)?;
        let relation = signature(rows.relation.predicate(), context)?;
        if cache.is_none() {
            *cache = Some(Cache {
                entries: Vec::new(),
                lease: context.computation.lease(),
            });
        }
        let cache = cache.as_mut().expect("the query owns its cache");
        reserve(
            &mut cache.entries,
            1,
            &mut cache.lease,
            0,
            Context::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
        )?;
        cache.entries.push(Entry {
            source: Some(rows),
            relation,
            column,
            covered: rows.row_count(),
            hull,
        });
        Ok(hull.range())
    }
}

/// The hull of a column read through its distinct values, once each.
fn distinct(
    rows: &RelationRows<'_>,
    column: usize,
    postings: &std::collections::BTreeMap<u32, Vec<usize>>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Hull, FormulaFailure> {
    let mut hull = Hull::Empty;
    for posting in postings.values() {
        tick(context)?;
        let position = *posting.first().expect("a distinct input has a source row");
        let value = rows
            .row(position)
            .expect("retained source row")
            .value(column)
            .expect("checked source column");
        hull.include(value.descriptor());
        if matches!(hull, Hull::Declined) {
            break;
        }
    }
    Ok(hull)
}

/// Read the rows appended since the hull was last read.
fn extend(
    entry: &mut Entry<'_>,
    rows: &RelationRows<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    let count = rows.row_count();
    debug_assert!(entry.covered <= count, "support rows are only appended");
    while entry.covered < count && !matches!(entry.hull, Hull::Declined) {
        tick(context)?;
        let value = rows
            .row(entry.covered)
            .expect("source row count bounds the hull")
            .value(entry.column)
            .expect("checked source column");
        entry.hull.include(value.descriptor());
        entry.covered += 1;
    }
    entry.covered = count;
    Ok(())
}

impl Workspace<'_> {
    /// Keep this snapshot's hulls for the next round of the same support.
    pub(in crate::formula_support) fn retain(
        self,
        location: ProgramSite,
    ) -> Result<Retained, FormulaFailure> {
        let Some(Cache { entries, mut lease }) = self.cache.into_inner() else {
            return Ok(Retained::default());
        };
        let entries: Vec<Entry<'static>> = entries.into_iter().map(Entry::unresolved).collect();
        lease.observe(entries.capacity() * size_of::<Entry<'_>>(), location)?;
        Ok(Retained(Some(Cache { entries, lease })))
    }
}

impl Retained {
    /// Lend the kept hulls to the next snapshot's queries.
    pub(in crate::formula_support) fn into_workspace<'source>(self) -> Workspace<'source> {
        Workspace {
            cache: RefCell::new(self.0.map(|Cache { entries, lease }| Cache {
                entries: entries.into_iter().map(Entry::unresolved).collect(),
                lease,
            })),
        }
    }
}

impl Entry<'_> {
    /// The same hull with no snapshot borrow: a later query resolves it.
    fn unresolved<'next>(self) -> Entry<'next> {
        let Entry {
            source: _,
            relation,
            column,
            covered,
            hull,
        } = self;
        Entry {
            source: None,
            relation,
            column,
            covered,
            hull,
        }
    }
}
