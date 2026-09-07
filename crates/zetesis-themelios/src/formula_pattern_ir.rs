//! Compile positive structural patterns without enumerating constructor values.

use themelios_program::program::{Arguments, Atom};
use themelios_program::term::{Term, UnaryOp, Variable};
use zetesis_core::{AtomPattern, Predicate, Sign, Term as CoreTerm};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_ir::{Compiler, Variables};
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode, push, reserve};
use crate::{
    AdmissionFailure, ExpansionResource, FormulaFailure, FormulaResource, ProfileFeature, compile,
};

impl Compiler<'_> {
    pub(super) fn positive_pattern(
        &mut self,
        atom: &Atom,
        variables: &mut Variables,
    ) -> Result<Option<PatternAtom>, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Ok(None);
        };
        if !arguments.iter().any(pattern_candidate) {
            return Ok(None);
        }
        ceiling(
            FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        let mut patterns = Vec::new();
        reserve(&mut terms, arguments.len(), self.budget, self.location)?;
        for (position, term) in arguments.iter().enumerate() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            terms.push(match term {
                Term::Variable(variable) => {
                    CoreTerm::Variable(self.pattern_slot(variable, variables)?)
                }
                Term::Symbolic(symbol) => {
                    let value = compile::scalar(symbol, self.location)?;
                    self.value(&value)?;
                    CoreTerm::Constant(value)
                }
                term if pattern_candidate(term) => {
                    // Every structured source argument has its own whole-value
                    // capture. Emission uses that capture, including wildcards.
                    let slot = self.pattern_slot(&Variable::Anonymous, variables)?;
                    let nodes = self.structural_pattern(term, variables)?;
                    push(
                        &mut patterns,
                        ArgumentPattern { position, nodes },
                        self.budget,
                        self.location,
                    )?;
                    CoreTerm::Variable(slot)
                }
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            });
        }
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            atom.name.as_str().len() as u128,
            self.location,
        )?;
        let predicate = Predicate::with_sign(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )
        .map_err(|error| AdmissionFailure::Construction {
            error,
            location: self.location,
        })?;
        let atom =
            AtomPattern::new(predicate, terms).map_err(|error| AdmissionFailure::Construction {
                error,
                location: self.location,
            })?;
        Ok(Some(PatternAtom {
            atom,
            arguments: patterns,
        }))
    }

    /// The pending stack contains unvisited source subtrees in reverse preorder.
    /// Each iteration consumes one root; constructor children partition its
    /// remaining subtree. Only sign wrappers may disappear, one charged step at
    /// a time. Work and retained plan/stack cells are bounded by the source tree.
    fn structural_pattern(
        &mut self,
        term: &Term,
        variables: &mut Variables,
    ) -> Result<Vec<PatternNode>, FormulaFailure> {
        let mut pending = Vec::new();
        let mut nodes = Vec::new();
        push(&mut pending, term, self.budget, self.location)?;
        while let Some(term) = pending.pop() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            self.budget
                .charge(ExpansionResource::Values, 1, self.location)?;
            let node = match term {
                Term::Tuple(children) => {
                    reserve(&mut pending, children.len(), self.budget, self.location)?;
                    pending.extend(children.iter().rev());
                    PatternNode::Tuple(children.len())
                }
                Term::Variable(Variable::Anonymous) => PatternNode::Wildcard,
                Term::Variable(variable) => {
                    PatternNode::Slot(self.pattern_slot(variable, variables)?)
                }
                Term::Symbolic(symbol) => {
                    let value = compile::scalar(symbol, self.location)?;
                    self.value(&value)?;
                    PatternNode::Constant(value)
                }
                _ => {
                    let (node, children) = self.function_pattern(term)?;
                    reserve(&mut pending, children.len(), self.budget, self.location)?;
                    pending.extend(children.iter().rev());
                    node
                }
            };
            push(&mut nodes, node, self.budget, self.location)?;
        }
        Ok(nodes)
    }

    /// Unary minus changes a function's sign; it never inverts an arithmetic
    /// expression or extracts a value from a negated variable. Every wrapper is
    /// consumed before the finite function root is admitted.
    fn function_pattern<'a>(
        &mut self,
        mut term: &'a Term,
    ) -> Result<(PatternNode, &'a [Term]), FormulaFailure> {
        let mut sign = Sign::Positive;
        while let Term::UnaryOperation {
            operator: UnaryOp::Negate,
            argument,
        } = term
        {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            self.budget
                .charge(ExpansionResource::Values, 1, self.location)?;
            sign = match sign {
                Sign::Positive => Sign::Negative,
                Sign::Negative => Sign::Positive,
            };
            term = argument;
        }
        let Term::Function { name, arguments } = term else {
            return Err(unsupported(ProfileFeature::Term, self.location).into());
        };
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            name.as_str().len() as u128,
            self.location,
        )?;
        Ok((
            PatternNode::Function {
                name: name.as_str().to_owned(),
                sign,
                arity: arguments.len(),
            },
            arguments,
        ))
    }

    fn pattern_slot(
        &mut self,
        variable: &Variable,
        variables: &mut Variables,
    ) -> Result<usize, FormulaFailure> {
        let existing = match variable {
            Variable::Named(name) => variables.named.get(name.as_str()).copied(),
            Variable::Anonymous => None,
        };
        if let Some(slot) = existing {
            variables.safe.insert(slot);
            return Ok(slot);
        }
        ceiling(
            FormulaResource::Variables,
            variables.count as u128 + 1,
            self.options.core_limits.max_variables_per_template as u128,
            self.location,
        )?;
        let text = match variable {
            Variable::Named(name) => name.as_str().len(),
            Variable::Anonymous => 0,
        };
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            (text + std::mem::size_of::<usize>()) as u128,
            self.location,
        )?;
        let slot = variables.slot(variable);
        variables.safe.insert(slot);
        Ok(slot)
    }
}

// A leading minus may denote a signed function. Compilation validates the
// complete chain before accepting it as structure; arithmetic remains separate.
fn pattern_candidate(term: &Term) -> bool {
    matches!(
        term,
        Term::Tuple(_)
            | Term::Function { .. }
            | Term::UnaryOperation {
                operator: UnaryOp::Negate,
                ..
            }
    )
}
