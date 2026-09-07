//! The admitted normal-rule specialization as a general formula theory.

use zetesis_core::GroundProgram;

use crate::{AdmissionError, AdmissionLimits, Node, Theory};

/// Translate an already bounded static normal program into Ferraris formulas.
/// Atom indices are exactly the input graph's dense indices, including unused
/// carrier atoms. Each rule becomes `body -> head`; constraints imply falsum.
/// Frozen true guards become double negations and false guards become single
/// negations. In particular, a choice's true guard must not become an ordinary
/// positive antecedent: `not not p -> p` supports either choice, while `p -> p`
/// cannot support a nonempty stable model on its own.
///
/// This performs no source parsing or grounding. All formula storage is counted
/// and admitted before construction; the input graph remains caller-owned.
/// The returned theory has its own interpretation identity.
///
/// # Errors
/// Returns [`AdmissionError::Limit`] for excessive formula dimensions or count
/// overflow, and [`AdmissionError::Allocation`] if storage cannot be reserved.
pub fn from_ground_program(
    program: &GroundProgram,
    limits: AdmissionLimits,
) -> Result<Theory, AdmissionError> {
    if program.atom_count() > limits.max_atoms || program.rules().len() > limits.max_roots {
        return Err(AdmissionError::Limit);
    }
    let mut count = program.atom_count() as u128 + 2;
    for rule in program.rules() {
        let literals = rule.positive().len() as u128
            + rule.gate_true().len() as u128
            + rule.gate_false().len() as u128;
        count += literals.saturating_sub(1)
            + 2 * rule.gate_true().len() as u128
            + rule.gate_false().len() as u128
            + 1;
        if count > limits.max_nodes as u128 {
            return Err(AdmissionError::Limit);
        }
    }
    if count > limits.max_nodes as u128 {
        return Err(AdmissionError::Limit);
    }
    let count = usize::try_from(count).map_err(|_| AdmissionError::Limit)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(program.rules().len())
        .map_err(|_| AdmissionError::Allocation)?;
    nodes.extend((0..program.atom_count()).map(Node::Atom));
    let falsum = push(&mut nodes, Node::False);
    let truth = push(&mut nodes, Node::Implies(falsum, falsum));
    for rule in program.rules() {
        let mut body = None;
        for &atom in rule.positive() {
            conjunct(&mut nodes, &mut body, atom as usize);
        }
        for &atom in rule.gate_true() {
            let negative = push(&mut nodes, Node::Implies(atom as usize, falsum));
            let positive = push(&mut nodes, Node::Implies(negative, falsum));
            conjunct(&mut nodes, &mut body, positive);
        }
        for &atom in rule.gate_false() {
            let negative = push(&mut nodes, Node::Implies(atom as usize, falsum));
            conjunct(&mut nodes, &mut body, negative);
        }
        let head = rule.head().map_or(falsum, |atom| atom as usize);
        roots.push(push(&mut nodes, Node::Implies(body.unwrap_or(truth), head)));
    }
    debug_assert_eq!(nodes.len(), count);
    Theory::new(program.atom_count(), nodes, roots, limits)
}

fn push(nodes: &mut Vec<Node>, node: Node) -> usize {
    let index = nodes.len();
    nodes.push(node);
    index
}

fn conjunct(nodes: &mut Vec<Node>, body: &mut Option<usize>, literal: usize) {
    *body = Some(match *body {
        None => literal,
        Some(previous) => push(nodes, Node::And(previous, literal)),
    });
}

/// Translate a static normal program and add candidate-only support conditions.
/// Each atom `a` gets the guard `not not (a -> OR producer_bodies)`. Every normal
/// stable model has such a producer in its least-closure derivation. For a
/// candidate satisfying the guard, its frozen reduct is true for every tested
/// subset; the added guard therefore cannot remove a countermodel or create
/// support. Self-supported positive cycles still need the minimality oracle.
///
/// This is a specialization justified by the input's normal-rule structure,
/// not a transformation for arbitrary formula theories. It avoids searching
/// classical assignments containing carrier atoms with no possible producer.
/// Formula dimensions include all support guards and producer disjunctions.
///
/// # Errors
/// Returns [`AdmissionError`] for the same reasons as [`from_ground_program`],
/// with additional guard storage included in the admission limits.
pub fn from_ground_program_supported(
    program: &GroundProgram,
    limits: AdmissionLimits,
) -> Result<Theory, AdmissionError> {
    let original = from_ground_program(program, limits)?;
    let roots_count = original.roots().len() as u128 + original.atom_count() as u128;
    if roots_count > limits.max_roots as u128 {
        return Err(AdmissionError::Limit);
    }
    let mut heads = Vec::new();
    heads
        .try_reserve_exact(original.atom_count())
        .map_err(|_| AdmissionError::Allocation)?;
    heads.resize(original.atom_count(), 0usize);
    for rule in program.rules() {
        if let Some(head) = rule.head() {
            heads[head as usize] += 1;
        }
    }
    let count = original.nodes().len() as u128
        + 3 * original.atom_count() as u128
        + heads
            .iter()
            .map(|count| count.saturating_sub(1) as u128)
            .sum::<u128>();
    if count > limits.max_nodes as u128 {
        return Err(AdmissionError::Limit);
    }
    let count = usize::try_from(count).map_err(|_| AdmissionError::Limit)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    nodes.extend_from_slice(original.nodes());
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(usize::try_from(roots_count).map_err(|_| AdmissionError::Limit)?)
        .map_err(|_| AdmissionError::Allocation)?;
    roots.extend_from_slice(original.roots());
    let mut support = Vec::new();
    support
        .try_reserve_exact(original.atom_count())
        .map_err(|_| AdmissionError::Allocation)?;
    support.resize(original.atom_count(), None);
    for (rule, &root) in program.rules().iter().zip(original.roots()) {
        if let Some(head) = rule.head() {
            let Node::Implies(body, _) = original.nodes()[root] else {
                unreachable!("normal translation constructs implication roots");
            };
            let disjunction = &mut support[head as usize];
            *disjunction = Some(match *disjunction {
                None => body,
                Some(previous) => push(&mut nodes, Node::Or(previous, body)),
            });
        }
    }
    // The direct translator puts falsum immediately after the atom nodes.
    let falsum = original.atom_count();
    for (atom, body) in support.into_iter().enumerate() {
        let condition = push(&mut nodes, Node::Implies(atom, body.unwrap_or(falsum)));
        let negative = push(&mut nodes, Node::Implies(condition, falsum));
        roots.push(push(&mut nodes, Node::Implies(negative, falsum)));
    }
    debug_assert_eq!(nodes.len(), count);
    Theory::new(original.atom_count(), nodes, roots, limits)
}
