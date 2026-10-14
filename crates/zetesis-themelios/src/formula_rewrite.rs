//! Shared original-owner replacement and lazy fixed-fact evidence.

use std::collections::BTreeMap;

#[cfg(test)]
use themelios_program::program::Program;
use themelios_program::program::Rule;
use themelios_program::provenance::Provenance;
use zetesis_domain::{FactIndex, KeyWork, Stop};

use crate::StatementId;

pub(crate) struct Replacement {
    pub(crate) provenance: Provenance,
    pub(crate) rules: Vec<Rule>,
    pub(crate) tag: &'static str,
}

pub(crate) type Replacements = BTreeMap<StatementId, Replacement>;

/// Optional key work stops remain distinct from a refusal to construct the
/// complete fact input under the mandatory preparation expansion budget.
#[derive(Debug)]
pub(crate) enum FactFailure {
    Stopped(Stop),
    Source(crate::FormulaFailure),
}

enum Input<'p> {
    Summary(&'p crate::formula_analysis::summary::PreparationFacts<'p>),
    #[cfg(test)]
    Program(&'p Program),
}

/// The index borrows atoms owned by the outer preparation reader. It never owns
/// the storage it indexes, and a stopped reading publishes no partial index.
pub(crate) struct FixedFacts<'p> {
    input: Input<'p>,
    index: Option<FactIndex<'p>>,
}

impl<'p> FixedFacts<'p> {
    pub(crate) const fn summary(
        reader: &'p crate::formula_analysis::summary::PreparationFacts<'p>,
    ) -> Self {
        Self {
            input: Input::Summary(reader),
            index: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn new(program: &'p Program) -> Self {
        Self {
            input: Input::Program(program),
            index: None,
        }
    }

    pub(crate) fn read(
        &mut self,
        work: &mut KeyWork,
        budget: &mut crate::expansion::Budget,
        site: crate::ProgramSite,
    ) -> Result<&FactIndex<'p>, FactFailure> {
        if self.index.is_none() {
            let index = match self.input {
                Input::Summary(reader) => {
                    let atoms = reader.atoms(budget, site).map_err(FactFailure::Source)?;
                    FactIndex::read_parts(reader.summary.producers(), atoms, work)
                }
                #[cfg(test)]
                Input::Program(program) => FactIndex::read(program, work),
            }
            .map_err(FactFailure::Stopped)?;
            self.index = Some(index);
        }
        Ok(self.index.as_ref().expect("complete fixed-fact reading"))
    }
}
