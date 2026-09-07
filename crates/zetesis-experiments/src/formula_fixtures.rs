//! Bounded original formula DAGs, independent of source-level normalization.

use clap::ValueEnum;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};

use crate::FormulaBenchmarkError;

/// Formula shapes exercise different frozen-reduct obligations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum FormulaFamily {
    /// Independent supported choices: every interpretation is stable.
    Choices,
    /// A positive cycle without facts: only the empty interpretation is stable.
    Cycle,
    /// Asserted conjunctions: precisely the full interpretation is stable.
    Conjunction,
    /// Independent disjunctive pairs, including nonminimal full candidates.
    Disjunction,
    /// Implications inside disjunctions, including disabled M-false subformulas.
    MaskedImplication,
}

impl FormulaFamily {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Choices => "choices",
            Self::Cycle => "cycle",
            Self::Conjunction => "conjunction",
            Self::Disjunction => "disjunction",
            Self::MaskedImplication => "masked-implication",
        }
    }
}

/// A finite formula fixture and reproducible frozen candidate generator.
/// These are synthetic kernel tests, not original-domain solving benchmarks.
#[derive(Clone, Debug)]
pub struct FormulaFixture {
    theory: Theory,
}

impl FormulaFixture {
    /// Construct an unrewritten DAG with at most three nodes per atom plus false.
    ///
    /// # Errors
    /// Refuses atom counts outside 1..=4096, allocation or formula admission.
    pub fn new(family: FormulaFamily, atoms: usize) -> Result<Self, FormulaBenchmarkError> {
        if !(1..=4096).contains(&atoms) {
            return Err(FormulaBenchmarkError::Dimensions);
        }
        let mut nodes = reserve(3 * atoms + 1)?;
        nodes.push(Node::False);
        for atom in 0..atoms {
            nodes.push(Node::Atom(atom));
        }
        let mut roots = reserve(atoms)?;
        for atom in 0..atoms {
            let a = atom + 1;
            let b = (atom + 1) % atoms + 1;
            match family {
                FormulaFamily::Choices => {
                    nodes.push(Node::Implies(a, 0));
                    nodes.push(Node::Or(a, nodes.len() - 1));
                }
                FormulaFamily::Cycle => nodes.push(Node::Implies(a, b)),
                FormulaFamily::Conjunction => nodes.push(Node::And(a, b)),
                FormulaFamily::Disjunction => {
                    if atom % 2 == 1 {
                        continue;
                    }
                    nodes.push(Node::Or(a, if atom + 1 < atoms { b } else { a }));
                }
                FormulaFamily::MaskedImplication => {
                    nodes.push(Node::Implies(a, b));
                    nodes.push(Node::Or(nodes.len() - 1, (atom + 2) % atoms + 1));
                }
            }
            roots.push(nodes.len() - 1);
        }
        let theory = Theory::new(atoms, nodes, roots, AdmissionLimits::default())
            .map_err(FormulaBenchmarkError::Admission)?;
        Ok(Self { theory })
    }

    /// Original instance used by every candidate, CPU check and device query.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Reproducible candidates with empty/full, singleton and mixed worlds.
    /// Larger batches deliberately repeat the 256-pattern stream; an epoch
    /// rotates that stream to exercise frozen-state replacement.
    ///
    /// # Errors
    /// Refuses batch counts outside 1..=4096, allocation or invalid atom indices.
    pub fn candidates(
        &self,
        count: usize,
        epoch: usize,
    ) -> Result<Vec<Interpretation>, FormulaBenchmarkError> {
        if !(1..=4096).contains(&count) {
            return Err(FormulaBenchmarkError::Dimensions);
        }
        let mut result = reserve(count)?;
        for index in 0..count {
            let pattern = index.wrapping_add(epoch.wrapping_mul(17)) % 256;
            let atoms = (0..self.theory.atom_count()).filter(|atom| match pattern {
                0 => false,
                1 => true,
                2 => *atom == 0,
                _ => pattern & (1 << (atom % 8)) != 0,
            });
            result.push(
                Interpretation::new(&self.theory, atoms)
                    .map_err(FormulaBenchmarkError::Admission)?,
            );
        }
        Ok(result)
    }
}

pub(crate) fn reserve<T>(count: usize) -> Result<Vec<T>, FormulaBenchmarkError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| FormulaBenchmarkError::Allocation)?;
    Ok(result)
}
