use zetesis_ferraris::{Interpretation, NodeView, Theory};

use crate::search::{Budget, Quota, storage};
use crate::{AdmissionLimits, Assignment, Cnf, Incomplete, Literal};

pub(crate) fn clause<const N: usize>(
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
    values: &mut Vec<bool>,
    theory: &Theory,
    candidate: &Interpretation,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<(), Incomplete> {
    reserve(values, theory.nodes().len())?;
    for index in 0..theory.view().len() {
        budget.tick()?;
        values.push(
            match theory
                .view()
                .node(index)
                .map_err(|_| Incomplete::InvalidWitness)?
            {
                NodeView::Atom(atom) => candidate.contains(atom),
                NodeView::False => false,
                NodeView::And(operands) => truth_operands(operands, values, true, budget)?,
                NodeView::Or(operands) => truth_operands(operands, values, false, budget)?,
                NodeView::Implies(a, b) => {
                    budget.tick()?;
                    let left = values[a];
                    budget.tick()?;
                    let right = values[b];
                    !left || right
                }
            },
        );
    }
    Ok(())
}

fn truth_operands(
    operands: &[usize],
    values: &[bool],
    conjunction: bool,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<bool, Incomplete> {
    let mut value = conjunction;
    for &child in operands {
        budget.tick()?;
        let truth = values[child];
        value = if conjunction {
            value & truth
        } else {
            value | truth
        };
    }
    Ok(value)
}

pub(crate) fn encode<Q: Quota>(
    theory: &Theory,
    candidate: Option<&Interpretation>,
    limits: AdmissionLimits,
    budget: &mut Budget<'_, Q>,
) -> Result<Cnf, Incomplete> {
    let mut workspace = Workspace::default();
    workspace.encode(theory, candidate, limits, budget)?;
    Ok(workspace.cnf.expect("completed encoding owns its CNF"))
}

/// Allocation ownership only: no candidate-dependent encoding survives reset.
#[derive(Debug, Default)]
pub(crate) struct Workspace {
    cnf: Option<Cnf>,
    mask: Vec<bool>,
    strict: Vec<Literal>,
    nodes: Vec<Encoded>,
    gates: HashMap<(usize, usize), Literal>,
}

impl Workspace {
    fn clear(&mut self) {
        self.mask.clear();
        self.strict.clear();
        self.nodes.clear();
        self.gates.clear();
    }

    pub(crate) fn reserve(
        &mut self,
        theory: &Theory,
        limits: AdmissionLimits,
        budget: &mut Budget<'_, impl Quota>,
    ) -> Result<(), Incomplete> {
        let gates = gate_count(theory, budget)?;
        let cnf = self
            .cnf
            .get_or_insert(Cnf::empty(theory.atom_count(), limits)?);
        cnf.reset(theory.atom_count(), limits)?;
        let dimensions = ClauseReservation::new(theory, limits, gates)?;
        cnf.reserve(dimensions.clauses, dimensions.literals)?;
        reserve(&mut self.mask, theory.nodes().len())?;
        reserve(&mut self.nodes, theory.nodes().len())?;
        reserve(&mut self.strict, theory.atom_count())?;
        self.gates
            .try_reserve(gates.saturating_sub(self.gates.len()))
            .map_err(|_| Incomplete::Allocation)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn retained_bytes(&self) -> u128 {
        use std::mem::size_of;
        size_of::<Self>() as u128
            + self.cnf.as_ref().map_or(0, Cnf::retained_bytes)
            + self.mask.capacity() as u128 * size_of::<bool>() as u128
            + self.strict.capacity() as u128 * size_of::<Literal>() as u128
            + self.nodes.capacity() as u128 * size_of::<Encoded>() as u128
            + self.gates.capacity() as u128 * size_of::<((usize, usize), Literal)>() as u128
    }

    pub(crate) fn encode<Q: Quota>(
        &mut self,
        theory: &Theory,
        candidate: Option<&Interpretation>,
        limits: AdmissionLimits,
        budget: &mut Budget<'_, Q>,
    ) -> Result<&Cnf, Incomplete> {
        // The entry operation polls before clearing the previous query. The
        // bounded table clear is an encoding reset, not a logical gate reuse.
        budget.tick()?;
        self.clear();
        if Q::BOUNDED_STORAGE {
            self.reserve(theory, limits, budget)?;
        }
        let cnf = self
            .cnf
            .get_or_insert(Cnf::empty(theory.atom_count(), limits)?);
        cnf.reset(theory.atom_count(), limits)?;
        if let Some(candidate) = candidate {
            frozen(&mut self.mask, theory, candidate, budget)?;
        }
        append_nodes(
            cnf,
            theory,
            candidate.map(|_| self.mask.as_slice()),
            &mut self.nodes,
            &mut self.gates,
            budget,
        )?;
        if let Some(candidate) = candidate {
            reserve(&mut self.strict, theory.atom_count())?;
            for atom in 0..theory.atom_count() {
                budget.tick()?;
                let negative = Literal::new(atom, false);
                if candidate.contains(atom) {
                    self.strict.push(negative);
                } else {
                    clause(cnf, [negative], budget)?;
                }
            }
            // J⊂M: atoms outside M are false; one member of M must be false.
            // M=∅ retains an empty clause, refuting any proper subset.
            cnf.append_slice(&mut self.strict)?;
        }
        Ok(cnf)
    }
}

/// Count the exact number of unsimplified binary CNF gates used by native
/// groups. Only metadata is inspected: every node costs one permit and each
/// arity is read in constant time from the admitted view.
pub(crate) fn gate_count(
    theory: &Theory,
    budget: &mut Budget<'_, impl Quota>,
) -> Result<usize, Incomplete> {
    let mut gates = 0usize;
    for index in 0..theory.view().len() {
        budget.tick()?;
        gates = gates
            .checked_add(node_gates(
                theory
                    .view()
                    .node(index)
                    .map_err(|_| Incomplete::InvalidWitness)?,
            ))
            .ok_or(Incomplete::CounterOverflow)?;
    }
    Ok(gates)
}

pub(crate) fn node_gates(node: NodeView<'_>) -> usize {
    match node {
        NodeView::Atom(_) | NodeView::False => 0,
        NodeView::Implies(_, _) => 1,
        NodeView::And(operands) | NodeView::Or(operands) => operands.len().saturating_sub(1),
    }
}

pub(crate) struct ClauseReservation {
    pub(crate) clauses: usize,
    pub(crate) literals: usize,
}

impl ClauseReservation {
    pub(crate) fn new(
        theory: &Theory,
        limits: AdmissionLimits,
        gates: usize,
    ) -> Result<Self, Incomplete> {
        let atoms = theory.atom_count() as u128;
        let gates = gates as u128;
        let roots = theory.roots().len() as u128;
        let narrow = |count: u128, limit: usize| {
            usize::try_from(count.min(limit as u128)).map_err(|_| Incomplete::CounterOverflow)
        };
        Ok(Self {
            clauses: narrow(3 * gates + roots + atoms + 1, limits.max_clauses)?,
            literals: narrow(7 * gates + roots + 2 * atoms, limits.max_literals)?,
        })
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Incomplete> {
    values
        .try_reserve_exact(count.saturating_sub(values.len()))
        .map_err(|_| Incomplete::Allocation)
}

/// Extend only the classical outer query; semantic atom identities stay fixed
/// while every new auxiliary gate uses the CNF's fresh variable allocator.
pub(crate) fn restrict(
    cnf: &mut Cnf,
    theory: &Theory,
    budget: &mut Budget<'_>,
) -> Result<(), Incomplete> {
    let checkpoint = cnf.checkpoint();
    let mut nodes = Vec::new();
    let mut gates = HashMap::new();
    let result = budget
        .tick()
        .and_then(|()| append_nodes(cnf, theory, None, &mut nodes, &mut gates, budget));
    if result.is_err() {
        cnf.rollback(checkpoint);
    }
    result
}

fn append_nodes<Q: Quota>(
    cnf: &mut Cnf,
    theory: &Theory,
    mask: Option<&[bool]>,
    nodes: &mut Vec<Encoded>,
    gates: &mut HashMap<(usize, usize), Literal>,
    budget: &mut Budget<'_, Q>,
) -> Result<(), Incomplete> {
    reserve(nodes, theory.nodes().len())?;
    for index in 0..theory.view().len() {
        budget.tick()?;
        // Compaction applies to this classical query only. The original theory
        // remains intact, and the frozen mask was computed before any aliasing.
        let value = if mask.as_ref().is_some_and(|values| !values[index]) {
            Encoded::Constant(false)
        } else {
            match theory
                .view()
                .node(index)
                .map_err(|_| Incomplete::InvalidWitness)?
            {
                NodeView::False => Encoded::Constant(false),
                NodeView::Atom(atom) => Encoded::Literal(Literal::new(atom, true)),
                NodeView::And(operands) => {
                    gate_operands(cnf, gates, false, operands, nodes, budget)?
                }
                NodeView::Or(operands) => gate_operands(cnf, gates, true, operands, nodes, budget)?,
                NodeView::Implies(a, b) => {
                    budget.tick()?;
                    let left = nodes[a].negated();
                    budget.tick()?;
                    let right = nodes[b];
                    gate(cnf, gates, true, left, right, budget)?
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

/// Translate a native group through the existing bounded clause-gate builder.
/// Only this optional classical query introduces auxiliary variables; the
/// authoritative formula and its frozen interpretation stay native and intact.
pub(crate) fn gate_operands(
    cnf: &mut Cnf,
    gates: &mut HashMap<(usize, usize), Literal>,
    disjunction: bool,
    operands: &[usize],
    nodes: &[Encoded],
    budget: &mut Budget<'_, impl Quota>,
) -> Result<Encoded, Incomplete> {
    let mut result = None;
    for &child in operands {
        budget.tick()?;
        let right = nodes[child];
        result = Some(match result {
            None => right,
            Some(left) => gate(cnf, gates, disjunction, left, right, budget)?,
        });
    }
    Ok(result.unwrap_or(Encoded::Constant(!disjunction)))
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Encoded {
    Constant(bool),
    Literal(Literal),
}
impl Encoded {
    pub(crate) const fn negated(self) -> Self {
        match self {
            Self::Constant(value) => Self::Constant(!value),
            Self::Literal(literal) => Self::Literal(literal.negated()),
        }
    }
}

pub(crate) fn gate(
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
    // query. Only already-composed child expressions may share equivalences.
    // Commutative keys never replace original truth or reduct guards.
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
    Interpretation::new(theory, selected).map_err(|error| match error {
        zetesis_ferraris::AdmissionError::Allocation => Incomplete::Allocation,
        // Every selected atom came from the theory's own range. Any other
        // constructor refusal invalidates that representation argument.
        zetesis_ferraris::AdmissionError::Atom
        | zetesis_ferraris::AdmissionError::Arity
        | zetesis_ferraris::AdmissionError::Span
        | zetesis_ferraris::AdmissionError::Edge
        | zetesis_ferraris::AdmissionError::Root
        | zetesis_ferraris::AdmissionError::Limit => Incomplete::InvalidWitness,
    })
}

use std::collections::HashMap;

#[cfg(test)]
mod tests;
