//! Bounded test-only observation of exact, predicate-local posting intersections.
//!
//! The production probe remains authoritative. This instrument neither supplies
//! rows to the join nor charges its semantic resource counters. Its own refusal
//! leaves the grounding attempt running and marks the observation incomplete.

mod tests;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::mem::size_of;

use zetesis_core::catalog::{PredicateRef, TermRef};
use zetesis_core::{BindingView, PatternRef, Predicate, TemplateTerm, Value, ValueLimits};

use super::RelationRows;

#[derive(Clone, Copy, Debug)]
struct Limits {
    probes: usize,
    rows: usize,
    columns: usize,
    keys: usize,
    key_bytes: usize,
    work: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            probes: 100_000,
            rows: 65_536,
            columns: 64,
            keys: 16_384,
            key_bytes: 16 * 1024 * 1024,
            work: 100_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Probes,
    Rows,
    Columns,
    Keys,
    KeyBytes,
    Work,
    CountOverflow,
    ValueExport,
}

#[derive(Default, Debug)]
struct Totals {
    probes: usize,
    bound: usize,
    multiple: usize,
    shortest_rows: usize,
    intersection_rows: usize,
    reduced_probes: usize,
    repeated_probes: usize,
    repeated_bound_probes: usize,
    posting_comparisons: usize,
}

#[derive(Debug)]
struct Report {
    totals: Totals,
    stop: Option<Stop>,
    work: usize,
    keys: usize,
    key_bytes: usize,
    support_builds: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Query {
    // The formula path has one sequential support builder per observation.
    // This is its build ordinal, not an identity for arbitrarily interleaved
    // Support values. Predicate plus append-only row count identifies a snapshot.
    support: usize,
    predicate: Predicate,
    rows: usize,
    bound: Vec<(usize, Value)>,
}

struct Observation {
    limits: Limits,
    report: Report,
    queries: BTreeSet<Query>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}

/// The guard confines row identities to one grounding attempt and clears on unwind.
fn record<T>(limits: Limits, run: impl FnOnce() -> T) -> (T, Report) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|active| *active.borrow_mut() = None);
        }
    }
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        assert!(active.is_none(), "posting observation cannot nest");
        *active = Some(Observation {
            limits,
            report: Report {
                totals: Totals::default(),
                stop: None,
                work: 0,
                keys: 0,
                key_bytes: 0,
                support_builds: 0,
            },
            queries: BTreeSet::new(),
        });
    });
    let _reset = Reset;
    let result = run();
    let observation = ACTIVE.with(|active| active.borrow_mut().take().expect("active observation"));
    (result, observation.report)
}

pub(super) fn begin_support() {
    ACTIVE.with(|active| {
        if let Some(observation) = active.borrow_mut().as_mut()
            && observation.report.stop.is_none()
            && let Err(stop) = add(&mut observation.report.support_builds, 1)
        {
            observation.report.stop = Some(stop);
        }
    });
}

pub(super) fn observe(
    relation: &RelationRows,
    pattern: PatternRef<'_>,
    values: BindingView<'_>,
    selected: Option<&[usize]>,
) {
    ACTIVE.with(|active| {
        if let Some(observation) = active.borrow_mut().as_mut()
            && observation.report.stop.is_none()
            && let Err(stop) = observation.probe(relation, pattern, values, selected)
        {
            observation.report.stop = Some(stop);
        }
    });
}

impl Observation {
    fn probe(
        &mut self,
        relation: &RelationRows,
        pattern: PatternRef<'_>,
        values: BindingView<'_>,
        selected: Option<&[usize]>,
    ) -> Result<(), Stop> {
        if self.report.totals.probes == self.limits.probes {
            return Err(Stop::Probes);
        }
        if relation.atoms.len() > self.limits.rows {
            return Err(Stop::Rows);
        }
        if relation.column_count() > self.limits.columns {
            return Err(Stop::Columns);
        }
        let mut bound = Vec::new();
        let mut postings = Vec::new();
        for (position, term) in pattern.terms().into_iter().enumerate() {
            self.tick()?;
            let value = match term {
                TemplateTerm::Constant(value) => Some(value),
                TemplateTerm::Variable(variable) => values.get(variable),
            };
            if let Some(value) = value {
                bound.push((position, value));
                // The diagnostic derives the logical key from the original
                // atoms, independently of production dictionary lookup.
                let mut id = None;
                for (row, atom) in relation.atoms.iter().enumerate() {
                    self.tick()?;
                    if atom
                        .values()
                        .at(position)
                        .expect("column")
                        .compare_ref_with(value, || self.tick())?
                        .is_eq()
                    {
                        id = Some(relation.relation.column(position).expect("source column")[row]);
                        break;
                    }
                }
                // A column without postings offers none to choose among.
                if let super::relations::Postings::Indexed(column) = relation.postings(position) {
                    postings.push(
                        id.and_then(|id| column.get(&id))
                            .map_or(&[][..], Vec::as_slice),
                    );
                }
            }
        }
        // An unavailable restriction and a known-empty posting have different
        // meanings. The actual production selector must make that distinction.
        let shortest = postings.iter().min_by_key(|rows| rows.len()).copied();
        assert_eq!(selected, shortest);
        let mut cursor = Intersection::new(&postings);
        let mut actual = Vec::new();
        while let Some(row) = cursor.next(self)? {
            actual.push(row);
        }
        if postings.is_empty() {
            actual.extend(0..relation.atoms.len());
        }
        let mut expected = Vec::new();
        for (row, atom) in relation.atoms.iter().enumerate() {
            self.tick()?;
            let mut matches = true;
            for &(position, value) in &bound {
                self.tick()?;
                matches &= atom
                    .values()
                    .at(position)
                    .expect("column")
                    .compare_ref_with(value, || self.tick())?
                    .is_eq();
            }
            if matches {
                expected.push(row);
            }
        }
        assert_eq!(actual, expected, "same-snapshot full-row equality");
        let shortest_rows = selected.map_or(relation.atoms.len(), <[usize]>::len);
        self.query(pattern.predicate(), relation.atoms.len(), &bound)?;
        let totals = &mut self.report.totals;
        add(&mut totals.probes, 1)?;
        add(&mut totals.bound, usize::from(!postings.is_empty()))?;
        add(&mut totals.multiple, usize::from(postings.len() > 1))?;
        add(&mut totals.shortest_rows, shortest_rows)?;
        add(&mut totals.intersection_rows, actual.len())?;
        add(
            &mut totals.reduced_probes,
            usize::from(actual.len() < shortest_rows),
        )?;
        add(&mut totals.posting_comparisons, cursor.comparisons)?;
        Ok(())
    }

    fn query(
        &mut self,
        predicate: PredicateRef<'_>,
        rows: usize,
        bound: &[(usize, TermRef<'_>)],
    ) -> Result<(), Stop> {
        // These explicit exports belong only to this test observation. They do
        // not supply production rows or bindings, and have a separate byte cap.
        let mut bytes = predicate.name().len();
        let remaining = self.limits.key_bytes.saturating_sub(self.report.key_bytes);
        let mut exported = Vec::new();
        for &(position, value) in bound {
            self.tick()?;
            add(&mut bytes, size_of::<(usize, Value)>())?;
            let available = remaining.checked_sub(bytes).ok_or(Stop::KeyBytes)?;
            let value = value
                .to_value(ValueLimits {
                    max_nodes: usize::MAX,
                    max_depth: usize::MAX,
                    max_bytes: available,
                })
                .map_err(|error| match error {
                    zetesis_core::ValueError::Limit {
                        resource: zetesis_core::ValueResource::Bytes,
                        ..
                    } => Stop::KeyBytes,
                    _ => Stop::ValueExport,
                })?;
            add(&mut bytes, value.payload_bytes())?;
            if bytes > remaining {
                return Err(Stop::KeyBytes);
            }
            exported.push((position, value));
        }
        if bytes > remaining {
            return Err(Stop::KeyBytes);
        }
        let query = Query {
            support: self.report.support_builds,
            predicate: Predicate::with_sign(predicate.name(), predicate.arity(), predicate.sign())
                .expect("observed admitted predicate"),
            rows,
            bound: exported,
        };
        if self.queries.contains(&query) {
            add(&mut self.report.totals.repeated_probes, 1)?;
            add(
                &mut self.report.totals.repeated_bound_probes,
                usize::from(!bound.is_empty()),
            )?;
        } else {
            if self.queries.len() == self.limits.keys {
                return Err(Stop::Keys);
            }
            self.queries.insert(query);
            add(&mut self.report.key_bytes, bytes)?;
            self.report.keys = self.queries.len();
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Stop> {
        if self.report.work == self.limits.work {
            return Err(Stop::Work);
        }
        self.report.work += 1;
        Ok(())
    }
}

fn add(total: &mut usize, increment: usize) -> Result<(), Stop> {
    *total = total.checked_add(increment).ok_or(Stop::CountOverflow)?;
    Ok(())
}

/// Shortest-posting rows drive monotone cursors in every other posting.
/// Each supplied posting is strictly increasing and belongs to one relation.
/// Advancing a cursor cannot skip a future driver row because driver rows also
/// increase. Output stays ordered and contains a row iff every posting contains it.
struct Intersection<'a> {
    postings: &'a [&'a [usize]],
    positions: Vec<usize>,
    driver: Option<usize>,
    comparisons: usize,
}

impl<'a> Intersection<'a> {
    fn new(postings: &'a [&'a [usize]]) -> Self {
        Self {
            postings,
            positions: vec![0; postings.len()],
            driver: postings
                .iter()
                .enumerate()
                .min_by_key(|(_, rows)| rows.len())
                .map(|(index, _)| index),
            comparisons: 0,
        }
    }

    fn next(&mut self, observation: &mut Observation) -> Result<Option<usize>, Stop> {
        let Some(driver) = self.driver else {
            return Ok(None);
        };
        while let Some(&row) = self.postings[driver].get(self.positions[driver]) {
            observation.tick()?;
            self.positions[driver] += 1;
            let mut present = true;
            for (index, posting) in self.postings.iter().enumerate() {
                if index == driver {
                    continue;
                }
                loop {
                    observation.tick()?;
                    add(&mut self.comparisons, 1)?;
                    match posting.get(self.positions[index]) {
                        Some(&other) if other < row => self.positions[index] += 1,
                        Some(&other) => {
                            present &= other == row;
                            break;
                        }
                        None => return Ok(None),
                    }
                }
            }
            if present {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }
}
