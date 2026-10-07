//! One immutable reduct owner plus disjoint worker and ordered result storage.

use std::mem::size_of;

use zetesis_ferraris::{Interpretation, Theory};

use super::Outcome;
use crate::{Incomplete, Limits, Literal, PreparedReduct, ReductWorkspace};

/// Named live storage for a completion attempt. Shared preparation is counted
/// once; disjoint worker query/transient storage is multiplied by concurrency.
/// Requested vector slots are replaced by actual retained capacities before
/// candidate work. Allocator/Arc metadata, reservation overlap, thread stacks,
/// shared original theory and separate candidate/proposal storage are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionScratch {
    /// Actual retained prepared owner, or a conservative source-shape estimate.
    pub shared_bytes: u64,
    /// One worker's evaluation, parameters, search and witness transients.
    pub query_bytes: u64,
    /// Full ordered results, accepted flags, and returned interpretation slots.
    pub result_bytes: u64,
}

pub(super) fn transient(prepared: &PreparedReduct) -> u128 {
    let (variables, _) = prepared.cnf_shape();
    transient_counts(
        prepared.theory().atom_count() as u128,
        prepared.theory().nodes().len() as u128,
        variables as u128,
    )
}

fn transient_counts(atoms: u128, nodes: u128, variables: u128) -> u128 {
    variables * size_of::<bool>() as u128
        + 2 * nodes * size_of::<bool>() as u128
        + atoms * size_of::<usize>() as u128
        + atoms.div_ceil(64) * size_of::<u64>() as u128
        + size_of::<Interpretation>() as u128
}

impl CompletionScratch {
    /// Admit the shared owner, every result slot and at most `workers` queries.
    /// Zero queries is valid when all supplied certificates decide membership.
    ///
    /// # Errors
    /// Refuses non-fitting fixed/query storage or unrepresentable arithmetic.
    pub fn admit(self, workers: usize, limit: u64) -> Result<(usize, u64), Incomplete> {
        let fixed = self
            .shared_bytes
            .checked_add(self.result_bytes)
            .ok_or(Incomplete::CounterOverflow)?;
        let remaining = limit
            .checked_sub(fixed)
            .ok_or(Incomplete::CompletionScratch)?;
        if workers == 0 {
            return Ok((0, fixed));
        }
        let count = usize::try_from(remaining / self.query_bytes.max(1))
            .unwrap_or(usize::MAX)
            .min(workers);
        if count == 0 {
            return Err(Incomplete::CompletionScratch);
        }
        let bytes = self
            .query_bytes
            .checked_mul(u64::try_from(count).map_err(|_| Incomplete::CounterOverflow)?)
            .and_then(|n| n.checked_add(fixed))
            .ok_or(Incomplete::CounterOverflow)?;
        Ok((count, bytes))
    }
}

pub(super) fn results(candidates: usize) -> Result<CompletionScratch, Incomplete> {
    Ok(CompletionScratch {
        shared_bytes: 0,
        query_bytes: 0,
        result_bytes: narrow(result_bytes(candidates as u128))?,
    })
}

pub(super) fn prepared(
    owner: &PreparedReduct,
    candidates: usize,
) -> Result<CompletionScratch, Incomplete> {
    let (variables, clauses) = owner.cnf_shape();
    Ok(CompletionScratch {
        shared_bytes: narrow(owner.statistics().retained_bytes)?,
        query_bytes: narrow(query_bytes(
            owner.theory().atom_count() as u128,
            owner.theory().nodes().len() as u128,
            owner.parameter_count() as u128,
            variables as u128,
            clauses as u128,
        ))?,
        result_bytes: narrow(result_bytes(candidates as u128))?,
    })
}

pub(super) fn requirements(
    theory: &Theory,
    limits: Limits,
    candidates: usize,
) -> Result<CompletionScratch, Incomplete> {
    counts(
        narrow(theory.atom_count() as u128)?,
        narrow(theory.nodes().len() as u128)?,
        narrow(theory.parts().occurrences() as u128)?,
        narrow(theory.roots().len() as u128)?,
        limits,
        narrow(candidates as u128)?,
    )
}

fn counts(
    atoms: u64,
    nodes: u64,
    operands: u64,
    roots: u64,
    limits: Limits,
    candidates: u64,
) -> Result<CompletionScratch, Incomplete> {
    let (atoms, nodes, roots) = (u128::from(atoms), u128::from(nodes), u128::from(roots));
    let operands = u128::from(operands);
    // Each operand occurrence bounds one classical gate; every node may also
    // be an implication with its parameter guard. These allocation-free source
    // bounds cover the parametric builder,
    // not the former candidate-simplified encoder. Shape admission can refuse
    // before these upper bounds are reached; no preparation occurs here.
    // At most every retained clause contributes one shared unit index.
    let variables =
        (3 * atoms + operands + 2 * nodes).min(limits.reduct_admission.max_variables as u128);
    let clauses = (3 * operands + 3 * nodes + 4 * atoms + roots + 1)
        .min(limits.reduct_admission.max_clauses as u128);
    let literals = (7 * operands + 7 * nodes + 10 * atoms + roots)
        .min(limits.reduct_admission.max_literals as u128);
    Ok(CompletionScratch {
        shared_bytes: narrow(
            crate::prepared_reduct::retained_header_bytes()
                + (2 * clauses + literals + nodes) * size_of::<usize>() as u128,
        )?,
        query_bytes: narrow(query_bytes(atoms, nodes, atoms + nodes, variables, clauses))?,
        result_bytes: narrow(result_bytes(u128::from(candidates)))?,
    })
}

fn query_bytes(atoms: u128, nodes: u128, parameters: u128, variables: u128, clauses: u128) -> u128 {
    // Search scratch already includes its assignment output. The extra
    // verification allowance therefore omits that one output term here.
    (size_of::<ReductWorkspace>() - size_of::<crate::search::Workspace>()) as u128
        + nodes * size_of::<bool>() as u128
        + parameters * size_of::<Literal>() as u128
        + crate::search::scratch_bytes(variables, clauses)
        + transient_counts(atoms, nodes, 0)
}

fn result_bytes(candidates: u128) -> u128 {
    candidates
        * (size_of::<Option<Outcome>>() + size_of::<bool>() + size_of::<Interpretation>()) as u128
}

fn narrow(bytes: u128) -> Result<u64, Incomplete> {
    u64::try_from(bytes).map_err(|_| Incomplete::CounterOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_width_is_included_in_completion_shape_bounds() {
        use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, OperandSpan};
        let width = 129;
        let theory = Theory::new(
            2,
            FormulaParts::new(
                vec![
                    Node::atom(0),
                    Node::atom(1),
                    Node::and_span(OperandSpan {
                        start: 0,
                        length: width,
                    }),
                ],
                (0..width).map(|index| index % 2).collect(),
            )
            .unwrap(),
            vec![2],
            AdmissionLimits::default(),
        )
        .unwrap();
        let source = requirements(&theory, Limits::default(), 1).unwrap();
        let owner = PreparedReduct::prepare(
            &theory,
            crate::ReductPreparationLimits::default(),
            &crate::Cancellation::default(),
        )
        .result
        .unwrap();
        let actual = prepared(&owner, 1).unwrap();
        assert!(owner.cnf_shape().0 > theory.nodes().len());
        assert!(source.shared_bytes >= actual.shared_bytes);
        assert!(source.query_bytes >= actual.query_bytes);
        assert_eq!(source.result_bytes, actual.result_bytes);
    }

    #[test]
    fn logical_byte_overflow_is_refused_before_allocation() {
        assert_eq!(
            counts(u64::MAX, u64::MAX, u64::MAX, u64::MAX, Limits::default(), 1),
            Err(Incomplete::CounterOverflow)
        );
        assert_eq!(
            counts(0, 0, 0, 0, Limits::default(), u64::MAX),
            Err(Incomplete::CounterOverflow)
        );
        let limits = CompletionScratch {
            shared_bytes: 0,
            query_bytes: u64::MAX,
            result_bytes: 1,
        };
        assert_eq!(
            limits.admit(4, u64::MAX),
            Err(Incomplete::CompletionScratch)
        );
        assert_eq!(limits.admit(0, 0), Err(Incomplete::CompletionScratch));
    }
}
