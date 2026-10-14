//! Typed caller-owned rules ready for the corresponding storage admission door.

use super::{
    AdmissionError, ExtremumCutRule, ExtremumRow, FormulaTransaction, NumericDomain, Row, Rule,
};

pub(in crate::formula_ground) enum Pending<'a> {
    Weights(&'a mut [i128]),
    Ordinal(&'a mut [ExtremumCutRule]),
}

impl Pending<'_> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Weights(rules) => rules.len(),
            Self::Ordinal(rules) => rules.len(),
        }
    }

    pub(super) fn set(&mut self, index: usize, rule: Rule) -> Result<(), AdmissionError> {
        match (self, rule) {
            (Self::Weights(rules), Rule::Threshold(bound)) => rules[index] = bound,
            (Self::Ordinal(rules), Rule::Extremum(rule)) => rules[index] = rule,
            _ => return Err(AdmissionError::NumericDomain),
        }
        Ok(())
    }

    pub(super) fn append(
        self,
        transaction: &mut FormulaTransaction<'_>,
        row: Row<'_>,
        length: usize,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<std::ops::Range<usize>, AdmissionError> {
        match (self, row.domain) {
            (Self::Weights(bounds), NumericDomain::Weights) => transaction
                .push_threshold_family_range(
                    row.conditions,
                    row.values,
                    &bounds[..length],
                    max_nodes,
                    max_operands,
                ),
            (Self::Ordinal(rules), NumericDomain::Ordinal { rank_count }) => transaction
                .push_extremum_cut_family_range(
                    ExtremumRow {
                        conditions: row.conditions,
                        ranks: row.values,
                        rank_count,
                    },
                    &rules[..length],
                    max_nodes,
                    max_operands,
                ),
            _ => Err(AdmissionError::NumericDomain),
        }
    }
}
