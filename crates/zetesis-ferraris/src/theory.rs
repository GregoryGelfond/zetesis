use std::fmt;
use std::sync::Arc;

/// A Boolean circuit node. Connectives refer only to preceding node indices.
/// Default negation is implication to falsum, not a complement in the reduct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    /// A proposition in the theory's finite atom universe.
    Atom(usize),
    /// Falsum.
    False,
    /// Conjunction of two preceding nodes.
    And(usize, usize),
    /// Disjunction of two preceding nodes.
    Or(usize, usize),
    /// Classical implication between two preceding nodes.
    Implies(usize, usize),
}

/// Explicit bounds on an admitted finite formula DAG.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Maximum number of propositions, including unsupported atoms.
    pub max_atoms: usize,
    /// Maximum number of circuit nodes.
    pub max_nodes: usize,
    /// Maximum number of asserted root formulas.
    pub max_roots: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_atoms: 65_536,
            max_nodes: 1_048_576,
            max_roots: 262_144,
        }
    }
}

/// Invalid shape, foreign interpretation, or refused storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// An explicit finite shape bound was exceeded.
    Limit,
    /// An atom index lies outside the declared universe.
    Atom,
    /// A circuit edge is forward, cyclic, or out of range.
    Edge,
    /// An asserted root is not a circuit node.
    Root,
    /// Interpretation storage could not be reserved.
    Allocation,
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Limit => "formula admission limit exceeded",
            Self::Atom => "atom is outside the formula universe",
            Self::Edge => "formula edge must refer to a preceding node",
            Self::Root => "formula root is outside the circuit",
            Self::Allocation => "formula interpretation storage could not be reserved",
        })
    }
}
impl std::error::Error for AdmissionError {}

#[derive(Debug)]
struct Data {
    atoms: usize,
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

/// An immutable, instance-identified finite theory in topological order.
#[derive(Clone, Debug)]
pub struct Theory(Arc<Data>);
impl Theory {
    /// Validate a circuit without recursive traversal. The caller owns input
    /// vector construction; these bounds govern admission and later evaluation.
    ///
    /// # Errors
    /// Refuses excessive dimensions, invalid atom/edge/root indices, or a word
    /// count that cannot be represented on this host.
    pub fn new(
        atoms: usize,
        nodes: Vec<Node>,
        roots: Vec<usize>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        if atoms > limits.max_atoms
            || nodes.len() > limits.max_nodes
            || roots.len() > limits.max_roots
            || atoms.checked_add(63).is_none()
        {
            return Err(AdmissionError::Limit);
        }
        for (index, node) in nodes.iter().enumerate() {
            match *node {
                Node::Atom(atom) if atom >= atoms => return Err(AdmissionError::Atom),
                Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b)
                    if a >= index || b >= index =>
                {
                    return Err(AdmissionError::Edge);
                }
                _ => {}
            }
        }
        if roots.iter().any(|root| *root >= nodes.len()) {
            return Err(AdmissionError::Root);
        }
        Ok(Self(Arc::new(Data {
            atoms,
            nodes,
            roots,
        })))
    }

    /// Number of atoms, whether or not they occur in an asserted formula.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.0.atoms
    }

    /// Topologically ordered formula nodes.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.0.nodes
    }

    /// Roots whose conjunction constitutes this theory.
    #[must_use]
    pub fn roots(&self) -> &[usize] {
        &self.0.roots
    }

    /// Clones share identity; independent equal admissions do not.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Packed membership in exactly one immutable theory's finite atom universe.
#[derive(Clone, Debug)]
pub struct Interpretation {
    pub(crate) theory: Theory,
    pub(crate) words: Vec<u64>,
}
impl Interpretation {
    /// Construct an interpretation, coalescing repeated atom indices.
    ///
    /// # Errors
    /// Refuses out-of-universe atoms or failed storage reservation.
    pub fn new(
        theory: &Theory,
        atoms: impl IntoIterator<Item = usize>,
    ) -> Result<Self, AdmissionError> {
        let count = theory.atom_count().div_ceil(64);
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| AdmissionError::Allocation)?;
        words.resize(count, 0);
        for atom in atoms {
            if atom >= theory.atom_count() {
                return Err(AdmissionError::Atom);
            }
            words[atom / 64] |= 1 << (atom % 64);
        }
        Ok(Self {
            theory: theory.clone(),
            words,
        })
    }

    /// The theory instance to which this interpretation belongs.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Membership; an out-of-universe index is false.
    #[must_use]
    pub fn contains(&self, atom: usize) -> bool {
        atom < self.theory.atom_count() && self.words[atom / 64] & (1 << (atom % 64)) != 0
    }

    /// Atom indices in ascending order, without materializing a second carrier.
    pub fn atoms(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.theory.atom_count()).filter(|atom| self.contains(*atom))
    }
}
