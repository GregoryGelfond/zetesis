//! Compile positive structural patterns without enumerating constructor values.

use themelios_program::program::{Arguments, Atom};
use themelios_program::term::{Term, UnaryOp, Variable};
use zetesis_core::{AtomPattern, Predicate, Sign, Term as CoreTerm};

use crate::diagnostic::unsupported;
use crate::formula::ceiling;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
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
        self.pattern_atom(atom, variables, None)
    }

    /// Compile a positive body occurrence and its nonbinding expression checks.
    /// Flat scalar atoms retain the existing path without allocating a plan.
    pub(super) fn positive_literal(
        &mut self,
        atom: &Atom,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<bool, FormulaFailure> {
        let Some(pattern) = self.positive_witness(atom, variables, body)? else {
            return Ok(false);
        };
        body.push(LiteralIr::PatternAtom(pattern));
        Ok(true)
    }

    /// Capture an existing complete row and separately check evaluated positions.
    /// Checks consume extracted or independently bound inputs; they cannot make
    /// an input safe, invert an expression, or establish the captured atom's truth.
    pub(super) fn positive_witness(
        &mut self,
        atom: &Atom,
        variables: &mut Variables,
        checks: &mut Vec<LiteralIr>,
    ) -> Result<Option<PatternAtom>, FormulaFailure> {
        self.pattern_atom(atom, variables, Some(checks))
    }

    fn pattern_atom(
        &mut self,
        atom: &Atom,
        variables: &mut Variables,
        mut checks: Option<&mut Vec<LiteralIr>>,
    ) -> Result<Option<PatternAtom>, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Ok(None);
        };
        let accepts = |term: &Term| {
            if checks.is_some() {
                !matches!(term, Term::Variable(_) | Term::Symbolic(_))
            } else {
                pattern_candidate(term)
            }
        };
        if !arguments.iter().any(accepts) {
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
                term if checks.is_some() || pattern_candidate(term) => {
                    // Every structured source argument has its own whole-value
                    // capture. Emission uses that capture, including wildcards.
                    let slot = self.pattern_slot(&Variable::Anonymous, variables)?;
                    let nodes =
                        self.structural_pattern(term, slot, variables, checks.as_deref_mut())?;
                    // A root expression is already captured by the ordinary
                    // argument slot, including scalar values. Only structure
                    // needs a subtree traversal of a StructuredValue.
                    if !matches!(nodes.as_slice(), [PatternNode::Slot(found)] if *found == slot) {
                        push(
                            &mut patterns,
                            ArgumentPattern { position, nodes },
                            self.budget,
                            self.location,
                        )?;
                    }
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
        root_capture: usize,
        variables: &mut Variables,
        mut checks: Option<&mut Vec<LiteralIr>>,
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
                    if let Some((node, children)) = self.function_pattern(term)? {
                        reserve(&mut pending, children.len(), self.budget, self.location)?;
                        pending.extend(children.iter().rev());
                        node
                    } else if let Some(checks) = &mut checks {
                        let capture = nodes.is_empty().then_some(root_capture);
                        self.argument_check(term, capture, variables, checks)?
                    } else {
                        return Err(unsupported(ProfileFeature::Term, self.location).into());
                    }
                }
            };
            push(&mut nodes, node, self.budget, self.location)?;
        }
        Ok(nodes)
    }

    /// Unary minus changes a function's sign; it never inverts an arithmetic
    /// expression or extracts a value from a negated variable. Every wrapper is
    /// consumed before the finite function root is admitted.
    pub(super) fn function_pattern<'a>(
        &mut self,
        mut term: &'a Term,
    ) -> Result<Option<(PatternNode, &'a [Term])>, FormulaFailure> {
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
            return Ok(None);
        };
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            name.as_str().len() as u128,
            self.location,
        )?;
        Ok(Some((
            PatternNode::Function {
                name: name.as_str().to_owned(),
                sign,
                arity: arguments.len(),
            },
            arguments,
        )))
    }

    /// A source expression may read named inputs but cannot name a private
    /// capture. Keep its equality separate from inferable source comparisons.
    /// Reuse the whole argument capture at a root, and capture a complete
    /// subtree when the evaluated position occurs below a constructor.
    fn argument_check(
        &mut self,
        term: &Term,
        captured: Option<usize>,
        variables: &mut Variables,
        checks: &mut Vec<LiteralIr>,
    ) -> Result<PatternNode, FormulaFailure> {
        let mut bytes = 0_u128;
        for node in term.subterms() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if matches!(node, Term::Pool(_) | Term::Interval { .. }) {
                return Err(unsupported(ProfileFeature::Term, self.location).into());
            }
            bytes += (std::mem::size_of::<Term>() + std::mem::size_of::<Operation>()) as u128;
            bytes += match node {
                Term::Variable(Variable::Named(name)) => name.as_str().len() as u128,
                Term::Symbolic(symbol) => crate::structural_value::symbol_bytes(symbol),
                Term::Function { name, .. } => name.as_str().len() as u128,
                _ => 0,
            };
        }
        // One captured-variable operation and the scheduler's pending/output
        // instruction cells accompany the new check. Reserve the actual source
        // instruction cell separately before pushing it.
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            bytes
                + (std::mem::size_of::<Operation>() + 2 * std::mem::size_of::<LiteralIr>()) as u128,
            self.location,
        )?;
        reserve(checks, 1, self.budget, self.location)?;
        let value = self.expression(term, variables)?;
        for input in value.inputs() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if !variables.argument_inputs.contains(&input) {
                self.budget.charge(
                    ExpansionResource::ScalarBytes,
                    std::mem::size_of::<usize>() as u128,
                    self.location,
                )?;
                variables.argument_inputs.insert(input);
            }
        }
        let captured = match captured {
            Some(captured) => captured,
            None => self.pattern_slot(&Variable::Anonymous, variables)?,
        };
        checks.push(LiteralIr::ArgumentCheck {
            captured: Expression {
                nodes: vec![Operation::Variable(captured)],
            },
            value,
        });
        Ok(PatternNode::Slot(captured))
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
