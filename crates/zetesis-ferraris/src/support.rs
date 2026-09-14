//! Necessary support for complete mixed ordinary/atomic-choice theories.

use crate::{AdmissionError, AdmissionLimits, Node, Theory};
use zetesis_cpu::{Control, Stop};

/// Bounds for constructing a candidate-only support restriction. Formula
/// dimensions bound retained nodes, roots and atom-indexed scratch. Work counts
/// original-node/root visits, head traversal, construction and final validation.
/// A root visit includes bounded exact atomic-choice recognition.
#[derive(Clone, Copy, Debug)]
pub struct SupportLimits {
    /// Bounds on the resulting restriction, including copied original nodes.
    pub admission: AdmissionLimits,
    /// Cumulative construction operations. Zero is an actual ceiling.
    pub max_work: u64,
}

/// A stopped restriction construction, never a logical rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportError {
    /// Formula shape or allocation could not be admitted.
    Admission(AdmissionError),
    /// Cancellation, deadline or the construction work bound was reached.
    Stopped(Stop),
}

impl std::fmt::Display for SupportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(formatter),
            Self::Stopped(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for SupportError {}
impl From<AdmissionError> for SupportError {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}
impl From<Stop> for SupportError {
    fn from(error: Stop) -> Self {
        Self::Stopped(error)
    }
}

/// A complete construction or conservative refusal, with work retained on every
/// path. `None` means an asserted head lies outside the certified grammar, or
/// no ordinary asserted head contains a syntactic disjunction. Choices alone
/// do not trigger this optional construction.
#[derive(Debug)]
pub struct SupportAttempt {
    /// Separate classical restriction over exactly the original atom indices.
    /// Its identity differs from the original, which remains the reduct subject.
    pub result: Result<Option<Theory>, SupportError>,
    /// Operations consumed, including a shape refusal or failed construction.
    pub work: u64,
}

/// Construct necessary supportedness for a complete mixed disjunctive theory.
///
/// Every asserted root must be a disjunction of atoms (possibly with falsum),
/// an exact atomic choice `a or not a`, an implication with either consequent,
/// a default negation, or falsum. Bodies may be arbitrary formulas. Choices
/// accept either operand order and separate occurrences of the same semantic
/// atom; richer or cross-atom alternatives decline the complete certificate.
/// Repeated ordinary head atoms count once, including shared DAGs. A theory
/// without an ordinary syntactic disjunction in a head also declines; choices
/// alone do not introduce this extra construction path.
///
/// For each atom `a`, require a rule whose body is true and whose other distinct
/// heads are false whenever `a` is true, or an atomic choice for `a` whose body
/// is true. An unconditional choice supplies true support; its chosen atom is
/// not a positive body premise. This condition is necessary: otherwise
/// removing `a` preserves every original root's frozen reduct, contradicting
/// minimality. Independent rules can support several heads simultaneously.
/// Neither ranks nor head exclusivity across the whole program are assumed.
///
/// The returned theory is only an outer-query restriction. It must not replace
/// the original theory or be frozen in its place. Recognition and construction
/// are iterative; scratch is linear in original nodes, atoms and the largest
/// distinct head. Work is linear in their traversals and constructed nodes.
#[must_use]
pub fn support_restriction(
    theory: &Theory,
    limits: SupportLimits,
    control: &Control,
) -> SupportAttempt {
    let mut builder = Builder {
        limits,
        control,
        work: 0,
        nodes: Vec::new(),
    };
    let result = builder.build(theory);
    SupportAttempt {
        result,
        work: builder.work,
    }
}

struct Builder<'a> {
    limits: SupportLimits,
    control: &'a Control,
    work: u64,
    nodes: Vec<Node>,
}

impl Builder<'_> {
    fn tick(&mut self) -> Result<(), SupportError> {
        self.control.poll()?;
        if self.work == self.limits.max_work {
            return Err(Stop::WorkLimit.into());
        }
        self.work += 1;
        Ok(())
    }

    fn push(&mut self, node: Node) -> Result<usize, SupportError> {
        self.tick()?;
        if self.nodes.len() == self.limits.admission.max_nodes {
            return Err(AdmissionError::Limit.into());
        }
        self.nodes
            .try_reserve(1)
            .map_err(|_| AdmissionError::Allocation)?;
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }

    fn applicable(&mut self, theory: &Theory) -> Result<bool, SupportError> {
        self.control.poll()?;
        if theory.atom_count() > self.limits.admission.max_atoms
            || theory.atom_count() > self.limits.admission.max_roots
            || theory.nodes().len() > self.limits.admission.max_nodes
        {
            return Err(AdmissionError::Limit.into());
        }
        let mut ordinary = reserve(theory.nodes().len())?;
        for node in theory.nodes() {
            self.tick()?;
            ordinary.push(match *node {
                Node::Atom(_) | Node::False => true,
                Node::Or(left, right) => ordinary[left] && ordinary[right],
                _ => false,
            });
        }
        // Inspect every root before constructing any restriction. A selected
        // subset of producers cannot justify an atom's lack of other support.
        let mut disjunctive = false;
        for &root in theory.roots() {
            self.tick()?;
            let head = match theory.nodes()[root] {
                Node::Implies(_, head) => head,
                _ => root,
            };
            if !ordinary[head] && crate::atomic_choice::atom(theory, head).is_none() {
                return Ok(false);
            }
            disjunctive |= ordinary[head] && matches!(theory.nodes()[head], Node::Or(_, _));
        }
        Ok(disjunctive)
    }

    fn build(&mut self, theory: &Theory) -> Result<Option<Theory>, SupportError> {
        if !self.applicable(theory)? {
            return Ok(None);
        }
        for &node in theory.nodes() {
            self.push(node)?;
        }
        let falsum = self.push(Node::False)?;
        let verum = self.push(Node::Implies(falsum, falsum))?;
        let mut support = self.filled(theory.atom_count(), falsum)?;
        let mut visited_nodes = self.filled(theory.nodes().len(), None)?;
        let mut visited_atoms = self.filled(theory.atom_count(), None)?;
        let mut stack = reserve(theory.nodes().len())?;
        let mut heads = reserve(theory.atom_count())?;
        let mut prefix = reserve(theory.atom_count())?;
        for (ordinal, &root) in theory.roots().iter().enumerate() {
            self.tick()?;
            let (body, head) = match theory.nodes()[root] {
                Node::Implies(body, head) => (body, head),
                _ => (verum, root),
            };
            if let Some(atom) = crate::atomic_choice::atom(theory, head) {
                // A selected choice head needs its original body's permission,
                // independently of any other ordinary producer's true heads.
                support[atom] = self.push(Node::Or(support[atom], body))?;
                continue;
            }
            heads.clear();
            stack.clear();
            stack.push(head);
            while let Some(node) = stack.pop() {
                self.tick()?;
                if visited_nodes[node] == Some(ordinal) {
                    continue;
                }
                visited_nodes[node] = Some(ordinal);
                match theory.nodes()[node] {
                    Node::Atom(atom) if visited_atoms[atom] != Some(ordinal) => {
                        visited_atoms[atom] = Some(ordinal);
                        heads.push(atom);
                    }
                    Node::Or(left, right) => {
                        // Each original node expands once per root, so at most
                        // two pending edges per original node can be retained.
                        stack
                            .try_reserve(2)
                            .map_err(|_| AdmissionError::Allocation)?;
                        stack.extend([right, left]);
                    }
                    _ => {}
                }
            }
            prefix.clear();
            let mut before = body;
            for &atom in &heads {
                self.tick()?;
                prefix.push(before);
                let atom_node = self.push(Node::Atom(atom))?;
                let negative = self.push(Node::Implies(atom_node, falsum))?;
                before = self.push(Node::And(before, negative))?;
            }
            let mut after = verum;
            for (&atom, &before) in heads.iter().zip(&prefix).rev() {
                self.tick()?;
                let witness = self.push(Node::And(before, after))?;
                support[atom] = self.push(Node::Or(support[atom], witness))?;
                let atom_node = self.push(Node::Atom(atom))?;
                let negative = self.push(Node::Implies(atom_node, falsum))?;
                after = self.push(Node::And(negative, after))?;
            }
        }
        let mut roots = reserve(theory.atom_count())?;
        for (atom, body) in support.into_iter().enumerate() {
            self.tick()?;
            let atom = self.push(Node::Atom(atom))?;
            roots.push(self.push(Node::Implies(atom, body))?);
        }
        // Theory admission checks each node and root. Charge that validation
        // before ownership transfer; a failed attempt retains this work.
        for _ in 0..self.nodes.len() {
            self.tick()?;
        }
        for _ in 0..roots.len() {
            self.tick()?;
        }
        Theory::new(
            theory.atom_count(),
            std::mem::take(&mut self.nodes),
            roots,
            self.limits.admission,
        )
        .map(Some)
        .map_err(SupportError::from)
    }

    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, SupportError> {
        let mut values = reserve(count)?;
        for _ in 0..count {
            self.tick()?;
            values.push(value);
        }
        Ok(values)
    }
}

fn reserve<T>(count: usize) -> Result<Vec<T>, SupportError> {
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(count)
        .map_err(|_| AdmissionError::Allocation)?;
    Ok(vector)
}
