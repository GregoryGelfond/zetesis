//! Groups proposed from the masked DAG: a guard, the clauses that would
//! falsify it, and each clause's base support reached through single-producer
//! chains and the two-atom disjunctive roots.

use std::mem::size_of;

use zetesis_algebra::{Quota, Stop};

use super::super::{
    budget::{Budget, BudgetError},
    source::{MAX_SUPPORT, Support},
    storage::Storage,
};
use crate::{Interpretation, NodeView, Producers, Region, Theory};

/// A clause of a guard: the rule whose body, with the conjuncts the closed
/// root holds dropped, is falsified by holding (`true`) or cutting (`false`)
/// each atom of its base support in turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Clause {
    pub guard: usize,
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
    /// Per clause, the pattern it forbids: bit `i` for `support.atoms[i]`.
    pub patterns: Vec<u8>,
}

/// A clause too wide for a group whose pattern cuts its whole support: no
/// member of the class cuts every atom of it, a lower bound of one that needs
/// no table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Wide {
    pub guard: usize,
    /// Base atoms in ascending order, without repetition.
    pub support: Vec<usize>,
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
    /// Parity groups: the clauses of a guard sharing a base support of at most
    /// `MAX_SUPPORT` atoms. A clause whose support is wider is counted and
    /// proposes no group.
    pub parity: Vec<Group>,
    /// The wider clauses whose pattern cuts their whole support, each once.
    pub wide: Vec<Wide>,
    pub counts: ProposalCounts,
}

/// A body is read through at most this many nested connectives, and a chain
/// followed through at most this many producers; the DAG is finite and each
/// node read is charged, so this bounds the recursion only.
const MAX_NESTING: usize = 1024;

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

/// Propose the guards of `candidate`'s frozen reduct and their clauses.
///
/// A guard is an atom of the candidate not held in `root`, open or already
/// cut there, heading at least two rules of its own (one head) whose bodies,
/// true in the candidate, are conjunctions over atoms with at least one
/// conjunct the root does not hold. A conjunct the root cuts still carries
/// its clause when a definition chain leads below it. A base atom the root
/// decides contributes nothing when its literal is already satisfied, and
/// satisfies the whole clause, which then proposes nothing, when its literal
/// is already false; so the facts the grounder keeps in bodies fall away.
/// Each open conjunct is followed through single-producer bodies, a single
/// atom or a conjunction of atoms, to base atoms, a base that is one disjunct
/// of a two-atom disjunctive root being read as the lower disjunct, through
/// its complement when it is the higher. A clause whose chain leaves the
/// candidate, reads the same base with both polarities, or runs deeper than
/// `MAX_NESTING` proposes nothing; a support wider than `MAX_SUPPORT` is counted
/// as a clause and as unsupported, and proposes no group, an all-cut one being
/// kept as a wide clause.
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
    storage: &mut Storage,
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
    storage.admit_record(size_of::<Proposals>() as u128, budget)?;
    let mut proposals = Proposals::default();
    let mut falsifiers = storage.reserve_bytes(0, budget)?;
    for guard in (0..theory.atom_count()).filter(|&atom| candidate.contains(atom)) {
        budget.charge(1)?;
        if root.is_held(guard) {
            continue;
        }
        for &rule in &producers.by_head[guard] {
            budget.charge(1)?;
            if let Some(falsifier) = falsifier(masked, rule, storage, budget)? {
                storage.push_bytes(&mut falsifiers, falsifier, budget)?;
                storage.embed::<usize>()?;
            }
        }
        budget.charge(1)?;
        let is_guard = falsifiers.len() >= 2;
        if is_guard {
            storage.push_bytes(&mut proposals.guards, guard, budget)?;
            proposals.counts.guards += 1;
        }
        for conjuncts in falsifiers.drain(..) {
            if is_guard && let Some(clause) = clause(masked, guard, &conjuncts, storage, budget)? {
                admit(&mut proposals, &clause, storage, budget)?;
                clause.release(storage)?;
            }
            storage.release_embedded(conjuncts)?;
        }
    }
    storage.release(falsifiers)?;
    debug_assert_eq!(storage.bytes(), retained(&proposals));
    budget.admit_bytes(storage.bytes())?;
    Ok(proposals)
}

/// The conjuncts of `rule`'s body the root does not hold, when the rule has
/// one head and its body is a conjunction over atoms true in the candidate;
/// none when the rule is a choice, a disjunction, a fact, masked, or every
/// conjunct is held.
fn falsifier<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    rule: usize,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Vec<usize>>, BudgetError<Q::Error>> {
    let producer = &masked.producers.rules[rule];
    budget.charge(3)?;
    let Some(body) = producer.body else {
        return Ok(None);
    };
    if producer.choice || producer.heads.len() != 1 || !masked.truth[body] {
        return Ok(None);
    }
    let mut conjuncts = storage.reserve_bytes(0, budget)?;
    if !conjunction(masked, body, 0, &mut conjuncts, storage, budget)? {
        storage.release(conjuncts)?;
        return Ok(None);
    }
    let mut unheld = storage.reserve_bytes(0, budget)?;
    for &atom in &conjuncts {
        budget.charge(1)?;
        if !masked.root.is_held(atom) {
            storage.push_bytes(&mut unheld, atom, budget)?;
        }
    }
    storage.release(conjuncts)?;
    budget.charge(1)?;
    if unheld.is_empty() {
        storage.release(unheld)?;
        return Ok(None);
    }
    Ok(Some(unheld))
}

/// Collect the atoms of a conjunction, read through nested conjunction nodes
/// as a builder without wide spans writes them, and through implications
/// whose antecedent the root holds, as the grounder expands a conditional
/// literal; `false` when an operand is none of these, or the nesting runs
/// deeper than `MAX_NESTING`.
fn conjunction<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    node: usize,
    depth: usize,
    atoms: &mut Vec<usize>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if depth > MAX_NESTING {
        return Ok(false);
    }
    match budget.node(masked.theory, node)? {
        NodeView::Atom(atom) => storage.push_bytes(atoms, atom, budget)?,
        NodeView::And(children) => {
            for &child in children {
                if !conjunction(masked, child, depth + 1, atoms, storage, budget)? {
                    return Ok(false);
                }
            }
        }
        NodeView::Implies(antecedent, consequent) => {
            // The conditional literal's expansion: `fact → conjunct`. An
            // antecedent the root knows true passes to the consequent, one
            // known false makes the implication true, an open one is no
            // conjunction.
            match known(masked, antecedent, depth + 1, budget)? {
                Some(true) => {
                    if !conjunction(masked, consequent, depth + 1, atoms, storage, budget)? {
                        return Ok(false);
                    }
                }
                Some(false) => {}
                None => return Ok(false),
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
    conjuncts: &[usize],
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<Clause>, BudgetError<Q::Error>> {
    let mut reached = storage.reserve_bytes(0, budget)?;
    for &conjunct in conjuncts {
        if !follow(masked, conjunct, 0, &mut reached, storage, budget)? {
            storage.release(reached)?;
            return Ok(None);
        }
    }
    reached.sort_unstable_by_key(|entry| entry.base);
    storage.admit_record(size_of::<Clause>() as u128, budget)?;
    let mut clause = Clause {
        guard,
        support: Vec::new(),
        held: Vec::new(),
    };
    for entry in &reached {
        budget.charge(1)?;
        if clause.support.last() == Some(&entry.base) {
            if clause.held.last() == Some(&!entry.complement) {
                continue;
            }
            storage.release(reached)?;
            clause.release(storage)?;
            return Ok(None);
        }
        storage.push_bytes(&mut clause.support, entry.base, budget)?;
        storage.push_bytes(&mut clause.held, !entry.complement, budget)?;
    }
    storage.release(reached)?;
    Ok(Some(clause))
}

impl Clause {
    fn release(self, storage: &mut Storage) -> Result<(), Stop> {
        storage.release_embedded(self.support)?;
        storage.release_embedded(self.held)?;
        storage.release_record(size_of::<Self>() as u128)
    }
}

/// Follow `atom` to its base atoms, appending them to `reached`; `false` when
/// the chain leaves the candidate, runs too deep, or reaches a base the root
/// already cuts, which satisfies the clause.
fn follow<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    atom: usize,
    depth: usize,
    reached: &mut Vec<Reached>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    budget.charge(2)?;
    if !masked.candidate.contains(atom) || depth > MAX_NESTING {
        return Ok(false);
    }
    let definitions = &masked.producers.by_head[atom];
    for &rule in definitions {
        budget.charge(1)?;
        let producer = &masked.producers.rules[rule];
        if !producer.choice
            && producer.heads.len() == 2
            && disjunction_active(masked, producer.body, budget)?
        {
            let other = if producer.heads[0] == atom {
                producer.heads[1]
            } else {
                producer.heads[0]
            };
            budget.charge(1)?;
            if masked.candidate.contains(other) {
                return settle(
                    masked,
                    atom.min(other),
                    other < atom,
                    reached,
                    storage,
                    budget,
                );
            }
        }
    }
    // The definitions the reduct keeps: a masked rule, whose body is false
    // in the candidate, defines nothing there.
    let mut live = None;
    let mut alive = 0;
    for &rule in definitions {
        budget.charge(1)?;
        let producer = &masked.producers.rules[rule];
        if producer.body.is_none_or(|body| masked.truth[body]) {
            alive += 1;
            live = Some(rule);
        }
    }
    budget.charge(1)?;
    if let (1, Some(rule)) = (alive, live) {
        let producer = &masked.producers.rules[rule];
        // Following a rule body requires it to entail this particular head.
        budget.charge(1)?;
        if producer.heads.len() == 1
            && !producer.choice
            && let Some(body) = producer.body
        {
            match budget.node(masked.theory, body)? {
                NodeView::Atom(next) => {
                    return follow(masked, next, depth + 1, reached, storage, budget);
                }
                NodeView::And(_) => {
                    let mut conjuncts = storage.reserve_bytes(0, budget)?;
                    if !conjunction(masked, body, 0, &mut conjuncts, storage, budget)? {
                        // A body the reader cannot read through, an
                        // aggregate's among them, makes the atom a base of
                        // the clause, ruled by the root as any base is.
                        storage.release(conjuncts)?;
                        return settle(masked, atom, false, reached, storage, budget);
                    }
                    for &next in &conjuncts {
                        if !follow(masked, next, depth + 1, reached, storage, budget)? {
                            storage.release(conjuncts)?;
                            return Ok(false);
                        }
                    }
                    storage.release(conjuncts)?;
                    return Ok(true);
                }
                _ => {}
            }
        }
    }
    budget.charge(1)?;
    settle(masked, atom, false, reached, storage, budget)
}

/// Whether a two-headed disjunctive root is in force at the closed root: a
/// bare disjunction, or one whose body the root knows true, as the grounder
/// guards `y(j) ∨ ny(j)` with the fact `universal(j)` or with the tautology
/// that fact leaves behind.
fn disjunction_active<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    body: Option<usize>,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    let Some(body) = body else {
        return Ok(true);
    };
    budget.charge(1)?;
    if !masked.truth[body] {
        return Ok(false);
    }
    Ok(known(masked, body, 0, budget)? == Some(true))
}

/// What the closed root knows of a node's truth, read structurally: an
/// atom's decision, falsum, and the connectives over what their operands are
/// known to be; `None` when open or read deeper than `MAX_NESTING`.
fn known<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    node: usize,
    depth: usize,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<Option<bool>, BudgetError<Q::Error>> {
    budget.charge(1)?;
    if depth > MAX_NESTING {
        return Ok(None);
    }
    budget.charge(1)?; // Read the candidate's frozen node mask.
    if !masked.truth[node] {
        return Ok(Some(false));
    }
    Ok(match budget.node(masked.theory, node)? {
        NodeView::Atom(atom) => masked.root.decision(atom),
        NodeView::False => Some(false),
        NodeView::And(children) => {
            let mut all = Some(true);
            for &child in children {
                match known(masked, child, depth + 1, budget)? {
                    Some(true) => {}
                    Some(false) => return Ok(Some(false)),
                    None => all = None,
                }
            }
            all
        }
        NodeView::Or(children) => {
            let mut any = Some(false);
            for &child in children {
                match known(masked, child, depth + 1, budget)? {
                    Some(true) => return Ok(Some(true)),
                    Some(false) => {}
                    None => any = None,
                }
            }
            any
        }
        NodeView::Implies(antecedent, consequent) => {
            match (
                known(masked, antecedent, depth + 1, budget)?,
                known(masked, consequent, depth + 1, budget)?,
            ) {
                (Some(false), _) | (_, Some(true)) => Some(true),
                (Some(true), Some(false)) => Some(false),
                _ => None,
            }
        }
        NodeView::Threshold(_) | NodeView::ExtremumCut(_) => None,
    })
}

/// Record a base the clause reads directly (`complement` false, falsified
/// when held) or through its disjunctive partner (`complement` true,
/// falsified when cut), unless the root has decided it: a literal already
/// satisfied contributes nothing, and one already false satisfies the clause,
/// which then proposes nothing.
fn settle<Q: Quota + ?Sized>(
    masked: Masked<'_>,
    base: usize,
    complement: bool,
    reached: &mut Vec<Reached>,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<bool, BudgetError<Q::Error>> {
    if let Some(decided) = masked.root.decision(base) {
        return Ok(decided != complement);
    }
    storage.push_bytes(reached, Reached { base, complement }, budget)?;
    Ok(true)
}

/// Record a clause, and its parity group when its support fits the table, or
/// its wide clause when it does not and its pattern cuts the whole support.
fn admit<Q: Quota + ?Sized>(
    proposals: &mut Proposals,
    clause: &Clause,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
    budget.charge(1)?;
    proposals.counts.clauses += 1;
    if clause.support.len() > MAX_SUPPORT {
        proposals.counts.unsupported_clauses += 1;
        budget.charge(clause.held.len() as u64)?;
        if clause.held.iter().all(|&held| !held) {
            // One comparison per support atom of each wide clause already kept.
            budget.charge((proposals.wide.len() as u64 + 1) * clause.support.len() as u64)?;
            let known = proposals
                .wide
                .iter()
                .any(|wide| wide.guard == clause.guard && wide.support == clause.support);
            if !known {
                budget.charge(clause.support.len() as u64)?;
                let mut support = storage.reserve_bytes(clause.support.len(), budget)?;
                support.extend_from_slice(&clause.support);
                storage.push_bytes(
                    &mut proposals.wide,
                    Wide {
                        guard: clause.guard,
                        support,
                    },
                    budget,
                )?;
                storage.embed::<usize>()?;
            }
        }
    } else {
        storage.with_record(size_of::<Support>() as u128, budget, |storage, budget| {
            admit_group(proposals, clause, storage, budget)
        })?;
    }
    Ok(())
}

/// Group a clause within the table width, under its fixed support scratch.
fn admit_group<Q: Quota + ?Sized>(
    proposals: &mut Proposals,
    clause: &Clause,
    storage: &mut Storage,
    budget: &mut Budget<'_, '_, Q>,
) -> Result<(), BudgetError<Q::Error>> {
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
        storage.push_bytes(&mut proposals.parity[index].patterns, pattern, budget)?;
    } else {
        proposals.counts.groups += 1;
        let mut patterns = storage.reserve_bytes(1, budget)?;
        patterns.push(pattern);
        storage.push_bytes(
            &mut proposals.parity,
            Group {
                guard: clause.guard,
                support,
                patterns,
            },
            budget,
        )?;
        storage.embed::<u8>()?;
    }

    Ok(())
}

/// The proposals' retained bytes: headers and actual capacities.
pub(super) fn retained(proposals: &Proposals) -> u128 {
    let patterns: u128 = proposals
        .parity
        .iter()
        .map(|group| group.patterns.capacity() as u128)
        .sum();
    let wide: u128 = proposals
        .wide
        .iter()
        .map(|clause| (clause.support.capacity() * size_of::<usize>()) as u128)
        .sum();
    size_of::<Proposals>() as u128
        + (proposals.guards.capacity() * size_of::<usize>()) as u128
        + (proposals.parity.capacity() * size_of::<Group>()) as u128
        + patterns
        + (proposals.wide.capacity() * size_of::<Wide>()) as u128
        + wide
}
