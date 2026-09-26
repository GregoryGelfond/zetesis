//! Sealed lanes and checked borrowed resolution. No segment refers to an older
//! segment: cross-segment edges are plain IDs, so teardown never follows depth.

use std::{mem::size_of, ops::Range};

use crate::{Sign, ValueNodeRef};

use super::nodes::{Compound, Measures, Nodes};
use super::{AtomId, PredicateId, Read, Store, TermId, TextId, budget};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Counts {
    pub(super) texts: usize,
    pub(super) terms: usize,
    pub(super) predicates: usize,
    pub(super) atoms: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Text {
    pub(super) bytes: Range<usize>,
    pub(super) quoted_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Signature {
    pub(super) name: TextId,
    pub(super) sign: Sign,
    pub(super) arity: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Locator {
    pub(super) predicate: PredicateId,
    pub(super) block: usize,
    pub(super) row: usize,
}

#[derive(Debug)]
pub(super) struct Columns {
    pub(super) predicate: PredicateId,
    pub(super) arguments: Vec<Vec<TermId>>,
    pub(super) rows: usize,
}

#[derive(Debug)]
pub(super) struct VocabularySegment {
    pub(super) start: Counts,
    pub(super) text: String,
    pub(super) texts: Vec<Text>,
    pub(super) terms: Nodes<TermId, TextId>,
    pub(super) predicates: Vec<Signature>,
}

impl VocabularySegment {
    pub(super) fn new(start: Counts) -> Self {
        Self {
            start,
            text: String::new(),
            texts: Vec::new(),
            terms: Nodes::new(),
            predicates: Vec::new(),
        }
    }

    pub(super) fn counts(&self) -> Counts {
        Counts {
            texts: self.start.texts + self.texts.len(),
            terms: self.start.terms + self.terms.kinds.len(),
            predicates: self.start.predicates + self.predicates.len(),
            atoms: 0,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.terms.kinds.is_empty() && self.predicates.is_empty()
    }

    pub(super) fn bytes(&self) -> u128 {
        size_of::<Self>() as u128
            + self.text.capacity() as u128
            + budget::capacity(&self.texts)
            + self.terms.buffer_bytes()
            + budget::capacity(&self.predicates)
    }
}

#[derive(Debug)]
pub(super) struct RowSegment {
    pub(super) start: usize,
    pub(super) atoms: Vec<Locator>,
    pub(super) columns: Vec<Columns>,
}

impl RowSegment {
    pub(super) fn new(start: usize) -> Self {
        Self {
            start,
            atoms: Vec::new(),
            columns: Vec::new(),
        }
    }

    pub(super) fn count(&self) -> usize {
        self.start + self.atoms.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
    pub(super) fn bytes_with<E>(
        &self,
        mut before: impl FnMut() -> Result<(), E>,
    ) -> Result<u128, E> {
        before()?;
        let mut bytes = size_of::<Self>() as u128
            + budget::capacity(&self.atoms)
            + budget::capacity(&self.columns);
        for columns in &self.columns {
            before()?;
            bytes += budget::capacity(&columns.arguments);
            for column in &columns.arguments {
                before()?;
                bytes += budget::capacity(column);
            }
        }
        Ok(bytes)
    }
}

impl Store {
    fn text_measures(&self, id: TextId) -> (usize, usize) {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.texts)
            .expect("admitted text ID");
        text_measures(segment, id)
    }

    pub(super) fn text(&self, id: TextId) -> &str {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.texts)
            .expect("admitted text ID");
        text(segment, id)
    }

    pub(super) fn term_segment(&self, id: TermId) -> (&VocabularySegment, usize) {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.terms)
            .expect("admitted term ID");
        (segment, id.0 as usize - segment.start.terms)
    }

    pub(super) fn term_measures(&self, id: TermId) -> Measures {
        let (segment, local) = self.term_segment(id);
        segment
            .terms
            .measures(local, |id| self.text_measures(TextId(id)))
    }

    pub(super) fn signature(&self, id: PredicateId) -> Signature {
        let segment = self
            .vocabulary_segment(id.0 as usize, |counts| counts.predicates)
            .expect("admitted predicate ID");
        segment.predicates[id.0 as usize - segment.start.predicates]
    }

    pub(super) fn atom_columns(&self, id: AtomId) -> (&Columns, Locator) {
        let segment = self.row_segment(id.0 as usize).expect("admitted atom ID");
        let locator = segment.atoms[id.0 as usize - segment.start];
        (&segment.columns[locator.block], locator)
    }
}

pub(super) fn text(segment: &VocabularySegment, id: TextId) -> &str {
    let range = segment.texts[id.0 as usize - segment.start.texts]
        .bytes
        .clone();
    // Append accepts str, and each recorded endpoint is an append boundary.
    // String slicing checks endpoints without rescanning the complete payload.
    segment.text.get(range).expect("admitted text boundaries")
}

pub(super) fn text_measures(segment: &VocabularySegment, id: TextId) -> (usize, usize) {
    let text = &segment.texts[id.0 as usize - segment.start.texts];
    (text.bytes.len(), text.quoted_bytes)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Term<'a> {
    pub(super) read: Read<'a>,
    pub(super) segment: &'a VocabularySegment,
    pub(super) local: usize,
}

impl<'a> Term<'a> {
    pub(crate) fn descriptor(self) -> ValueNodeRef<'a> {
        self.segment.terms.descriptor(
            self.local,
            |id| self.read.text(TextId(id)),
            |id| self.read.text(id),
        )
    }

    fn compound(self) -> Option<&'a Compound<TextId>> {
        self.segment.terms.compound(self.local)
    }

    pub(crate) fn child(self, index: usize) -> Option<TermId> {
        self.segment.terms.child(self.local, index)
    }

    pub(crate) fn child_end(self, index: usize) -> Option<usize> {
        self.segment.terms.child_end(self.local, index)
    }

    fn measures(self) -> Measures {
        self.segment
            .terms
            .measures(self.local, |id| self.read.text_measures(TextId(id)))
    }
    pub(crate) fn expanded_nodes(self) -> usize {
        self.compound()
            .map_or(1, |compound| compound.measures.nodes)
    }
    pub(crate) fn depth(self) -> usize {
        self.compound()
            .map_or(1, |compound| compound.measures.depth)
    }
    pub(crate) fn canonical_bytes(self) -> usize {
        self.measures().canonical
    }
    pub(crate) fn rendered_bytes(self) -> usize {
        self.measures().rendered
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Predicate<'a> {
    pub(super) name: &'a str,
    pub(super) arity: usize,
    pub(super) sign: Sign,
}

impl<'a> Predicate<'a> {
    pub(crate) fn name(self) -> &'a str {
        self.name
    }
    pub(crate) fn sign(self) -> Sign {
        self.sign
    }
    pub(crate) fn arity(self) -> usize {
        self.arity
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Atom<'a> {
    pub(super) columns: &'a Columns,
    pub(super) locator: Locator,
}

impl Atom<'_> {
    pub(crate) fn predicate(self) -> PredicateId {
        self.locator.predicate
    }
    // Publication checks each row against its signature's arity, and a block
    // holds one column per argument, so no signature read is needed.
    pub(crate) fn arity(self) -> usize {
        self.columns.arguments.len()
    }
    pub(crate) fn argument(self, index: usize) -> Option<TermId> {
        self.columns
            .arguments
            .get(index)
            .map(|column| column[self.locator.row])
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn grouped_lanes_preserve_the_vocabulary_header() {
        // Before grouping, the segment held Counts, String and seven Vec
        // headers directly. No extra owner, enum or box belongs in that layout.
        let prior = size_of::<Counts>() + size_of::<String>() + 7 * size_of::<Vec<u8>>();
        assert_eq!(size_of::<VocabularySegment>(), prior);
        assert_eq!(
            VocabularySegment::new(Counts::default()).bytes(),
            prior as u128
        );
    }
}
