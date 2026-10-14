//! Borrowed numeric identity over the authoritative formula columns.

use std::hash::{BuildHasher, Hash, Hasher};
use zetesis_ferraris::{
    AdmissionError, ExtremumCut, ExtremumCutRule, ExtremumRow, NodeView, NumericDomain, Threshold,
};

use crate::FormulaFailure;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::formula_ground) enum Rule {
    Threshold(i128),
    Extremum(ExtremumCutRule),
}

#[derive(Clone, Copy)]
pub(in crate::formula_ground) struct Row<'a> {
    pub(in crate::formula_ground) conditions: &'a [usize],
    pub(in crate::formula_ground) values: &'a [u64],
    pub(in crate::formula_ground) domain: NumericDomain,
}

impl<'a> Row<'a> {
    pub(in crate::formula_ground) fn read(node: NodeView<'a>) -> Option<(Self, Rule)> {
        match node {
            NodeView::Threshold(row) => Some((
                Self {
                    conditions: row.conditions,
                    values: row.weights,
                    domain: NumericDomain::Weights,
                },
                Rule::Threshold(row.bound),
            )),
            NodeView::ExtremumCut(cut) => Some((
                Self {
                    conditions: cut.row.conditions,
                    values: cut.row.ranks,
                    domain: NumericDomain::Ordinal {
                        rank_count: cut.row.rank_count,
                    },
                },
                Rule::Extremum(*cut.rule),
            )),
            _ => None,
        }
    }

    pub(super) fn view<'b>(self, rule: &'b Rule) -> Result<NodeView<'b>, AdmissionError>
    where
        'a: 'b,
    {
        match (self.domain, rule) {
            (NumericDomain::Weights, Rule::Threshold(bound)) => {
                Ok(NodeView::Threshold(Threshold {
                    conditions: self.conditions,
                    weights: self.values,
                    bound: *bound,
                }))
            }
            (NumericDomain::Ordinal { rank_count }, Rule::Extremum(rule)) => {
                Ok(NodeView::ExtremumCut(ExtremumCut {
                    row: ExtremumRow {
                        conditions: self.conditions,
                        ranks: self.values,
                        rank_count,
                    },
                    rule,
                }))
            }
            _ => Err(AdmissionError::NumericDomain),
        }
    }

    pub(super) fn hash<S: BuildHasher>(
        self,
        builder: &S,
        work: &mut impl FnMut() -> Result<(), FormulaFailure>,
    ) -> Result<u64, FormulaFailure> {
        let mut hasher = builder.build_hasher();
        work()?;
        self.domain.hash(&mut hasher);
        work()?;
        self.conditions.len().hash(&mut hasher);
        work()?;
        self.values.len().hash(&mut hasher);
        for (&condition, &value) in self.conditions.iter().zip(self.values) {
            work()?;
            condition.hash(&mut hasher);
            work()?;
            value.hash(&mut hasher);
        }
        Ok(hasher.finish())
    }

    pub(super) fn equal(
        self,
        other: Self,
        work: &mut impl FnMut() -> Result<(), FormulaFailure>,
    ) -> Result<bool, FormulaFailure> {
        if self.domain != other.domain
            || self.conditions.len() != other.conditions.len()
            || self.values.len() != other.values.len()
        {
            return Ok(false);
        }
        for ((a, av), (b, bv)) in self
            .conditions
            .iter()
            .zip(self.values)
            .zip(other.conditions.iter().zip(other.values))
        {
            work()?;
            if a != b || av != bv {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(super) fn hash<S: BuildHasher>(
    builder: &S,
    row: u64,
    rule: Rule,
    work: &mut impl FnMut() -> Result<(), FormulaFailure>,
) -> Result<u64, FormulaFailure> {
    let mut hasher = builder.build_hasher();
    work()?;
    5_usize.hash(&mut hasher);
    work()?;
    row.hash(&mut hasher);
    work()?;
    rule.hash(&mut hasher);
    Ok(hasher.finish())
}
