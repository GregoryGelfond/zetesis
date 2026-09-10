//! Shared finite typed relations for scalar and device equality experiments.
//!
//! These fixtures supply construction, lookup, equality selection and typed
//! row reconstruction experiments. They do not measure full pattern matching, source
//! grounding or solving. Every route receives the same immutable source rows
//! and ordered typed query occurrences; equality IDs are not fixture identity.

use std::{fmt, mem::size_of};

use zetesis_core::{Atom, ConstructionError, Predicate, Value, ValueError, ValueLimits, ValueNode};

const ARITY: usize = 3;
const MAX_ROWS: usize = 65_536;
const MAX_QUERIES: usize = 256;
const VALUE_CONSTRUCTION_BYTES: usize = 1_024;
const GENERATOR_VERSION: u32 = 1;
const QUERY_PERIOD: usize = 8;
const QUERY_STRIDE: usize = 17;
const SKEW_PERIOD: usize = 16;

/// Generic equality distributions, independent of an ASP encoding name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Family {
    /// Every query has at most one usable column equality.
    Single,
    /// The first two columns vary independently on a rectangular grid.
    Independent,
    /// The first two columns have identical values.
    Correlated,
    /// Most rows share one first-column value; the rest have sparse keys.
    Skewed,
}

/// The whole typed value stored in each argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Payload {
    /// Checked-width scalar integers.
    Numeric,
    /// Unary tuples containing the corresponding integer.
    Tuple,
}

/// Reproducible generator inputs; row/query order is part of the subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specification {
    /// Version of the deterministic generator defined by this module.
    pub version: u32,
    /// Equality distribution.
    pub family: Family,
    /// Scalar or structural logical payload.
    pub payload: Payload,
    /// Source row count, including every occurrence.
    pub rows: usize,
    /// Ordered query occurrence count, including repeats.
    pub queries: usize,
}

/// Inclusive ceilings for one fixture's authored storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Accounted live vector capacity, predicate text and logical payload bytes.
    ///
    /// Includes bounded value-construction scratch. Allocation metadata, `Arc`
    /// control blocks, other caller frames and process RSS are separate.
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 134_217_728,
        }
    }
}

/// Authored storage, separate from column/device views and process RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storage {
    /// Fixture, source/query vector capacities, predicate text and value payload.
    pub retained_bytes: usize,
    /// Largest accounted live storage, including value construction scratch.
    pub peak_construction_bytes: usize,
    /// Complete source argument cells.
    pub source_cells: usize,
    /// Complete query equality cells, including repeated query occurrences.
    pub equalities: usize,
}

/// A bounded fixture or independent reference could not be completed.
#[derive(Debug)]
pub enum Error {
    /// Rows must be within 1..=65536 and query count within 0..=256.
    Dimensions,
    /// A query index is outside this fixture's ordered query population.
    Query,
    /// The independent reference exceeds its output position ceiling.
    Positions,
    /// Authored live storage exceeds its inclusive byte ceiling.
    Bytes,
    /// A shape/capacity cannot be represented.
    Overflow,
    /// Storage could not be reserved.
    Allocation,
    /// A typed source atom could not be constructed.
    Atom(ConstructionError),
    /// A bounded structural value could not be constructed.
    Value(ValueError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dimensions => {
                f.write_str("relation fixture dimensions are outside finite bounds")
            }
            Self::Query => f.write_str("relation fixture query is out of range"),
            Self::Positions => f.write_str("relation reference exceeds its position ceiling"),
            Self::Bytes => f.write_str("relation fixture exceeds its live byte ceiling"),
            Self::Overflow => {
                f.write_str("relation fixture shape or capacity is not representable")
            }
            Self::Allocation => f.write_str("relation fixture storage could not be reserved"),
            Self::Atom(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// One authoritative synthetic atom owner with immutable typed query keys.
pub struct Fixture {
    specification: Specification,
    predicate: Predicate,
    atoms: Vec<Atom>,
    queries: Vec<Vec<(usize, Value)>>,
    storage: Storage,
}

impl Fixture {
    /// Construct deterministic row/query populations under finite ceilings.
    ///
    /// A unique third argument preserves extensional atom uniqueness even when
    /// the first two columns repeat. Queries never select by that identity
    /// argument. Each eight-query period includes empty and missing-value
    /// controls; other occurrences use reproducible present-row keys.
    ///
    /// # Errors
    /// Refuses dimensions, capacity excess, allocation or typed construction.
    pub fn new(
        family: Family,
        payload: Payload,
        rows: usize,
        queries: usize,
        limits: Limits,
    ) -> Result<Self, Error> {
        if !(1..=MAX_ROWS).contains(&rows) || queries > MAX_QUERIES {
            return Err(Error::Dimensions);
        }
        let specification = Specification {
            version: GENERATOR_VERSION,
            family,
            payload,
            rows,
            queries,
        };
        let name = "row";
        let mut budget = Budget::new(limits, size_of::<Self>() + name.len())?;
        let predicate = Predicate::new(name, ARITY).map_err(Error::Atom)?;
        let atoms = source(specification, &predicate, &mut budget)?;
        let query_rows = query_rows(specification, &mut budget)?;
        let storage = Storage {
            retained_bytes: budget.live,
            peak_construction_bytes: budget.peak,
            source_cells: rows * ARITY,
            equalities: query_rows.iter().map(Vec::len).sum(),
        };
        Ok(Self {
            specification,
            predicate,
            atoms,
            queries: query_rows,
            storage,
        })
    }

    /// Exact generator parameters; neither a content hash nor an owner token.
    #[must_use]
    pub const fn specification(&self) -> Specification {
        self.specification
    }

    /// Full signed source predicate.
    #[must_use]
    pub fn predicate(&self) -> &Predicate {
        &self.predicate
    }

    /// The immutable original source rows in occurrence order.
    #[must_use]
    pub fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    /// Whole typed equality keys, preserving columns, order and repetition.
    #[must_use]
    pub fn queries(&self) -> &[Vec<(usize, Value)>] {
        &self.queries
    }

    /// Fixture-owned capacity; relation/device/query views add their own cost.
    #[must_use]
    pub const fn storage(&self) -> Storage {
        self.storage
    }

    /// Independently compare original typed row values for one complete query.
    ///
    /// The result is the increasing source-position sequence. This full-row
    /// equality reference is not a pattern matcher or a shortest-posting probe.
    /// The caller accounts its returned vector separately from fixture storage.
    ///
    /// # Errors
    /// Refuses a foreign query index, output excess or allocation failure.
    pub fn reference_positions(
        &self,
        query: usize,
        max_positions: usize,
    ) -> Result<Vec<usize>, Error> {
        let keys = self.queries.get(query).ok_or(Error::Query)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(self.atoms.len().min(max_positions))
            .map_err(|_| Error::Allocation)?;
        for (position, atom) in self.atoms.iter().enumerate() {
            if keys
                .iter()
                .all(|(column, value)| atom.values().get(*column) == Some(value))
            {
                if positions.len() == max_positions {
                    return Err(Error::Positions);
                }
                positions.push(position);
            }
        }
        Ok(positions)
    }
}

fn source(
    specification: Specification,
    predicate: &Predicate,
    budget: &mut Budget,
) -> Result<Vec<Atom>, Error> {
    let mut atoms = budget.reserve(specification.rows)?;
    for row in 0..specification.rows {
        let mut values = budget.reserve(ARITY)?;
        for value in row_values(specification, row) {
            values.push(budget.value(specification.payload, value)?);
        }
        budget.add(predicate.name().len())?;
        atoms.push(Atom::new(predicate.clone(), values).map_err(Error::Atom)?);
    }
    Ok(atoms)
}

fn row_values(specification: Specification, row: usize) -> [usize; ARITY] {
    let width = specification.rows.isqrt().max(1);
    let first = row % width;
    let second = row / width;
    match specification.family {
        Family::Single | Family::Independent => [first, second, row],
        Family::Correlated => [first, first, row],
        Family::Skewed => [
            if row.is_multiple_of(SKEW_PERIOD) {
                row + 1
            } else {
                0
            },
            first,
            row,
        ],
    }
}

fn query_rows(
    specification: Specification,
    budget: &mut Budget,
) -> Result<Vec<Vec<(usize, Value)>>, Error> {
    let mut queries = budget.reserve(specification.queries)?;
    for query in 0..specification.queries {
        let columns: &[usize] = match query % QUERY_PERIOD {
            0 => &[],
            1 => &[0],
            _ if specification.family == Family::Single => &[0],
            2 => &[1],
            _ => &[0, 1],
        };
        let values = row_values(specification, (query * QUERY_STRIDE) % specification.rows);
        let mut keys = budget.reserve(columns.len())?;
        for &column in columns {
            let value = if query % QUERY_PERIOD == 1 {
                specification.rows + 1
            } else {
                values[column]
            };
            keys.push((column, budget.value(specification.payload, value)?));
        }
        queries.push(keys);
    }
    Ok(queries)
}

struct Budget {
    maximum: usize,
    live: usize,
    peak: usize,
}

impl Budget {
    fn new(limits: Limits, initial: usize) -> Result<Self, Error> {
        let mut budget = Self {
            maximum: limits.max_bytes,
            live: 0,
            peak: 0,
        };
        budget.add(initial)?;
        Ok(budget)
    }

    fn check(&mut self, additional: usize) -> Result<usize, Error> {
        let requested = self.live.checked_add(additional).ok_or(Error::Overflow)?;
        if requested > self.maximum {
            return Err(Error::Bytes);
        }
        self.peak = self.peak.max(requested);
        Ok(requested)
    }

    fn add(&mut self, additional: usize) -> Result<(), Error> {
        self.live = self.check(additional)?;
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Error> {
        self.check(count.checked_mul(size_of::<T>()).ok_or(Error::Overflow)?)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        self.add(
            values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(Error::Overflow)?,
        )?;
        Ok(values)
    }

    fn value(&mut self, payload: Payload, value: usize) -> Result<Value, Error> {
        let number = i32::try_from(value).map_err(|_| Error::Overflow)?;
        if payload == Payload::Numeric {
            return Ok(Value::Number(number));
        }
        self.check(VALUE_CONSTRUCTION_BYTES)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(2).map_err(|_| Error::Allocation)?;
        nodes.push(ValueNode::Tuple { arity: 1 });
        nodes.push(ValueNode::Number(number));
        let value = Value::from_nodes(
            nodes,
            ValueLimits {
                max_nodes: 2,
                max_depth: 2,
                max_bytes: VALUE_CONSTRUCTION_BYTES,
            },
        )
        .map_err(Error::Value)?;
        self.add(value.payload_bytes())?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
