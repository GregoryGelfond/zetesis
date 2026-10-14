//! Shared weighted rows and processed-event state for threshold narrowing.
//!
//! Equal complete contribution rows share one numerical index. Repeated
//! conditions are grouped with checked addition; weights retain tuple
//! multiplicity. Immutable term/guard orders let each propagation cursor advance
//! only once along a region branch, independently of pending truth bits.
//! Residual divisibility has a separate coalesced original-row event: each scan
//! reads closed condition knowledge and may refute without adding any bound.

use std::collections::HashMap;

use crate::{NodeView, Theory, Threshold};
use zetesis_cpu::Stop;

use super::adjacency::Adjacency;

mod connectives;
#[cfg(test)]
mod frozen_tests;
#[cfg(test)]
mod incremental_tests;
mod prefixes;
mod propagate;
mod residual;

#[derive(Clone, Debug)]
pub(super) struct Thresholds {
    pub(super) rows: Vec<Row>,
    by_descriptor: Vec<Option<usize>>,
    by_condition: Option<Adjacency>,
    incidences: Vec<Incidence>,
    /// Undirected exact-prefix/equality links, used only on original subjects.
    relations: Option<Adjacency>,
    /// Whether unit rows may exchange requirements with live Boolean chains.
    connectives: Option<connectives::Connectives>,
    /// A non-unit grouped coefficient can expose residual divisibility.
    residuals: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Row {
    /// Grouped positive coefficients, descending by weight then by condition.
    pub(super) terms: Vec<(usize, i128)>,
    /// All guards in ascending (bound, node) order.
    pub(super) guards: Vec<(usize, i128)>,
    total: i128,
}

#[derive(Clone, Copy, Debug)]
struct Incidence {
    condition: usize,
    row: usize,
    weight: i128,
}

/// Sums include only processed condition events. Required bounds include only
/// processed, unmasked guard events and, on original subjects, transported
/// exact-prefix requirements. Cursors retain completed monotone scans;
/// they are cloned with the region and never shared between sibling regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct State {
    known: i128,
    possible: i128,
    lower: i128,
    upper: Option<i128>,
    sure_guard: usize,
    never_guard: usize,
    true_term: usize,
    false_term: usize,
}

impl Row {
    fn new(threshold: Threshold<'_>) -> Result<Self, Stop> {
        let mut terms = reserve(threshold.conditions.len())?;
        let mut total = 0_i128;
        for (&condition, &weight) in threshold.conditions.iter().zip(threshold.weights) {
            if weight != 0 {
                let weight = i128::from(weight);
                total = total.checked_add(weight).ok_or(Stop::InvalidProgram)?;
                terms.push((condition, weight));
            }
        }
        terms.sort_unstable_by_key(|&(condition, _)| condition);
        let mut length = 0;
        for position in 0..terms.len() {
            let (condition, weight) = terms[position];
            if length != 0 && terms[length - 1].0 == condition {
                terms[length - 1].1 = terms[length - 1]
                    .1
                    .checked_add(weight)
                    .ok_or(Stop::InvalidProgram)?;
            } else {
                terms[length] = (condition, weight);
                length += 1;
            }
        }
        terms.truncate(length);
        terms.sort_unstable_by(|&(left, a), &(right, b)| b.cmp(&a).then(left.cmp(&right)));
        Ok(Self {
            terms,
            guards: Vec::new(),
            total,
        })
    }

    fn has_nonunit_coefficient(&self) -> bool {
        self.terms.first().is_some_and(|&(_, weight)| weight > 1)
    }

    fn state(&self) -> State {
        State {
            known: 0,
            possible: self.total,
            lower: 0,
            upper: None,
            sure_guard: 0,
            never_guard: self.guards.len(),
            true_term: 0,
            false_term: 0,
        }
    }
}

impl Thresholds {
    /// Shared anchor projection capacity, including any ordinal-chain postings.
    pub(super) fn connective_storage_bytes(&self) -> u128 {
        self.connectives
            .as_ref()
            .map_or(0, connectives::Connectives::retained_bytes)
    }

    pub(super) fn prepare(
        theory: &Theory,
        chains: &[super::Chain],
        chain_of: &[Option<std::num::NonZeroUsize>],
        extrema: &super::extrema::Extrema,
        shared_connectives: bool,
    ) -> Result<Self, Stop> {
        let nodes = theory.view();
        let mut rows: Vec<Row> = Vec::new();
        let mut shared = HashMap::new();
        let mut by_descriptor = reserve(theory.parts().thresholds().len())?;
        by_descriptor.resize(theory.parts().thresholds().len(), None);
        let mut by_source_row = reserve(theory.parts().rows().len())?;
        by_source_row.resize(theory.parts().rows().len(), None);
        for node in 0..nodes.len() {
            let NodeView::Threshold(threshold) =
                nodes.node(node).map_err(|_| Stop::InvalidProgram)?
            else {
                continue;
            };
            let descriptor = theory.nodes()[node]
                .threshold_descriptor()
                .ok_or(Stop::InvalidProgram)?;
            let source_row = theory.parts().thresholds()[descriptor].row;
            let row = if let Some(row) = by_source_row[source_row] {
                row
            } else {
                let key = (threshold.conditions, threshold.weights);
                let row = if let Some(&row) = shared.get(&key) {
                    row
                } else {
                    let row = rows.len();
                    rows.try_reserve(1).map_err(|_| Stop::Allocation)?;
                    rows.push(Row::new(threshold)?);
                    shared.try_reserve(1).map_err(|_| Stop::Allocation)?;
                    shared.insert(key, row);
                    row
                };
                by_source_row[source_row] = Some(row);
                row
            };
            rows[row]
                .guards
                .try_reserve(1)
                .map_err(|_| Stop::Allocation)?;
            rows[row].guards.push((node, threshold.bound));
            by_descriptor[descriptor] = Some(row);
        }
        let mut incidences = Vec::new();
        let mut residuals = false;
        for (row, data) in rows.iter_mut().enumerate() {
            residuals |= data.has_nonunit_coefficient();
            data.guards
                .sort_unstable_by_key(|&(node, bound)| (bound, node));
            incidences
                .try_reserve(data.terms.len())
                .map_err(|_| Stop::Allocation)?;
            incidences.extend(data.terms.iter().map(|&(condition, weight)| Incidence {
                condition,
                row,
                weight,
            }));
        }
        let by_condition = if rows.is_empty() {
            None
        } else {
            Some(Adjacency::build(
                nodes.len(),
                incidences
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| (edge.condition, index)),
            )?)
        };
        let relations = prefixes::prepare(&rows)?;
        let connectives = connectives::Connectives::prepare(
            &rows,
            chains,
            chain_of,
            extrema,
            shared_connectives,
        )?;
        Ok(Self {
            rows,
            by_descriptor,
            by_condition,
            incidences,
            relations,
            connectives,
            residuals,
        })
    }

    fn row(&self, theory: &Theory, node: usize) -> usize {
        let descriptor = theory.nodes()[node]
            .threshold_descriptor()
            .expect("threshold node");
        self.by_descriptor[descriptor].expect("prepared threshold descriptor")
    }

    fn condition(&self, node: usize) -> &[usize] {
        self.by_condition.as_ref().map_or(&[], |rows| &rows[node])
    }

    pub(super) fn has_relations(&self) -> bool {
        self.relations.is_some() || self.connectives.is_some()
    }

    pub(super) fn has_residuals(&self) -> bool {
        self.residuals
    }

    fn related(&self, row: usize) -> &[usize] {
        self.relations.as_ref().map_or(&[], |links| &links[row])
    }

    /// Original threshold parent occurrences still rank atom splits; the
    /// propagation incidence is smaller, one entry per shared row condition.
    pub(super) fn parent_count(&self, node: usize) -> usize {
        self.condition(node)
            .iter()
            .map(|&edge| self.rows[self.incidences[edge].row].guards.len())
            .sum()
    }

    pub(super) fn dependencies(&self) -> impl Iterator<Item = (usize, usize)> + Clone + '_ {
        self.rows.iter().flat_map(|row| {
            row.guards.iter().flat_map(move |&(parent, _)| {
                row.terms.iter().map(move |&(child, _)| (child, parent))
            })
        })
    }

    pub(super) fn knowledge(&self) -> Box<[State]> {
        self.rows.iter().map(Row::state).collect()
    }
}

fn reserve<T>(length: usize) -> Result<Vec<T>, Stop> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Stop::Allocation)?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use crate::{AdmissionLimits, FormulaNodes, FormulaParts, Node, NodeView, Theory};
    use crate::{Narrower, Narrowing, NarrowingScratch, OriginalSubject, Region, RegionLimits};
    use zetesis_cpu::Cancellation;

    fn theory(conditions: &[usize], weights: &[u64], bounds: &[i128], negative: bool) -> Theory {
        let mut nodes =
            FormulaNodes::new(FormulaParts::new((0..3).map(Node::atom).collect(), vec![]).unwrap());
        let roots = nodes
            .push_threshold_family(conditions, weights, bounds, 100, 100)
            .unwrap();
        let roots = if negative {
            let mut append = nodes.transaction();
            let falsum = append.push(NodeView::False, 100, 100).unwrap();
            let roots = roots
                .into_iter()
                .map(|root| {
                    append
                        .push(NodeView::Implies(root, falsum), 100, 100)
                        .unwrap()
                })
                .collect();
            append.commit();
            roots
        } else {
            roots
        };
        Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
    }

    fn close(theory: &Theory, region: &mut Region) -> Narrowing {
        let narrower = Narrower::new(theory);
        narrower
            .narrow_known(
                OriginalSubject::new(theory, None),
                region,
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .0
    }

    #[test]
    fn repeated_condition_weights_force_one_atom() {
        let theory = theory(&[0, 0, 1], &[2, 3, 1], &[5], false);
        let mut region = Region::all_open(3);
        assert_eq!(
            close(&theory, &mut region),
            Narrowing::Fixed { changed: true }
        );
        assert!(!region.is_open(0));
        assert!(region.is_open(1));
        assert!(!region.cut(0));
    }

    #[test]
    fn failed_threshold_cuts_excess_contributions() {
        let theory = theory(&[0, 1, 2], &[2, 3, 4], &[5], true);
        let mut region = Region::all_open(3);
        assert!(region.hold(0));
        assert_eq!(
            close(&theory, &mut region),
            Narrowing::Fixed { changed: true }
        );
        assert!(!region.hold(1));
        assert!(!region.hold(2));
    }

    #[test]
    fn empty_false_threshold_refutes_the_root() {
        let theory = theory(&[], &[], &[1], false);
        assert_eq!(close(&theory, &mut Region::all_open(3)), Narrowing::Refuted);
    }

    #[test]
    fn propagated_regions_preserve_all_original_models() {
        use crate::{EvaluationLimits, EvaluationWorkspace, Interpretation};
        for weights in [[0, 0, 0, 0], [2, 3, 1, 4], [1, 1, 1, 1]] {
            for bound in -1..=11 {
                for negative in [false, true] {
                    let theory = theory(&[0, 0, 1, 2], &weights, &[bound], negative);
                    for code in 0_u32..27 {
                        let mut region = Region::all_open(3);
                        let mut rest = code;
                        for atom in 0..3 {
                            match rest % 3 {
                                1 => {
                                    assert!(region.hold(atom));
                                }
                                2 => {
                                    assert!(region.cut(atom));
                                }
                                _ => {}
                            }
                            rest /= 3;
                        }
                        let before = region.clone();
                        let outcome = close(&theory, &mut region);
                        for mask in 0_usize..8 {
                            let agrees = |region: &Region| {
                                let mut probe = region.clone();
                                (0..3).all(|atom| {
                                    if mask & (1 << atom) != 0 {
                                        probe.hold(atom)
                                    } else {
                                        probe.cut(atom)
                                    }
                                })
                            };
                            if !agrees(&before) {
                                continue;
                            }
                            let candidate = Interpretation::new(
                                &theory,
                                (0..3).filter(|atom| mask & (1 << atom) != 0),
                            )
                            .unwrap();
                            let mut evaluation = EvaluationWorkspace::default();
                            let result = evaluation
                                .evaluate(
                                    &candidate,
                                    EvaluationLimits::default(),
                                    &Cancellation::default(),
                                )
                                .result
                                .unwrap();
                            if result.is_model() {
                                assert_ne!(
                                    outcome,
                                    Narrowing::Refuted,
                                    "weights={weights:?}, bound={bound}, code={code}, mask={mask}"
                                );
                                assert!(agrees(&region), "narrowing removed an original model");
                            }
                        }
                    }
                }
            }
        }
    }
}
