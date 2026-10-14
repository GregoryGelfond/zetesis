//! One closed fact authority, relation columns and original row occurrences.

mod input;
pub use input::{FactSegment, FactTables};
mod build;
pub(super) use build::{Prepared, append, close};

use std::ops::Range;

use super::{ProgramData, Templates};
use crate::atom_interner::ClosedCatalog;
use crate::catalog::{AtomRef, Atoms, CatalogRead};
use crate::relation::{Catalog, Relation};
use crate::{PatternRef, TemplateRef};

pub(super) struct Data {
    pub(super) store: ClosedCatalog,
    pub(super) relations: Vec<Catalog>,
    pub(super) relation_order: Vec<usize>,
    pub(super) occurrences: Vec<Occurrence>,
    pub(super) blocks: Vec<Block>,
}

#[derive(Clone, Copy)]
pub(super) struct Occurrence {
    pub(super) relation: usize,
    pub(super) row: usize,
}

pub(super) struct Block {
    pub(super) before: usize,
    pub(super) occurrences: Range<usize>,
}

impl std::fmt::Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Facts")
            .field("predicates", &self.relations.len())
            .field("occurrences", &self.occurrences.len())
            .field("blocks", &self.blocks.len())
            .finish_non_exhaustive()
    }
}

impl Data {
    /// Extra retained payload beyond the containing `ProgramData` header and its
    /// already-counted vocabulary. Relation headers reside in their vector.
    pub(super) fn bytes(&self, vocabulary: u128) -> u128 {
        self.store.storage_bytes() - size_of::<ClosedCatalog>() as u128 - vocabulary
            + self.relations.capacity() as u128 * size_of::<Catalog>() as u128
            + self
                .relations
                .iter()
                .map(|relation| relation.retained_bytes() as u128 - size_of::<Catalog>() as u128)
                .sum::<u128>()
            + self.relation_order.capacity() as u128 * size_of::<usize>() as u128
            + self.occurrences.capacity() as u128 * size_of::<Occurrence>() as u128
            + self.blocks.capacity() as u128 * size_of::<Block>() as u128
    }

    fn atom(&self, occurrence: Occurrence) -> AtomRef<'_> {
        self.relations[occurrence.relation]
            .atoms(self.store.read())
            .expect("fact relation belongs to its closed authority")
            .at(occurrence.row)
            .expect("fact occurrence names an admitted relation row")
    }
}

/// One original fact occurrence, preserving source order and duplicates.
/// Its atom borrows the shared fact store. No value or tuple is reconstructed.
#[derive(Clone, Copy)]
pub struct FactRef<'a> {
    data: &'a Data,
    index: usize,
}

impl<'a> FactRef<'a> {
    /// Position in the original fact-occurrence sequence, for parallel evidence.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Canonical atom of this occurrence. Equal occurrences share its payload.
    #[must_use]
    pub fn atom(self) -> AtomRef<'a> {
        self.data.atom(self.data.occurrences[self.index])
    }

    /// A constant head pattern borrowing the same canonical atom.
    #[must_use]
    pub fn pattern(self) -> PatternRef<'a> {
        PatternRef::fact(self.atom())
    }

    /// A body-free rule view for readers that already consume normalized rules.
    /// This allocates no template, term array or owned value.
    #[must_use]
    pub fn as_template(self) -> TemplateRef<'a> {
        TemplateRef::fact(self.atom())
    }
}

impl std::fmt::Debug for FactRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fact")
            .field("index", &self.index)
            .field("atom", &self.atom())
            .finish()
    }
}

/// Borrowed random access and source-order traversal of original fact occurrences.
#[derive(Clone, Copy)]
pub struct FactOccurrences<'a> {
    pub(super) data: Option<&'a Data>,
}
impl<'a> FactOccurrences<'a> {
    #[must_use]
    /// Number of original occurrences, including duplicates.
    pub fn len(self) -> usize {
        self.data.map_or(0, |data| data.occurrences.len())
    }
    #[must_use]
    /// Whether this view contains no occurrence.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[must_use]
    /// Borrow an occurrence by its original table-fact index.
    pub fn at(self, index: usize) -> Option<FactRef<'a>> {
        let data = self.data?;
        (index < data.occurrences.len()).then_some(FactRef { data, index })
    }
    /// Traverse every original occurrence in order.
    #[must_use]
    pub fn iter(self) -> impl ExactSizeIterator<Item = FactRef<'a>> {
        self.into_iter()
    }
}
impl<'a> IntoIterator for FactOccurrences<'a> {
    type Item = FactRef<'a>;
    type IntoIter = FactIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        FactIter {
            facts: self,
            next: 0,
        }
    }
}
/// Iterator over original table-fact occurrences.
pub struct FactIter<'a> {
    facts: FactOccurrences<'a>,
    next: usize,
}
impl<'a> Iterator for FactIter<'a> {
    type Item = FactRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let fact = self.facts.at(self.next)?;
        self.next += 1;
        Some(fact)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.facts.len() - self.next;
        (count, Some(count))
    }
}
impl ExactSizeIterator for FactIter<'_> {}
impl std::iter::FusedIterator for FactIter<'_> {}

/// One predicate's distinct fact rows, columns and canonical order, all borrowed
/// from the program's single closed authority and relation catalog.
#[derive(Clone, Copy)]
pub struct FactRelation<'a> {
    relation: &'a Catalog,
    read: CatalogRead<'a>,
}
impl<'a> FactRelation<'a> {
    /// Columnar values and row IDs; equality IDs have no numeric ordering meaning.
    ///
    /// # Panics
    /// Panics if admission's internal shared-authority invariant is violated.
    /// Callers cannot construct an invalid relation view.
    #[must_use]
    pub fn relation(self) -> Relation<'a> {
        self.relation.view(self.read).expect("closed fact relation")
    }
    /// Distinct atoms in catalog insertion order.
    ///
    /// # Panics
    /// Panics if admission's internal shared-authority invariant is violated.
    /// Callers cannot construct an invalid relation view.
    #[must_use]
    pub fn atoms(self) -> Atoms<'a> {
        self.relation
            .atoms(self.read)
            .expect("closed fact relation")
    }
    /// Distinct atoms in canonical semantic storage order. Admission prepares the
    /// catalog once after its final append, so its sole sorted run covers all rows.
    ///
    /// # Panics
    /// Panics if admission's internal shared-authority or ordered-row invariant is
    /// violated. Callers cannot construct an invalid relation view.
    #[must_use]
    pub fn canonical_atoms(self) -> impl ExactSizeIterator<Item = AtomRef<'a>> {
        let atoms = self.atoms();
        self.relation
            .ordered()
            .expect("sealed ordered facts")
            .tail()
            .iter()
            .map(move |&row| atoms.at(row).expect("ordered fact row"))
    }
}

pub(super) fn relations(data: Option<&Data>) -> impl Iterator<Item = FactRelation<'_>> {
    data.into_iter().flat_map(|data| {
        data.relation_order.iter().map(|&index| FactRelation {
            relation: &data.relations[index],
            read: data.store.read(),
        })
    })
}

/// An original program occurrence. Template indices and fact occurrence indices
/// belong to separate sequences and are never inferred from one another.
#[derive(Clone, Copy, Debug)]
pub enum RuleRef<'a> {
    /// One caller-supplied template.
    Template {
        /// Position in the template-only sequence.
        index: usize,
        /// Borrowed canonical template.
        template: TemplateRef<'a>,
    },
    /// One explicit table-fact occurrence.
    Fact(FactRef<'a>),
}

/// Whole-program traversal, merging anchored fact blocks with template order.
pub struct Rules<'a> {
    pub(super) program: &'a ProgramData,
    template: usize,
    block: usize,
    fact: usize,
    remaining: usize,
}
impl<'a> Rules<'a> {
    pub(super) fn new(program: &'a ProgramData) -> Self {
        Self {
            program,
            template: 0,
            block: 0,
            fact: 0,
            remaining: program.templates.len()
                + program
                    .facts
                    .as_ref()
                    .map_or(0, |data| data.occurrences.len()),
        }
    }
}
impl<'a> Iterator for Rules<'a> {
    type Item = RuleRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        if let Some(data) = &self.program.facts
            && let Some(block) = data.blocks.get(self.block)
            && block.before == self.template
        {
            let index = block.occurrences.start + self.fact;
            self.fact += 1;
            if index + 1 == block.occurrences.end {
                self.block += 1;
                self.fact = 0;
            }
            self.remaining -= 1;
            return Some(RuleRef::Fact(FactRef { data, index }));
        }
        let index = self.template;
        let template = Templates {
            program: self.program,
        }
        .at(index)
        .expect("valid fact anchors preserve template traversal");
        self.template += 1;
        self.remaining -= 1;
        Some(RuleRef::Template { index, template })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for Rules<'_> {}
impl std::iter::FusedIterator for Rules<'_> {}
