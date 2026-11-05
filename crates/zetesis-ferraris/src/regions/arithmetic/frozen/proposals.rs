//! Groups proposed from the masked DAG: a guard, the clauses that would
//! falsify it, and each clause's base support reached through single-producer
//! chains and the two-atom disjunctive roots.

use super::super::source::Support;

/// A clause of a guard: the rule whose body, with the conjuncts the closed
/// root holds dropped, is falsified by holding (`true`) or cutting (`false`)
/// each atom of its base support in turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Clause {
    pub guard: usize,
    pub rule: usize,
    /// Base atoms in ascending order, without repetition.
    pub support: Vec<usize>,
    /// Per support position: the clause is falsified when the atom is held.
    pub held: Vec<bool>,
}

/// Clauses of one guard over one base support of at most `MAX_SUPPORT` atoms,
/// each with the pattern over the support it forbids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Group {
    pub guard: usize,
    pub support: Support,
    /// Per clause: its rule and the forbidden pattern, bit `i` for `support.atoms[i]`.
    pub clauses: Vec<(usize, u8)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ProposalCounts {
    pub guards: u64,
    pub clauses: u64,
    pub groups: u64,
    pub unsupported_clauses: u64,
}

/// What the masked DAG proposes for one candidate.
#[derive(Debug, Default)]
pub(super) struct Proposals {
    /// Candidate guards: open atoms of the candidate heading at least two
    /// clause-shaped rules.
    pub guards: Vec<usize>,
    /// Every clause reached, whatever its support's size.
    pub clauses: Vec<Clause>,
    /// Parity groups: the clauses of a guard sharing a base support of at most
    /// `MAX_SUPPORT` atoms.
    pub parity: Vec<Group>,
    pub counts: ProposalCounts,
}

use std::mem::size_of;

use zetesis_algebra::{Quota, Stop};

use super::super::{
    budget::{Budget, BudgetError},
    source::MAX_SUPPORT,
};
use crate::{Interpretation, NodeView, Producers, Region, Theory};

/// A chain from a guard's conjunct is followed through at most this many
/// producers; the quantified encodings need three.
const MAX_CHAIN: usize = 8;

/// One base atom reached by a chain, and whether it was reached through the
/// other disjunct of its two-atom root, so that the clause is falsified when
/// the base is cut rather than held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reached {
    base: usize,
    complement: bool,
}

/// The candidate's masked reading: the theory, its producers, the candidate
/// and the truth of every node in it.
#[derive(Clone, Copy)]
struct Masked<'a> {
    theory: &'a Theory,
    producers: &'a Producers,
    candidate: &'a Interpretation,
    truth: &'a [bool],
    root: &'a Region,
}

/// A rule of a guard whose body is a conjunction over atoms, after dropping
/// the conjuncts the closed root holds: the open conjuncts, in body order.
struct Falsifier {
    rule: usize,
    conjuncts: Vec<usize>,
}

/// Propose the guards of `candidate`'s frozen reduct and their clauses.
///
/// A guard is an atom of the candidate not held in `root`, open or already
/// cut there, heading at least two rules whose bodies, true in the candidate,
/// are conjunctions over atoms with at least one conjunct the root does not
/// hold. A conjunct the root cuts still carries its clause when a definition
/// chain leads below it; a base atom the root cuts satisfies its clause,
/// which then proposes nothing.
/// Each open conjunct is followed through single-producer bodies, a single
/// atom or a conjunction of atoms, to base atoms, a base that is one disjunct
/// of a two-atom disjunctive root being read as the lower disjunct, through
/// its complement when it is the higher. A clause whose chain leaves the
/// candidate, reads the same base with both polarities, or runs deeper than
/// `MAX_CHAIN` proposes nothing; a support wider than `MAX_SUPPORT` is kept as
/// a clause and counted as unsupported for parity.
///
/// # Errors
/// The budget's refusals; a producer set of another theory stops as an
/// invalid program.
pub(super) fn propose<Q: Quota + ?Sized>(
    theory: &Theory,
    producers: &Producers,
    candidate: &Interpretation,
    truth: &[bool],
    root: &Region,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Proposals, BudgetError<Q::Error>> {
    budget.charge(2)?;
    if !producers.belongs_to(theory) || !candidate.theory().same_instance(theory) {
        return Err(BudgetError::Stopped(Stop::InvalidProgram));
    }
    let masked = Masked {
        theory,
        producers,
        candidate,
        truth,
        root,
    };
    let mut proposals = Proposals::default();
    let mut falsifiers = Vec::new();
    for guard in (0..theory.atom_count()).filter(|&atom| candidate.contains(atom)) {
        budget.charge(1)?;
        if root.is_held(guard) {
            continue;
        }
        falsifiers.clear();
        for &rule in &producers.by_head[guard] {
            budget.charge(1)?;
            if let Some(falsifier) = falsifier(masked, rule, budget)? {
                falsifiers.push(falsifier);
            }
        }
        budget.charge(1)?;
        if falsifiers.len() < 2 {
            continue;
        }
        proposals.guards.push(guard);
        proposals.counts.guards += 1;
        for falsifier in falsifiers.drain(..) {
            if let Some(clause) = clause(masked, guard, &falsifier, budget)? {
                admit(&mut proposals, clause, budget)?;
            }
        }
    }
    budget.admit_bytes(retained(&proposals))?;
    Ok(proposals)
}

/// The conjuncts of `rule`'s body the root does not hold, when the body is a
/// conjunction over atoms true in the candidate; none when the rule is a
/// choice, a fact, masked, or every conjunct is held.
fn falsifier<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    rule: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Falsifier>, BudgetError<Q::Error>> {
    let producer = &masked.producers.rules[rule];
    budget.charge(2)?;
    let Some(body) = producer.body else {
        return Ok(None);
    };
    if producer.choice || !masked.truth[body] {
        return Ok(None);
    }
    let mut conjuncts = Vec::new();
    if !conjunction(masked, body, 0, &mut conjuncts, budget)? {
        return Ok(None);
    }
    let mut unheld = Vec::new();
    for atom in conjuncts {
        budget.charge(1)?;
        if !masked.root.is_held(atom) {
            unheld.push(atom);
        }
    }
    budget.charge(1)?;
    if unheld.is_empty() {
        return Ok(None);
    }
    Ok(Some(Falsifier {
        rule,
        conjuncts: unheld,
    }))
}

/// Collect the atoms of a conjunction, read through nested conjunction nodes
/// as a builder without wide spans writes them; `false` when an operand is
/// neither an atom nor a conjunction, or the nesting runs deeper than
/// `MAX_CHAIN`.
fn conjunction<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    node: usize,
    depth: usize,
    atoms: &mut Vec<usize>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if depth > MAX_CHAIN {
        return Ok(false);
    }
    match budget.node(masked.theory, node)? {
        NodeView::Atom(atom) => atoms.push(atom),
        NodeView::And(children) => {
            for &child in children {
                if !conjunction(masked, child, depth + 1, atoms, budget)? {
                    return Ok(false);
                }
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// The clause of a falsifier: its conjuncts' chains merged into one ascending
/// base support with the polarity that falsifies each.
fn clause<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    guard: usize,
    falsifier: &Falsifier,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Clause>, BudgetError<Q::Error>> {
    let mut reached = Vec::new();
    for &conjunct in &falsifier.conjuncts {
        if !follow(masked, conjunct, 0, &mut reached, budget)? {
            return Ok(None);
        }
    }
    reached.sort_unstable_by_key(|entry| entry.base);
    let mut support = Vec::new();
    let mut held = Vec::new();
    for entry in reached {
        budget.charge(1)?;
        if support.last() == Some(&entry.base) {
            if held.last() == Some(&!entry.complement) {
                continue;
            }
            return Ok(None);
        }
        support.push(entry.base);
        held.push(!entry.complement);
    }
    Ok(Some(Clause {
        guard,
        rule: falsifier.rule,
        support,
        held,
    }))
}

/// Follow `atom` to its base atoms, appending them to `reached`; `false` when
/// the chain leaves the candidate, runs too deep, or reaches a base the root
/// already cuts, which satisfies the clause.
fn follow<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    atom: usize,
    depth: usize,
    reached: &mut Vec<Reached>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(2)?;
    if !masked.candidate.contains(atom) || depth > MAX_CHAIN {
        return Ok(false);
    }
    let definitions = &masked.producers.by_head[atom];
    for &rule in definitions {
        budget.charge(1)?;
        let producer = &masked.producers.rules[rule];
        if producer.body.is_none() && !producer.choice && producer.heads.len() == 2 {
            let other = if producer.heads[0] == atom {
                producer.heads[1]
            } else {
                producer.heads[0]
            };
            budget.charge(1)?;
            if masked.candidate.contains(other) {
                let base = atom.min(other);
                let complement = other < atom;
                // The clause is falsified by the base held or cut; the root
                // deciding it the other way satisfies the clause already.
                if masked.root.decision(base) == Some(complement) {
                    return Ok(false);
                }
                reached.push(Reached { base, complement });
                return Ok(true);
            }
        }
    }
    budget.charge(1)?;
    if let [rule] = definitions[..] {
        let producer = &masked.producers.rules[rule];
        if let Some(body) = producer.body
            && !producer.choice
            && masked.truth[body]
        {
            match budget.node(masked.theory, body)? {
                NodeView::Atom(next) => {
                    return follow(masked, next, depth + 1, reached, budget);
                }
                NodeView::And(_) => {
                    let mut conjuncts = Vec::new();
                    if !conjunction(masked, body, 0, &mut conjuncts, budget)? {
                        reached.push(Reached {
                            base: atom,
                            complement: false,
                        });
                        return Ok(true);
                    }
                    for next in conjuncts {
                        if !follow(masked, next, depth + 1, reached, budget)? {
                            return Ok(false);
                        }
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
    }
    budget.charge(1)?;
    if masked.root.is_cut(atom) {
        return Ok(false);
    }
    reached.push(Reached {
        base: atom,
        complement: false,
    });
    Ok(true)
}

/// Record a clause, and its parity group when its support fits the table.
fn admit<Q: Quota + ?Sized>(
    proposals: &mut Proposals,
    clause: Clause,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?;
    proposals.counts.clauses += 1;
    if clause.support.len() > MAX_SUPPORT {
        proposals.counts.unsupported_clauses += 1;
    } else {
        let mut support = Support {
            atoms: [0; MAX_SUPPORT],
            len: clause.support.len(),
        };
        support.atoms[..support.len].copy_from_slice(&clause.support);
        let pattern = clause
            .held
            .iter()
            .enumerate()
            .fold(0_u8, |bits, (position, &held)| {
                bits | (u8::from(held) << position)
            });
        let position = proposals
            .parity
            .iter()
            .position(|group| group.guard == clause.guard && group.support == support);
        budget.charge(proposals.parity.len() as u64 + 1)?;
        if let Some(index) = position {
            proposals.parity[index].clauses.push((clause.rule, pattern));
        } else {
            proposals.counts.groups += 1;
            proposals.parity.push(Group {
                guard: clause.guard,
                support,
                clauses: vec![(clause.rule, pattern)],
            });
        }
    }
    proposals.clauses.push(clause);
    Ok(())
}

/// The proposals' retained bytes: headers and actual capacities.
fn retained(proposals: &Proposals) -> u128 {
    let clauses: u128 = proposals
        .clauses
        .iter()
        .map(|clause| {
            (clause.support.capacity() * size_of::<usize>() + clause.held.capacity()) as u128
        })
        .sum();
    let groups: u128 = proposals
        .parity
        .iter()
        .map(|group| (group.clauses.capacity() * size_of::<(usize, u8)>()) as u128)
        .sum();
    size_of::<Proposals>() as u128
        + (proposals.guards.capacity() * size_of::<usize>()) as u128
        + (proposals.clauses.capacity() * size_of::<Clause>()) as u128
        + clauses
        + (proposals.parity.capacity() * size_of::<Group>()) as u128
        + groups
}
