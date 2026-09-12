use zetesis_ferraris::{Interpretation, Node, Theory};

use crate::search::{Budget, Quota, storage};
use crate::{AdmissionLimits, Assignment, Cnf, Incomplete, Literal};

fn clause<const N: usize>(
    cnf: &mut Cnf,
    mut literals: [Literal; N],
    budget: &mut Budget<'_, impl Quota>,
) -> Result<(), Incomplete> {
    for _ in &literals {
        budget.tick()?;
    }
    cnf.append_slice(&mut literals)?;
    Ok(())
}

fn frozen(
    theory: &Theory,
    candidate: &Interpretation,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Vec<bool>, Incomplete> {
    let mut values = storage(theory.nodes().len())?;
    for node in theory.nodes() {
        budget.tick()?;
        values.push(match *node {
            Node::Atom(atom) => candidate.contains(atom),
            Node::False => false,
            Node::And(a, b) => values[a] && values[b],
            Node::Or(a, b) => values[a] || values[b],
            Node::Implies(a, b) => !values[a] || values[b],
        });
    }
    Ok(values)
}

pub(crate) fn encode<Q: Quota>(
    theory: &Theory,
    candidate: Option<&Interpretation>,
    limits: AdmissionLimits,
    budget: &mut Budget<'_, Q>,
) -> Result<Cnf, Incomplete> {
    budget.tick()?;
    let atoms = theory.atom_count();
    let mut cnf = Cnf::empty(atoms, limits)?;
    if Q::BOUNDED_STORAGE {
        let clauses = theory
            .nodes()
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(theory.roots().len()))
            .and_then(|n| n.checked_add(atoms))
            .and_then(|n| n.checked_add(1))
            .ok_or(Incomplete::CounterOverflow)?;
        let literals = theory
            .nodes()
            .len()
            .checked_mul(7)
            .and_then(|n| n.checked_add(theory.roots().len()))
            .and_then(|n| atoms.checked_mul(2).and_then(|atoms| n.checked_add(atoms)))
            .ok_or(Incomplete::CounterOverflow)?;
        cnf.reserve(clauses, literals)?;
    }
    let mask = candidate
        .map(|candidate| frozen(theory, candidate, budget))
        .transpose()?;
    append_nodes(&mut cnf, theory, mask.as_deref(), budget)?;
    if let Some(candidate) = candidate {
        let mut strict = storage(atoms)?;
        for atom in 0..atoms {
            budget.tick()?;
            let negative = Literal::new(atom, false);
            if candidate.contains(atom) {
                strict.push(negative);
            } else {
                clause(&mut cnf, [negative], budget)?;
            }
        }
        // J⊂M: atoms outside M are false; at least one member of M is false.
        // For M=∅ this is the empty clause, correctly refuting a proper subset.
        cnf.append(strict)?;
    }
    Ok(cnf)
}

/// Extend only the classical outer query; semantic atom identities stay fixed
/// while every new auxiliary gate uses the CNF's fresh variable allocator.
pub(crate) fn restrict(
    cnf: &mut Cnf,
    theory: &Theory,
    budget: &mut Budget<'_>,
) -> Result<(), Incomplete> {
    let checkpoint = cnf.checkpoint();
    let result = budget
        .tick()
        .and_then(|()| append_nodes(cnf, theory, None, budget));
    if result.is_err() {
        cnf.rollback(checkpoint);
    }
    result
}

fn append_nodes<Q: Quota>(
    cnf: &mut Cnf,
    theory: &Theory,
    mask: Option<&[bool]>,
    budget: &mut Budget<'_, Q>,
) -> Result<(), Incomplete> {
    let mut nodes = storage(theory.nodes().len())?;
    let mut gates = HashMap::new();
    if Q::BOUNDED_STORAGE {
        gates
            .try_reserve(theory.nodes().len())
            .map_err(|_| Incomplete::Allocation)?;
    }
    for (index, node) in theory.nodes().iter().enumerate() {
        budget.tick()?;
        // Compaction applies to this classical query only. The original theory
        // remains intact, and the frozen mask was computed before any aliasing.
        let value = if mask.as_ref().is_some_and(|values| !values[index]) {
            Encoded::Constant(false)
        } else {
            match *node {
                Node::False => Encoded::Constant(false),
                Node::Atom(atom) => Encoded::Literal(Literal::new(atom, true)),
                Node::And(a, b) => gate(cnf, &mut gates, false, nodes[a], nodes[b], budget)?,
                Node::Or(a, b) => gate(cnf, &mut gates, true, nodes[a], nodes[b], budget)?,
                Node::Implies(a, b) => {
                    gate(cnf, &mut gates, true, nodes[a].negated(), nodes[b], budget)?
                }
            }
        };
        nodes.push(value);
    }
    for root in theory.roots() {
        budget.tick()?;
        match nodes[*root] {
            Encoded::Constant(true) => (),
            Encoded::Constant(false) => clause(cnf, [], budget)?,
            Encoded::Literal(literal) => clause(cnf, [literal], budget)?,
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Encoded {
    Constant(bool),
    Literal(Literal),
}
impl Encoded {
    const fn negated(self) -> Self {
        match self {
            Self::Constant(value) => Self::Constant(!value),
            Self::Literal(literal) => Self::Literal(literal.negated()),
        }
    }
}

fn gate(
    cnf: &mut Cnf,
    gates: &mut HashMap<(usize, usize), Literal>,
    disjunction: bool,
    left: Encoded,
    right: Encoded,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Encoded, Incomplete> {
    budget.tick()?;
    let (left, right) = match (left, right) {
        (Encoded::Constant(a), Encoded::Constant(b)) => {
            return Ok(Encoded::Constant(if disjunction { a || b } else { a && b }));
        }
        (Encoded::Constant(value), other) | (other, Encoded::Constant(value)) => {
            return Ok(if value == disjunction {
                Encoded::Constant(value)
            } else {
                other
            });
        }
        (Encoded::Literal(a), Encoded::Literal(b)) if a == b => return Ok(Encoded::Literal(a)),
        (Encoded::Literal(a), Encoded::Literal(b)) if a == b.negated() => {
            return Ok(Encoded::Constant(disjunction));
        }
        (Encoded::Literal(a), Encoded::Literal(b)) => (a, b),
    };
    // Classical aliases can make distinct original DAG nodes identical in this
    // query. Reuse a complete equivalence only after computing the frozen mask.
    // Commutative keys never change the original formula or its reduct mask.
    let (left, right) = if disjunction {
        (left.negated(), right.negated())
    } else {
        (left, right)
    };
    let key = (left.min(right).index(), left.max(right).index());
    budget.tick()?;
    if let Some(output) = gates.get(&key) {
        return Ok(Encoded::Literal(if disjunction {
            output.negated()
        } else {
            *output
        }));
    }
    gates.try_reserve(1).map_err(|_| Incomplete::Allocation)?;
    let output = cnf.fresh()?;
    clause(cnf, [output.negated(), left], budget)?;
    clause(cnf, [output.negated(), right], budget)?;
    clause(cnf, [output, left.negated(), right.negated()], budget)?;
    gates.insert(key, output);
    Ok(Encoded::Literal(if disjunction {
        output.negated()
    } else {
        output
    }))
}

pub(crate) fn interpretation(
    theory: &Theory,
    assignment: &Assignment,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Interpretation, Incomplete> {
    let mut selected = storage(theory.atom_count())?;
    for atom in 0..theory.atom_count() {
        budget.tick()?;
        if assignment.value(atom).ok_or(Incomplete::InvalidWitness)? {
            selected.push(atom);
        }
    }
    Interpretation::new(theory, selected).map_err(|_| Incomplete::Allocation)
}

use std::collections::HashMap;

pub(crate) fn scratch_bytes(atoms: u128, nodes: u128, roots: u128, clauses: u128) -> u128 {
    use std::mem::size_of;
    // Frozen values, node aliases, explicitly reserved alias-map entries,
    // retained clause capacities and the largest in-flight submitted clause.
    size_of::<Cnf>() as u128
        + size_of::<HashMap<(usize, usize), Literal>>() as u128
        + nodes
            * (size_of::<bool>() + size_of::<Encoded>() + size_of::<((usize, usize), Literal)>())
                as u128
        + clauses * size_of::<usize>() as u128
        + (7 * nodes + roots + 2 * atoms) * size_of::<usize>() as u128
        + atoms.max(3) * size_of::<Literal>() as u128
}
