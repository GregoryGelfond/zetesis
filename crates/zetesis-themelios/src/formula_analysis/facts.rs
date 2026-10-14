//! Exact analysis facts over prevalidated source leaves, not exported core values.
//!
//! A recipe is admitted only after `fact_expansion` checked the complete family.
//! Its integer segments and leaf positions describe that immutable normalized
//! carrier. Neither preparation readers nor later public inspection replay
//! normalization, arithmetic, admission or the structured-value bridge.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Range;

use themelios_program::program::{
    Arguments, Atom, Body, DefaultNegation, Head, LiteralInner, Program, Rule, Statement,
};
use themelios_program::provenance::{Provenance, WithProvenance};
use themelios_program::symbol::{Name, Sign, Symbol};
use themelios_program::term::Term;

use crate::expansion::Budget;
use crate::{ExpansionResource, FormulaFailure, ProgramSite, StatementId};

#[derive(Default)]
pub(crate) struct Families {
    sources: Vec<Family>,
    products: Vec<Product>,
    columns: Vec<Column>,
    segments: Vec<Segment>,
}

struct Family {
    owner: StatementId,
    normalized: Option<Program>,
    provenance: Provenance,
}

struct Product {
    family: usize,
    columns: Range<usize>,
    rows: usize,
}

struct Column {
    segments: Range<usize>,
    width: usize,
    stride: usize,
}

struct Segment {
    end: usize,
    value: SegmentValue,
}

enum SegmentValue {
    Symbol(usize),
    Integers(i32),
}

/// A source product and its mixed-radix row, before canonical merging.
#[derive(Clone, Copy)]
pub(super) struct Position {
    product: usize,
    row: usize,
}

/// A canonical fact keeps its first source occurrence and the other families
/// whose root evidence contributes. Repeated rows of one family add no evidence.
pub(super) struct Fact {
    position: Position,
    others: Vec<usize>,
}

/// Borrows source atoms and scalar leaves. No owned value or row is reconstructed.
pub(super) struct Reader<'a> {
    families: &'a Families,
    inputs: Vec<Input<'a>>,
}

struct Input<'a> {
    atom: &'a Atom,
    symbols: Vec<&'a Symbol>,
}

/// Preparation reserves with typed limits; already-admitted public inspection
/// uses upstream's ordinary infallible allocation boundary. The same traversal
/// and private locator invariant serve both readers.
pub(super) trait Allocation {
    type Failure;
    fn poll(&mut self) -> Result<(), Self::Failure>;
    fn reserve_prepaid<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> Result<(), Self::Failure>;
    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Failure>;
}

pub(crate) struct Checked<'a> {
    pub(crate) budget: &'a mut Budget,
    pub(crate) site: ProgramSite,
}

impl Allocation for Checked<'_> {
    type Failure = FormulaFailure;
    fn reserve_prepaid<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> Result<(), Self::Failure> {
        self.budget.poll(self.site)?;
        values
            .try_reserve_exact(count)
            .map_err(|error| crate::ExpansionFailure::Allocation {
                error,
                location: self.site,
            })?;
        Ok(())
    }
    fn poll(&mut self) -> Result<(), Self::Failure> {
        Ok(self.budget.poll(self.site)?)
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Failure> {
        self.budget.poll(self.site)?;
        crate::formula_pattern::reserve_amortized(values, count, self.budget, self.site)
    }
}

pub(super) struct Inspection;
impl Allocation for Inspection {
    type Failure = std::convert::Infallible;
    fn reserve_prepaid<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
    ) -> Result<(), Self::Failure> {
        values.reserve_exact(count);
        Ok(())
    }
    fn poll(&mut self) -> Result<(), Self::Failure> {
        Ok(())
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, count: usize) -> Result<(), Self::Failure> {
        values.reserve(count);
        Ok(())
    }
}

impl Families {
    /// Record a family whose whole shape and expansion have already passed
    /// `fact_expansion`. Empty products are omitted, but every scalar leaf keeps
    /// its source ordinal so later reads do not depend on omitted products.
    pub(crate) fn record(
        &mut self,
        atom: &Atom,
        provenance: &Provenance,
        owner: StatementId,
        expected: usize,
        allocation: &mut Checked<'_>,
    ) -> Result<usize, FormulaFailure> {
        let family = self.sources.len();
        let mut scalar = 0;
        let mut rows = 0;
        for arguments in atom.alternatives() {
            let first_column = self.columns.len();
            let first_segment = self.segments.len();
            for term in arguments {
                self.record_column(term, &mut scalar, allocation)?;
            }
            if self.columns[first_column..]
                .iter()
                .any(|column| column.width == 0)
            {
                self.columns.truncate(first_column);
                self.segments.truncate(first_segment);
                continue;
            }
            let mut count = 1_usize;
            for column in self.columns[first_column..].iter_mut().rev() {
                column.stride = count;
                count = count
                    .checked_mul(column.width)
                    .expect("the admitted product fits usize");
            }
            if count == 0 {
                self.columns.truncate(first_column);
                self.segments.truncate(first_segment);
                continue;
            }
            rows += count;
            allocation.reserve(&mut self.products, 1)?;
            self.products.push(Product {
                family,
                columns: first_column..self.columns.len(),
                rows: count,
            });
        }
        assert_eq!(
            rows, expected,
            "source recipe and accepted expansion have the same rows"
        );
        allocation.reserve(&mut self.sources, 1)?;
        self.sources.push(Family {
            owner,
            normalized: None,
            provenance: provenance.clone(),
        });
        Ok(family)
    }

    /// One column preserves source leaf order; empty intervals add no segment.
    fn record_column(
        &mut self,
        term: &Term,
        scalar: &mut usize,
        allocation: &mut Checked<'_>,
    ) -> Result<(), FormulaFailure> {
        let start = self.segments.len();
        let mut width = 0;
        let mut pending = Vec::new();
        allocation.reserve(&mut pending, 1)?;
        pending.push(term);
        while let Some(term) = pending.pop() {
            allocation.budget.poll(allocation.site)?;
            let (value, count) = match term {
                Term::Pool(items) => {
                    allocation.reserve(&mut pending, items.len())?;
                    pending.extend(items.iter().rev());
                    continue;
                }
                Term::Symbolic(_) => {
                    let index = *scalar;
                    *scalar += 1;
                    (SegmentValue::Symbol(index), 1)
                }
                Term::Interval { lower, upper } => {
                    let (Term::Symbolic(lower), Term::Symbolic(upper)) = (&**lower, &**upper)
                    else {
                        unreachable!("fact expansion accepted symbolic interval endpoints")
                    };
                    let (Symbol::Number(lower), Symbol::Number(upper)) = (lower, upper) else {
                        continue;
                    };
                    if lower > upper {
                        continue;
                    }
                    let count = usize::try_from(i64::from(*upper) - i64::from(*lower) + 1)
                        .expect("the admitted family fits usize");
                    (SegmentValue::Integers(*lower), count)
                }
                _ => unreachable!("fact expansion accepted only symbolic leaves and intervals"),
            };
            width += count;
            allocation.reserve(&mut self.segments, 1)?;
            self.segments.push(Segment { end: width, value });
        }
        allocation.reserve(&mut self.columns, 1)?;
        self.columns.push(Column {
            segments: start..self.segments.len(),
            width,
            stride: 1,
        });
        Ok(())
    }

    pub(crate) fn normalized(&mut self, family: usize, source: Program) {
        self.sources[family].normalized = Some(source);
    }

    pub(super) fn rows(&self) -> usize {
        self.products.iter().map(|product| product.rows).sum()
    }

    /// One first row per source product, in original family/alternative order.
    pub(super) fn first_rows(&self) -> impl Iterator<Item = Position> + '_ {
        self.products
            .iter()
            .enumerate()
            .map(|(product, _)| Position { product, row: 0 })
    }

    pub(super) fn read<'a, A: Allocation>(
        &'a self,
        source: &'a Program,
        allocation: &mut A,
    ) -> Result<Reader<'a>, A::Failure> {
        allocation.poll()?;
        if self.sources.is_empty() {
            return Ok(Reader {
                families: self,
                inputs: Vec::new(),
            });
        }
        let mut originals = Vec::new();
        for statement in source.statements() {
            allocation.reserve(&mut originals, 1)?;
            originals.push(statement);
        }
        let mut inputs = Vec::new();
        allocation.reserve(&mut inputs, self.sources.len())?;
        for family in &self.sources {
            allocation.poll()?;
            let statement = family.normalized.as_ref().map_or_else(
                || originals[family.owner.index()],
                |program| {
                    program
                        .statements()
                        .next()
                        .expect("one normalized family carrier")
                },
            );
            let atom = crate::fact_expansion::fact_atom(statement.get())
                .expect("validated fact recipe carrier");
            let mut symbols = Vec::new();
            let mut pending = Vec::new();
            for arguments in atom.alternatives() {
                for term in arguments {
                    allocation.reserve(&mut pending, 1)?;
                    pending.push(term);
                    while let Some(term) = pending.pop() {
                        match term {
                            Term::Pool(items) => {
                                allocation.reserve(&mut pending, items.len())?;
                                pending.extend(items.iter().rev());
                            }
                            Term::Symbolic(symbol) => {
                                allocation.reserve(&mut symbols, 1)?;
                                symbols.push(symbol);
                            }
                            Term::Interval { .. } => {}
                            _ => unreachable!("validated fact source recipe"),
                        }
                    }
                }
            }
            inputs.push(Input { atom, symbols });
        }
        Ok(Reader {
            families: self,
            inputs,
        })
    }
}

impl Reader<'_> {
    pub(super) fn canonical(
        &self,
        allocation: &mut Checked<'_>,
    ) -> Result<Vec<Fact>, FormulaFailure> {
        let mut map = BTreeMap::<Key<'_, '_>, Fact>::new();
        for (product, plan) in self.families.products.iter().enumerate() {
            for row in 0..plan.rows {
                allocation
                    .budget
                    .charge(ExpansionResource::TermWork, 1, allocation.site)?;
                // As for Owners, this names BTreeMap payload, not allocator bookkeeping.
                allocation.budget.charge(
                    ExpansionResource::ScalarBytes,
                    size_of::<(Key<'_, '_>, Fact)>() as u128,
                    allocation.site,
                )?;
                let position = Position { product, row };
                match map.entry(Key {
                    reader: self,
                    position,
                }) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(Fact {
                            position,
                            others: Vec::new(),
                        });
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        let fact = entry.get_mut();
                        let first = self.families.products[fact.position.product].family;
                        if plan.family != first && fact.others.last() != Some(&plan.family) {
                            allocation.reserve(&mut fact.others, 1)?;
                            fact.others.push(plan.family);
                        }
                    }
                }
            }
        }
        let mut facts = Vec::new();
        allocation.reserve(&mut facts, map.len())?;
        for fact in map.into_values() {
            allocation.poll()?;
            facts.push(fact);
        }
        Ok(facts)
    }

    fn product(&self, position: Position) -> &Product {
        &self.families.products[position.product]
    }

    fn input(&self, position: Position) -> &Input<'_> {
        &self.inputs[self.product(position).family]
    }

    pub(super) fn signature(&self, position: Position) -> (Sign, &Name, usize) {
        let atom = self.input(position).atom;
        (atom.sign, &atom.name, self.product(position).columns.len())
    }

    fn symbols(&self, position: Position) -> impl Iterator<Item = Scalar<'_>> {
        let product = self.product(position);
        self.families.columns[product.columns.clone()]
            .iter()
            .map(move |column| {
                let offset = (position.row / column.stride) % column.width;
                let segments = &self.families.segments[column.segments.clone()];
                let index = segments.partition_point(|segment| segment.end <= offset);
                let segment = &segments[index];
                match segment.value {
                    SegmentValue::Symbol(index) => {
                        Scalar::Symbol(self.input(position).symbols[index])
                    }
                    SegmentValue::Integers(first) => {
                        let start = if index == 0 {
                            0
                        } else {
                            segments[index - 1].end
                        };
                        let number = i64::from(first)
                            + i64::try_from(offset - start).expect("admitted i32 interval width");
                        Scalar::Number(
                            i32::try_from(number).expect("position in admitted i32 interval"),
                        )
                    }
                }
            })
    }

    fn compare(&self, left: Position, right: Position) -> Ordering {
        let left_atom = self.input(left).atom;
        let right_atom = self.input(right).atom;
        left_atom
            .sign
            .cmp(&right_atom.sign)
            .then_with(|| left_atom.name.cmp(&right_atom.name))
            .then_with(|| self.symbols(left).cmp(self.symbols(right)))
    }

    pub(super) fn atom(&self, fact: &Fact) -> Atom {
        self.atom_at(fact.position)
    }

    pub(super) fn atom_at(&self, position: Position) -> Atom {
        let source = self.input(position).atom;
        Atom {
            sign: source.sign,
            name: source.name.clone(),
            arguments: Arguments::Single(
                self.symbols(position)
                    .map(|value| Term::Symbolic(value.owned()))
                    .collect(),
            ),
        }
    }

    pub(super) fn representative(&self, position: Position) -> WithProvenance<Statement> {
        let provenance = &self.families.sources[self.product(position).family].provenance;
        statement(self.atom_at(position), provenance.clone())
    }

    pub(super) fn statement(&self, fact: &Fact) -> WithProvenance<Statement> {
        statement(self.atom(fact), self.provenance(fact).into_owned())
    }

    pub(super) fn provenance(&self, fact: &Fact) -> Cow<'_, Provenance> {
        let first = &self.families.sources[self.product(fact.position).family].provenance;
        if fact.others.is_empty() {
            Cow::Borrowed(first)
        } else {
            Cow::Owned(
                fact.others
                    .iter()
                    .fold(first.clone(), |provenance, &family| {
                        provenance.merge(self.families.sources[family].provenance.clone())
                    }),
            )
        }
    }

    /// The last source family determines the fact carrier's content on ingest;
    /// all fact children are constructed, while root evidence is unioned.
    pub(super) fn last_owner(&self, fact: &Fact) -> StatementId {
        let family = fact
            .others
            .last()
            .copied()
            .unwrap_or(self.product(fact.position).family);
        self.families.sources[family].owner
    }

    pub(super) fn arity(&self, fact: &Fact) -> usize {
        self.product(fact.position).columns.len()
    }

    pub(super) fn name_bytes(&self, fact: &Fact) -> usize {
        self.input(fact.position).atom.name.as_str().len()
    }

    /// A fact is `Statement::Rule` with a positive atom literal and empty body.
    /// These comparisons pin the reviewed themelios derived field/variant order;
    /// no source value is copied merely to compare a virtual fact with a rule.
    pub(super) fn compare_statement(&self, fact: &Fact, statement: &Statement) -> Ordering {
        let Statement::Rule(rule) = statement else {
            return Ordering::Less;
        };
        let Head::Literal(literal) = rule.head().get() else {
            return Ordering::Less;
        };
        let negation = DefaultNegation::None.cmp(&literal.negation);
        if negation != Ordering::Equal {
            return negation;
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Ordering::Less;
        };
        let source = self.input(fact.position).atom;
        let order = source
            .sign
            .cmp(&atom.get().sign)
            .then_with(|| source.name.cmp(&atom.get().name));
        if order != Ordering::Equal {
            return order;
        }
        let Arguments::Single(terms) = &atom.get().arguments else {
            return Ordering::Less;
        };
        let mut left = self.symbols(fact.position);
        let mut right = terms.iter();
        loop {
            let order = match (left.next(), right.next()) {
                (Some(left), Some(right)) => match right {
                    Term::Symbolic(right) => left.cmp(&Scalar::Symbol(right)),
                    Term::Variable(_) => Ordering::Greater,
                    _ => Ordering::Less,
                },
                (None, None) => break,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        if rule.body().get().elements().next().is_none() {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    }
}

fn statement(atom: Atom, provenance: Provenance) -> WithProvenance<Statement> {
    WithProvenance::new(Statement::Rule(Rule::new(atom, Body::new([]))), provenance)
}

struct Key<'r, 's> {
    reader: &'r Reader<'s>,
    position: Position,
}
impl PartialEq for Key<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Key<'_, '_> {}
impl PartialOrd for Key<'_, '_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key<'_, '_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.reader.compare(self.position, other.position)
    }
}

enum Scalar<'a> {
    Symbol(&'a Symbol),
    Number(i32),
}
impl Scalar<'_> {
    fn owned(self) -> Symbol {
        match self {
            Self::Symbol(symbol) => symbol.clone(),
            Self::Number(number) => Symbol::Number(number),
        }
    }
}
impl PartialEq for Scalar<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Scalar<'_> {}
impl PartialOrd for Scalar<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Scalar<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Symbol(left), Self::Symbol(right)) => left.cmp(right),
            (Self::Symbol(left), Self::Number(right)) => (*left).cmp(&Symbol::Number(*right)),
            (Self::Number(left), Self::Symbol(right)) => Symbol::Number(*left).cmp(right),
            (Self::Number(left), Self::Number(right)) => left.cmp(right),
        }
    }
}
