//! Necessary support for complete mixed ordinary/atomic-choice theories.

use crate::{AdmissionError, AdmissionLimits, FormulaNodes, NodeView, Theory};
use zetesis_cpu::{Cancellation, Stop};

/// Bounds for constructing a candidate-only support restriction. Formula
/// dimensions bound retained nodes, roots and atom-indexed scratch. Work counts
/// original-node/operand/root visits, head traversal, construction and final validation.
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
/// are iterative; scratch is linear in original nodes, operand occurrences, atoms and the largest
/// distinct head. Work is linear in their traversals and constructed nodes.
#[must_use]
pub fn support_restriction(
    theory: &Theory,
    limits: SupportLimits,
    cancellation: &Cancellation,
) -> SupportAttempt {
    let mut builder = Builder {
        limits,
        cancellation,
        work: 0,
        nodes: FormulaNodes::default(),
    };
    let result = builder.build(theory);
    SupportAttempt {
        result,
        work: builder.work,
    }
}

struct Builder<'a> {
    limits: SupportLimits,
    cancellation: &'a Cancellation,
    work: u64,
    nodes: FormulaNodes,
}

impl Builder<'_> {
    fn tick(&mut self) -> Result<(), SupportError> {
        self.cancellation.poll()?;
        if self.work == self.limits.max_work {
            return Err(Stop::WorkLimit.into());
        }
        self.work += 1;
        Ok(())
    }

    fn push(&mut self, node: NodeView<'_>) -> Result<usize, SupportError> {
        self.tick()?;
        match node {
            NodeView::And(operands) | NodeView::Or(operands) => {
                for _ in operands {
                    self.tick()?;
                }
            }
            NodeView::Implies(_, _) => {
                self.tick()?;
                self.tick()?;
            }
            NodeView::Atom(_) | NodeView::False => {}
        }
        let mut transaction = self.nodes.transaction();
        let index = transaction.push(
            node,
            self.limits.admission.max_nodes,
            self.limits.admission.max_operands,
        )?;
        transaction.commit();
        Ok(index)
    }

    fn ordinary_operands(
        &mut self,
        operands: &[usize],
        ordinary: &[bool],
    ) -> Result<bool, SupportError> {
        let mut all = true;
        for &child in operands {
            self.tick()?;
            all &= ordinary[child];
        }
        Ok(all)
    }

    fn applicable(&mut self, theory: &Theory) -> Result<bool, SupportError> {
        self.cancellation.poll()?;
        if theory.atom_count() > self.limits.admission.max_atoms
            || theory.atom_count() > self.limits.admission.max_roots
            || theory.nodes().len() > self.limits.admission.max_nodes
            || theory.parts().occurrences() > self.limits.admission.max_operands
            || theory.operands().len() > self.limits.admission.max_operands
        {
            return Err(AdmissionError::Limit.into());
        }
        let mut ordinary = reserve(theory.nodes().len())?;
        for index in 0..theory.view().len() {
            self.tick()?;
            ordinary.push(match theory.view().node(index)? {
                NodeView::Atom(_) | NodeView::False => true,
                NodeView::Or(operands) => self.ordinary_operands(operands, &ordinary)?,
                _ => false,
            });
        }
        // Inspect every root before constructing any restriction. A selected
        // subset of producers cannot justify an atom's lack of other support.
        let mut disjunctive = false;
        for &root in theory.roots() {
            self.tick()?;
            let head = match theory.view().node(root)? {
                NodeView::Implies(_, head) => head,
                _ => root,
            };
            if !ordinary[head] && crate::atomic_choice::atom(theory, head).is_none() {
                return Ok(false);
            }
            disjunctive |= ordinary[head] && matches!(theory.view().node(head)?, NodeView::Or(_));
        }
        Ok(disjunctive)
    }

    fn build(&mut self, theory: &Theory) -> Result<Option<Theory>, SupportError> {
        if !self.applicable(theory)? {
            return Ok(None);
        }
        for index in 0..theory.view().len() {
            self.push(theory.view().node(index)?)?;
        }
        let falsum = self.push(NodeView::False)?;
        let verum = self.push(NodeView::Implies(falsum, falsum))?;
        let mut support = self.filled(theory.atom_count(), falsum)?;
        let mut visited_nodes = self.filled(theory.nodes().len(), None)?;
        let mut visited_atoms = self.filled(theory.atom_count(), None)?;
        let mut stack = reserve(theory.nodes().len())?;
        let mut heads = reserve(theory.atom_count())?;
        let mut prefix = reserve(theory.atom_count())?;
        for (ordinal, &root) in theory.roots().iter().enumerate() {
            self.tick()?;
            let (body, head) = match theory.view().node(root)? {
                NodeView::Implies(body, head) => (body, head),
                _ => (verum, root),
            };
            if let Some(atom) = crate::atomic_choice::atom(theory, head) {
                // A selected choice head needs its original body's permission,
                // independently of any other ordinary producer's true heads.
                support[atom] = self.push(NodeView::Or(&[support[atom], body]))?;
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
                match theory.view().node(node)? {
                    NodeView::Atom(atom) if visited_atoms[atom] != Some(ordinal) => {
                        visited_atoms[atom] = Some(ordinal);
                        heads.push(atom);
                    }
                    NodeView::Or(operands) => {
                        // Each original node expands once per root; the pending
                        // stack is bounded by admitted operand occurrences.
                        stack
                            .try_reserve(operands.len())
                            .map_err(|_| AdmissionError::Allocation)?;
                        for &child in operands.iter().rev() {
                            self.tick()?;
                            stack.push(child);
                        }
                    }
                    _ => {}
                }
            }
            prefix.clear();
            let mut before = body;
            for &atom in &heads {
                self.tick()?;
                prefix.push(before);
                let atom_node = self.push(NodeView::Atom(atom))?;
                let negative = self.push(NodeView::Implies(atom_node, falsum))?;
                before = self.push(NodeView::And(&[before, negative]))?;
            }
            let mut after = verum;
            for (&atom, &before) in heads.iter().zip(&prefix).rev() {
                self.tick()?;
                let witness = self.push(NodeView::And(&[before, after]))?;
                support[atom] = self.push(NodeView::Or(&[support[atom], witness]))?;
                let atom_node = self.push(NodeView::Atom(atom))?;
                let negative = self.push(NodeView::Implies(atom_node, falsum))?;
                after = self.push(NodeView::And(&[negative, after]))?;
            }
        }
        let mut roots = reserve(theory.atom_count())?;
        for (atom, body) in support.into_iter().enumerate() {
            self.tick()?;
            let atom = self.push(NodeView::Atom(atom))?;
            roots.push(self.push(NodeView::Implies(atom, body))?);
        }
        // Theory admission recounts each node, then validates nodes, operands
        // and roots. Charge that validation
        // before ownership transfer; a failed attempt retains this work.
        for _ in 0..self.nodes.parts().nodes().len() {
            self.tick()?;
            self.tick()?;
        }
        for _ in 0..self.nodes.parts().occurrences() {
            self.tick()?;
        }
        for _ in 0..roots.len() {
            self.tick()?;
        }
        Theory::new(
            theory.atom_count(),
            std::mem::take(&mut self.nodes).into_parts(),
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
