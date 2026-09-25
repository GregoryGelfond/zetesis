//! Anonymous default-negated arguments are existential projection positions.
//!
//! Computed values and finite range rows precede witness matching. Repeated
//! anonymous-subtree inspection can be quadratic in source depth; every visited
//! node is charged to the cumulative term-work ceiling before inspection.

use crate::formula_support::components::{Predicate, Term as CoreTerm};
use themelios_program::program::{Arguments, Atom, DefaultNegation, Relation};
use themelios_program::term::{Term, Variable};
use zetesis_core::ValueNodeRef;

use crate::formula::ceiling;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Projection, Variables};
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode, push, reserve};
use crate::{ExpansionResource, FormulaFailure, FormulaResource};

impl Compiler<'_> {
    pub(super) fn projected_atom(
        &mut self,
        atom: &Atom,
        negation: DefaultNegation,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<Option<LiteralIr>, FormulaFailure> {
        // Classical-negative anonymous occurrences retain their ordinary unsafe
        // slots. Neither sign may make a named input safe in the enclosing scope.
        if negation == DefaultNegation::None
            || atom.sign == themelios_program::symbol::Sign::Negative
        {
            return Ok(None);
        }
        let Arguments::Single(arguments) = &atom.arguments else {
            return Ok(None);
        };
        let mut anonymous = false;
        for term in arguments {
            anonymous |= self.projection_anonymous(term)?;
        }
        if !anonymous {
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
            let term = if matches!(term, Term::Variable(Variable::Anonymous)) {
                None
            } else if self.projection_anonymous(term)? {
                let nodes = self.projection_pattern(term, variables, bindings)?;
                push(
                    &mut patterns,
                    ArgumentPattern { position, nodes },
                    self.budget,
                    self.location,
                )?;
                None
            } else {
                Some(self.projection_value(term, variables, bindings)?)
            };
            terms.push(term);
        }
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            atom.name.as_str().len() as u128,
            self.location,
        )?;
        let predicate = self.predicate(
            atom.name.as_str(),
            terms.len(),
            crate::coherence::core_sign(atom.sign),
        )?;
        let projection = if patterns.is_empty() {
            Projection::Arguments { predicate, terms }
        } else {
            self.projection_witnesses(predicate, terms, patterns, variables)?
        };
        Ok(Some(LiteralIr::ProjectedAtom(negation, projection)))
    }

    /// Evaluate data before enumerating witnesses. In a conditional, each range
    /// row remains a source alternative outside existential witness projection.
    fn projection_value(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<CoreTerm, FormulaFailure> {
        if matches!(term, Term::Symbolic(_) | Term::Variable(_)) {
            return self.domain_term(term, variables);
        }
        let value = self.ranged_expression(term, variables, bindings)?;
        let target = self.consequent_slot(variables)?;
        self.budget.charge(
            ExpansionResource::ScalarBytes,
            std::mem::size_of::<Operation>() as u128,
            self.location,
        )?;
        push(
            bindings,
            LiteralIr::Compare(
                Expression {
                    nodes: vec![Operation::Variable(target)],
                },
                Relation::Eq,
                value,
            ),
            self.budget,
            self.location,
        )?;
        Ok(CoreTerm::Variable(target))
    }

    /// Shapes share the structural matcher's flat preorder vocabulary. Only
    /// constructor descendants can be wildcards: an anonymous arithmetic input
    /// remains an unbound input, never a relation to invert.
    fn projection_pattern(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<Vec<PatternNode>, FormulaFailure> {
        let mut pending = Vec::new();
        let mut nodes = Vec::new();
        push(&mut pending, term, self.budget, self.location)?;
        while let Some(term) = pending.pop() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            self.budget
                .charge(ExpansionResource::Values, 1, self.location)?;
            let node = if matches!(term, Term::Variable(Variable::Anonymous)) {
                PatternNode::Wildcard
            } else if self.projection_anonymous(term)? {
                if let Term::Tuple(children) = term {
                    reserve(&mut pending, children.len(), self.budget, self.location)?;
                    pending.extend(children.iter().rev());
                    PatternNode::Constructor(self.source.constructor(
                        ValueNodeRef::Tuple {
                            arity: children.len(),
                        },
                        self.limits,
                        self.counters,
                        self.location,
                    )?)
                } else if matches!(term, Term::Function { .. })
                    && let Some((node, children)) = self.function_pattern(term)?
                {
                    reserve(&mut pending, children.len(), self.budget, self.location)?;
                    pending.extend(children.iter().rev());
                    node
                } else {
                    match self.projection_value(term, variables, bindings)? {
                        CoreTerm::Constant(value) => PatternNode::Constant(value),
                        CoreTerm::Variable(slot) => PatternNode::Slot(slot),
                    }
                }
            } else {
                match self.projection_value(term, variables, bindings)? {
                    CoreTerm::Constant(value) => PatternNode::Constant(value),
                    CoreTerm::Variable(slot) => PatternNode::Slot(slot),
                }
            };
            push(&mut nodes, node, self.budget, self.location)?;
        }
        Ok(nodes)
    }

    fn projection_anonymous(&mut self, term: &Term) -> Result<bool, FormulaFailure> {
        for node in term.subterms() {
            self.budget
                .charge(ExpansionResource::TermWork, 1, self.location)?;
            if matches!(node, Term::Variable(Variable::Anonymous)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn projection_witnesses(
        &mut self,
        predicate: Predicate,
        terms: Vec<Option<CoreTerm>>,
        arguments: Vec<ArgumentPattern>,
        variables: &Variables,
    ) -> Result<Projection, FormulaFailure> {
        let inputs = variables.count;
        let mut count = inputs;
        let mut captured = Vec::new();
        reserve(&mut captured, terms.len(), self.budget, self.location)?;
        for term in terms {
            captured.push(if let Some(term) = term {
                term
            } else {
                ceiling(
                    FormulaResource::Variables,
                    count as u128 + 1,
                    self.options.core_limits.max_variables_per_template as u128,
                    self.location,
                )?;
                let slot = count;
                count += 1;
                CoreTerm::Variable(slot)
            });
        }
        let atom = self.pattern_from_parts(predicate, &captured)?;
        let mut bindings = Vec::new();
        push(
            &mut bindings,
            LiteralIr::PatternAtom(PatternAtom { atom, arguments }),
            self.budget,
            self.location,
        )?;
        Ok(Projection::Witnesses {
            atom,
            bindings,
            variables: count,
            inputs,
        })
    }
}
