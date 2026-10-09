//! Shared original-owner replacement and lazy fixed-fact evidence.

use std::collections::BTreeMap;

use themelios_program::program::{Program, Rule};
use themelios_program::provenance::Provenance;
use zetesis_domain::{FactIndex, KeyWork, Stop};

use crate::StatementId;

pub(crate) struct Replacement {
    pub(crate) provenance: Provenance,
    pub(crate) rules: Vec<Rule>,
    pub(crate) tag: &'static str,
}

pub(crate) type Replacements = BTreeMap<StatementId, Replacement>;

/// Paired with one immutable normalized program. The first consumer pays the
/// one complete reading; later consumers borrow precisely that same evidence.
pub(crate) struct FixedFacts<'p> {
    program: &'p Program,
    index: Option<FactIndex<'p>>,
}

impl<'p> FixedFacts<'p> {
    pub(crate) const fn new(program: &'p Program) -> Self {
        Self {
            program,
            index: None,
        }
    }

    pub(crate) fn read(&mut self, work: &mut KeyWork) -> Result<&FactIndex<'p>, Stop> {
        if self.index.is_none() {
            self.index = Some(FactIndex::read(self.program, work)?);
        }
        Ok(self.index.as_ref().expect("complete fixed-fact reading"))
    }
}
