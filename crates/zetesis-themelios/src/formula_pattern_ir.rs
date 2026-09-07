//! Compile positive tuple destructuring without enumerating constructor values.

use themelios_program::program::{Arguments, Atom};
use themelios_program::term::{Term, Variable};
use zetesis_core::{AtomPattern, Predicate, Term as CoreTerm};

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
        if !arguments.iter().any(|term| matches!(term, Term::Tuple(_))) {
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
                Term::Tuple(_) => {
                    // Every structured source argument has its own whole-value
                    // capture. Emission uses that capture, including wildcards.
                    let slot = self.pattern_slot(&Variable::Anonymous, variables)?;
                    let nodes = self.tuple_pattern(term, variables)?;
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

    fn tuple_pattern(
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
                // Evaluated terms are consumers, not positive binders. Named
                // nonground constructors remain outside this tuple-only slice.
                _ => return Err(unsupported(ProfileFeature::Term, self.location).into()),
            };
            push(&mut nodes, node, self.budget, self.location)?;
        }
        Ok(nodes)
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
