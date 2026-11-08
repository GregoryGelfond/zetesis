//! Bounded source patterns for checked candidate equations.
//!
//! A clause names an original root ordinal and its canonical atom coordinates.
//! It is a proposal, not an entailment certificate. The checker rereads those
//! ordinals from the exact admitted owner before trusting an equation. No
//! formula, atom coordinate or reduct is rewritten.

use zetesis_algebra::Quota;

use zetesis_algebra::words::Words;

use crate::{AdmissionError, NodeView, Theory};

use super::{
    budget::{Budget, BudgetError},
    conjunction::Conjunction,
    root_facts::CheckedRootFacts,
    storage::Storage,
};

/// The finite pattern table has at most 256 bits, in four words. This limit
/// bounds recognition only; a larger formula remains available to narrowing.
pub(super) const MAX_SUPPORT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Literal {
    pub atom: usize,
    pub positive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Support {
    pub atoms: [usize; MAX_SUPPORT],
    pub len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Clause {
    pub support: Support,
    /// The body-true pattern forbidden by this original constraint.
    pub forbidden: u8,
    pub root: usize,
}

/// Fixed literal scratch and the output record while decoding one clause.
pub(super) const CLAUSE_SCRATCH_BYTES: usize =
    std::mem::size_of::<[Literal; MAX_SUPPORT]>() + std::mem::size_of::<Clause>();

/// The residual record is constructed while its original clause remains live.
pub(super) const CONDITIONED_SCRATCH_BYTES: usize =
    CLAUSE_SCRATCH_BYTES + std::mem::size_of::<Clause>();

/// A source reread also keeps its independently reconstructed pattern table.
pub(super) const CHECK_SCRATCH_BYTES: usize =
    CONDITIONED_SCRATCH_BYTES + std::mem::size_of::<[u64; 4]>();

/// Read a signed atom without recursively interpreting another formula shape.
/// A previously read view is consumed directly, avoiding another source read.
pub(super) fn literal<Q: Quota + ?Sized>(
    theory: &Theory,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Literal>, BudgetError<Q::Error>> {
    budget.charge(1)?; // inspect the source kind
    let (atom, positive) = match view {
        NodeView::Atom(atom) => (atom, true),
        NodeView::Implies(left, right) => {
            let right = budget.node(theory, right)?;
            budget.charge(1)?;
            if !matches!(right, NodeView::False) {
                budget.charge(1)?;
                return Ok(None);
            }
            let left = budget.node(theory, left)?;
            budget.charge(1)?;
            let NodeView::Atom(atom) = left else {
                budget.charge(1)?;
                return Ok(None);
            };
            (atom, false)
        }
        _ => {
            budget.charge(1)?;
            return Ok(None);
        }
    };
    budget.charge(1)?;
    if atom >= theory.atom_count() {
        budget.charge(1)?;
        return Err(BudgetError::Source(AdmissionError::Atom));
    }
    budget.charge(3)?; // atom, polarity and result tag
    Ok(Some(Literal { atom, positive }))
}

/// Authenticate only the canonical truth formula, falsum implying falsum.
/// Other tautologies are deliberately outside this source view.
pub(super) fn canonical_truth<Q: Quota + ?Sized>(
    theory: &Theory,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let NodeView::Implies(left, right) = view else {
        budget.charge(1)?;
        return Ok(false);
    };
    let left = budget.node(theory, left)?;
    budget.charge(1)?;
    if !matches!(left, NodeView::False) {
        budget.charge(1)?;
        return Ok(false);
    }
    let right = budget.node(theory, right)?;
    budget.charge(2)?;
    Ok(matches!(right, NodeView::False))
}

/// Read a signed clause leaf through exact canonical-TRUE antecedents.
/// Each accepted wrapper moves to a strictly earlier node, so the loop is
/// bounded by the admitted DAG. It neither flattens a hidden conjunction nor
/// treats a different tautology as canonical truth.
pub(super) fn clause_literal<'a, Q: Quota + ?Sized>(
    theory: &'a Theory,
    mut node: usize,
    mut view: NodeView<'a>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Literal>, BudgetError<Q::Error>> {
    loop {
        budget.charge(1)?;
        let NodeView::Implies(left, right) = view else {
            break;
        };
        budget.charge(3)?;
        if left >= node || right >= node {
            budget.charge(1)?;
            return Err(BudgetError::Source(AdmissionError::Edge));
        }
        let antecedent = budget.node(theory, left)?;
        if !canonical_truth(theory, antecedent, budget)? {
            break;
        }
        budget.charge(1)?;
        node = right;
        view = budget.node(theory, node)?;
    }
    literal(theory, view, budget)
}

/// Authenticate a constraint's False consequent from an already-charged view.
/// The body remains unread; callers retain their root ordinal and source grammar.
pub(super) fn constraint_body<Q: Quota + ?Sized>(
    theory: &Theory,
    view: NodeView<'_>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<usize>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let NodeView::Implies(body, consequent) = view else {
        budget.charge(1)?;
        return Ok(None);
    };
    let consequent = budget.node(theory, consequent)?;
    budget.charge(1)?;
    if !matches!(consequent, NodeView::False) {
        budget.charge(1)?;
        return Ok(None);
    }
    Ok(Some(body))
}

/// Recognize a constraint through its nested conjunction occurrences and
/// declared signed-clause views. Duplicate atoms still decline the source,
/// including opposite polarities. The exact constant-true body has no atoms
/// and forbids its sole empty pattern.
pub(super) fn clause<Q: Quota + ?Sized>(
    theory: &Theory,
    ordinal: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Clause>, BudgetError<Q::Error>> {
    budget.charge(2)?; // authenticate root ordinal and copy its node identity
    let root = *theory
        .roots()
        .get(ordinal)
        .ok_or(BudgetError::Source(AdmissionError::Root))?;
    let root_view = budget.node(theory, root)?;
    let Some(body) = constraint_body(theory, root_view, budget)? else {
        return Ok(None);
    };
    let body_view = budget.node(theory, body)?;
    budget.charge(17)?; // fixed literal scratch and its initialized length
    let mut literals = [Literal {
        atom: 0,
        positive: false,
    }; MAX_SUPPORT];
    let mut len = 0;
    if !canonical_truth(theory, body_view, budget)? {
        let mut conjunction = Conjunction::new(storage, budget)?;
        let accepted = conjunction.walk_view(
            theory,
            (body, body_view),
            storage,
            budget,
            |node, view, _, budget| {
                let Some(value) = clause_literal(theory, node, view, budget)? else {
                    budget.charge(1)?;
                    return Ok(false);
                };
                budget.charge(1)?;
                if len == MAX_SUPPORT {
                    budget.charge(1)?;
                    return Ok(false);
                }
                budget.charge(3)?;
                literals[len] = value;
                len += 1;
                budget.charge(1)?;
                Ok(true)
            },
        );
        let released = conjunction.release(storage);
        if !accepted? {
            released?;
            budget.charge(1)?;
            return Ok(None);
        }
        released?;
    }
    for position in 1..len {
        budget.charge(2)?;
        let value = literals[position];
        let mut destination = position;
        while destination > 0 {
            budget.charge(1)?;
            if literals[destination - 1].atom <= value.atom {
                break;
            }
            budget.charge(2)?;
            literals[destination] = literals[destination - 1];
            destination -= 1;
        }
        budget.charge(2)?;
        literals[destination] = value;
    }
    for pair in literals[..len].windows(2) {
        budget.charge(1)?;
        if pair[0].atom == pair[1].atom {
            budget.charge(1)?;
            return Ok(None);
        }
    }
    budget.charge(9)?; // independent output coordinates and pattern accumulator
    let mut atoms = [0; MAX_SUPPORT];
    let mut forbidden = 0;
    for (position, value) in literals[..len].iter().enumerate() {
        budget.charge(1)?;
        atoms[position] = value.atom;
        budget.charge(2)?;
        forbidden |= u8::from(value.positive) << position;
    }
    budget.charge(4)?; // length, pattern, root ordinal and result tag
    Ok(Some(Clause {
        support: Support { atoms, len },
        forbidden,
        root: ordinal,
    }))
}

/// Condition a recognized original clause on independently checked facts.
/// Raw shape, distinctness and size checks precede conditioning. Thus known
/// facts cannot admit an otherwise unsupported clause. A known negative leaf
/// makes the body false; unknown atoms retain both possible truth values.
///
/// Callers account for `CONDITIONED_SCRATCH_BYTES` while this record is built.
pub(super) fn conditioned_clause<Q: Quota + ?Sized>(
    theory: &Theory,
    facts: &CheckedRootFacts,
    ordinal: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Clause>, BudgetError<Q::Error>> {
    let Some(original) = clause(theory, ordinal, storage, budget)? else {
        return Ok(None);
    };
    budget.charge(1)?;
    if !facts.belongs_to(theory) {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(11)?; // eight coordinates, length, pattern and original root
    let mut residual = Clause {
        support: Support {
            atoms: [0; MAX_SUPPORT],
            len: 0,
        },
        forbidden: 0,
        root: original.root,
    };
    for (position, &atom) in original.support.atoms[..original.support.len]
        .iter()
        .enumerate()
    {
        budget.charge(3)?;
        let positive = original.forbidden & (1 << position) != 0;
        if facts.contains(atom, budget)? {
            budget.charge(1)?;
            if !positive {
                budget.charge(1)?;
                return Ok(None);
            }
        } else {
            budget.charge(4)?; // coordinate, shift/union and length
            residual.support.atoms[residual.support.len] = atom;
            residual.forbidden |= u8::from(positive) << residual.support.len;
            residual.support.len += 1;
        }
    }
    budget.charge(1)?;
    Ok(Some(residual))
}

/// Reread the claimed source roots and check the equation at every
/// allowed pattern. The proposal's cached forbidden patterns are never read.
pub(super) fn check_clause_equation<Q: Quota + ?Sized>(
    theory: &Theory,
    facts: &CheckedRootFacts,
    roots: &[Clause],
    support: Support,
    (words, constant): (&[u64], bool),
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if !facts.belongs_to(theory) {
        budget.charge(1)?;
        return Ok(false);
    }
    let Some(coefficients) = local_coefficients(theory, support, words, budget)? else {
        return Ok(false);
    };
    budget.charge(4)?;
    let mut forbidden = Words::<u64, _>::new([0; 4]);
    for proposed in roots {
        budget.charge(1)?;
        let Some(checked) = conditioned_clause(theory, facts, proposed.root, storage, budget)?
        else {
            budget.charge(1)?;
            return Ok(false);
        };
        budget.charge(1)?;
        if checked.support.len != support.len {
            budget.charge(1)?;
            return Ok(false);
        }
        for (actual, expected) in checked.support.atoms.iter().zip(support.atoms) {
            budget.charge(1)?;
            if *actual != expected {
                budget.charge(1)?;
                return Ok(false);
            }
        }
        budget.charge(4)?;
        forbidden.insert(usize::from(checked.forbidden));
    }
    implied_by_table(
        forbidden.storage(),
        support.len,
        coefficients,
        constant,
        budget,
    )
}

/// Check dense coefficients against an authenticated bounded coordinate map.
/// Every padded word is inspected; no coefficient outside that map is accepted.
pub(super) fn local_coefficients<Q: Quota + ?Sized>(
    theory: &Theory,
    support: Support,
    words: &[u64],
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<u8>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if support.len > MAX_SUPPORT {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(1)?;
    if words.len() != theory.atom_count().div_ceil(128) * 2 {
        budget.charge(1)?;
        return Ok(None);
    }
    budget.charge(1)?;
    let mut coefficients = 0_u8;
    for (word_id, &word) in words.iter().enumerate() {
        budget.charge(1)?;
        let mut permitted = 0_u64;
        for (local, &atom) in support.atoms[..support.len].iter().enumerate() {
            budget.charge(2)?; // coordinate word and membership comparison
            if atom / 64 == word_id {
                budget.charge(3)?;
                let bit = 1_u64 << (atom % 64);
                permitted |= bit;
                budget.charge(2)?;
                if word & bit != 0 {
                    budget.charge(2)?;
                    coefficients |= 1 << local;
                }
            }
        }
        budget.charge(3)?;
        if word & !permitted != 0 {
            budget.charge(1)?;
            return Ok(None);
        }
    }
    budget.charge(1)?;
    Ok(Some(coefficients))
}

/// The independent finite-pattern check shared by both parity sources.
pub(super) fn implied_by_table<Q: Quota + ?Sized>(
    forbidden: &[u64; 4],
    support_len: usize,
    coefficients: u8,
    constant: bool,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    let extent = 1_u16 << support_len;
    for pattern in 0..extent {
        budget.charge(5)?;
        let value = (pattern & u16::from(coefficients)).count_ones() & 1 != 0;
        if value != constant {
            budget.charge(5)?;
            if !Words::<u64, _>::new(forbidden).contains(usize::from(pattern)) {
                budget.charge(1)?;
                return Ok(false);
            }
        }
    }
    budget.charge(1)?;
    Ok(true)
}
