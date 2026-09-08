use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};

use super::{Case, Error, Family, reserve};

const PATTERN_BITS: usize = 8;
const PATTERNS: usize = 1 << PATTERN_BITS;

pub(super) struct Fixture {
    pub theory: Theory,
    pub candidates: Vec<Interpretation>,
}

pub(super) fn build(case: Case) -> Result<Fixture, Error> {
    let atoms = case.atoms.get();
    // Validated dimensions bound this topological circuit by four nodes per atom.
    let mut nodes = reserve(4 * atoms)?;
    nodes.push(Node::False);
    for atom in 0..atoms {
        nodes.push(Node::Atom(atom));
    }
    let mut roots = reserve(atoms - 1)?;
    for atom in 0..atoms - 1 {
        let head = atom + 1;
        let consequent = match case.family {
            Family::Normal => head,
            Family::Choices => {
                nodes.push(Node::Implies(head, 0));
                nodes.push(Node::Or(head, nodes.len() - 1));
                nodes.len() - 1
            }
        };
        let conditional = atom != 0 && (case.family == Family::Normal || atom == atoms - 2);
        let root = if conditional {
            nodes.push(Node::Implies(atom, consequent));
            nodes.len() - 1
        } else {
            consequent
        };
        roots.push(root);
    }
    // Root order is part of the witness contract; it need not be node-ID order.
    roots.reverse();
    let theory =
        Theory::new(atoms, nodes, roots, AdmissionLimits::default()).map_err(Error::Admission)?;
    let mut candidates = reserve(case.candidates.get())?;
    for occurrence in 0..case.candidates.get() {
        let selected = (0..atoms).filter(|atom| match occurrence {
            0 => false,
            1 => true,
            2 => *atom < atoms - 1,
            _ => ((occurrence - 3) % PATTERNS) & (1 << (*atom % PATTERN_BITS)) != 0,
        });
        candidates.push(Interpretation::new(&theory, selected).map_err(Error::Admission)?);
    }
    Ok(Fixture { theory, candidates })
}
