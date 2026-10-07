//! The admitted normal-rule specialization as a general formula theory.

use zetesis_core::GroundProgram;

use crate::{
    AdmissionError, AdmissionLimits, FormulaNodes, FormulaParts, FormulaTransaction, NodeView,
    Theory,
};

/// Translate an already bounded static normal program into Ferraris formulas.
/// Atom indices remain the input graph's dense indices, including unused atoms.
/// Each rule becomes `body -> head`; constraints imply falsum. Frozen true gates
/// become double negations, false gates single negations. Complete bodies retain
/// one native conjunction; choice gates never become positive antecedents.
///
/// This parses no source. Inclusive node and operand dimensions admit all graph
/// appends. Temporary body rows are bounded by the same operand ceiling. The
/// returned theory has its own interpretation identity.
///
/// # Errors
/// Refuses excessive dimensions, invalid edges, overflow or failed allocation.
pub fn from_ground_program(
    program: &GroundProgram,
    limits: AdmissionLimits,
) -> Result<Theory, AdmissionError> {
    if program.atom_count() > limits.max_atoms || program.rules().len() > limits.max_roots {
        return Err(AdmissionError::Limit);
    }
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..program.atom_count() {
        push(&mut transaction, NodeView::Atom(atom), limits)?;
    }
    let falsum = push(&mut transaction, NodeView::False, limits)?;
    let truth = push(&mut transaction, NodeView::Implies(falsum, falsum), limits)?;
    let mut roots = reserve(program.rules().len())?;
    for rule in program.rules() {
        let count = rule
            .positive()
            .len()
            .checked_add(rule.gate_true().len())
            .and_then(|count| count.checked_add(rule.gate_false().len()))
            .ok_or(AdmissionError::Limit)?;
        if count > limits.max_operands {
            return Err(AdmissionError::Limit);
        }
        let mut body = reserve(count)?;
        body.extend(rule.positive().iter().map(|&atom| atom as usize));
        for &atom in rule.gate_true() {
            let negative = push(
                &mut transaction,
                NodeView::Implies(atom as usize, falsum),
                limits,
            )?;
            body.push(push(
                &mut transaction,
                NodeView::Implies(negative, falsum),
                limits,
            )?);
        }
        for &atom in rule.gate_false() {
            body.push(push(
                &mut transaction,
                NodeView::Implies(atom as usize, falsum),
                limits,
            )?);
        }
        let body = if body.is_empty() {
            truth
        } else {
            push(&mut transaction, NodeView::And(&body), limits)?
        };
        let head = rule.head().map_or(falsum, |atom| atom as usize);
        roots.push(push(
            &mut transaction,
            NodeView::Implies(body, head),
            limits,
        )?);
    }
    transaction.commit();
    Theory::new(program.atom_count(), nodes.into_parts(), roots, limits)
}

fn reserve<T>(count: usize) -> Result<Vec<T>, AdmissionError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    Ok(values)
}

fn push(
    transaction: &mut FormulaTransaction<'_>,
    node: NodeView<'_>,
    limits: AdmissionLimits,
) -> Result<usize, AdmissionError> {
    transaction.push(node, limits.max_nodes, limits.max_operands)
}

/// Translate a static normal program and add candidate-only support conditions.
/// Each atom `a` gets `not not (a -> OR producer_bodies)`. Every normal stable
/// model has such a producer in its least-closure derivation. A passing guard's
/// reduct is true for every tested subset; self-supported cycles still need the
/// minimality oracle. Complete producer disjunctions retain native groups.
///
/// This specialization uses normal-rule structure, not arbitrary formulas.
/// Admission includes all support nodes and every logical operand occurrence.
/// The paired original graph and the copied construction owner overlap briefly.
///
/// # Errors
/// Refuses the same conditions as [`from_ground_program`], including additional
/// support graph dimensions and temporary producer incidence allocation.
pub fn from_ground_program_supported(
    program: &GroundProgram,
    limits: AdmissionLimits,
) -> Result<Theory, AdmissionError> {
    let original = from_ground_program(program, limits)?;
    let root_count = original
        .roots()
        .len()
        .checked_add(original.atom_count())
        .ok_or(AdmissionError::Limit)?;
    if root_count > limits.max_roots {
        return Err(AdmissionError::Limit);
    }
    let (offsets, bodies) = producers(program, &original)?;
    let mut raw_nodes = reserve(original.nodes().len())?;
    raw_nodes.extend_from_slice(original.nodes());
    let mut operands = reserve(original.operands().len())?;
    operands.extend_from_slice(original.operands());
    let mut nodes = FormulaNodes::new(FormulaParts::new(raw_nodes, operands)?);
    let mut roots = reserve(root_count)?;
    roots.extend_from_slice(original.roots());
    let mut transaction = nodes.transaction();
    let falsum = original.atom_count();
    for atom in 0..original.atom_count() {
        let row = &bodies[offsets[atom]..offsets[atom + 1]];
        let body = if row.is_empty() {
            falsum
        } else {
            push(&mut transaction, NodeView::Or(row), limits)?
        };
        let condition = push(&mut transaction, NodeView::Implies(atom, body), limits)?;
        let negative = push(
            &mut transaction,
            NodeView::Implies(condition, falsum),
            limits,
        )?;
        roots.push(push(
            &mut transaction,
            NodeView::Implies(negative, falsum),
            limits,
        )?);
    }
    transaction.commit();
    Theory::new(original.atom_count(), nodes.into_parts(), roots, limits)
}

/// One flat producer incidence row per atom, preserving original rule order.
fn producers(
    program: &GroundProgram,
    original: &Theory,
) -> Result<(Vec<usize>, Vec<usize>), AdmissionError> {
    let length = original
        .atom_count()
        .checked_add(1)
        .ok_or(AdmissionError::Limit)?;
    let mut offsets = reserve(length)?;
    offsets.resize(length, 0usize);
    for rule in program.rules() {
        if let Some(head) = rule.head() {
            offsets[head as usize + 1] += 1;
        }
    }
    for atom in 0..original.atom_count() {
        offsets[atom + 1] += offsets[atom];
    }
    let count = offsets[original.atom_count()];
    let mut bodies = reserve(count)?;
    bodies.resize(count, 0);
    let mut cursors = reserve(original.atom_count())?;
    cursors.extend_from_slice(&offsets[..original.atom_count()]);
    for (rule, &root) in program.rules().iter().zip(original.roots()) {
        if let Some(head) = rule.head() {
            let NodeView::Implies(body, _) = original.view().node(root)? else {
                unreachable!("normal translation constructs implication roots");
            };
            let cursor = &mut cursors[head as usize];
            bodies[*cursor] = body;
            *cursor += 1;
        }
    }
    Ok((offsets, bodies))
}
