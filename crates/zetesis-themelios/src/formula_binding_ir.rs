//! Scalar/range bindings and evaluated heads lower to scoped value slots.

use crate::formula_support::components::{Pattern, Term as CoreTerm};
use themelios_program::program::{Arguments, Literal, LiteralInner, Relation};
use themelios_program::term::{Term, Variable};

use crate::diagnostic::unsupported;
use crate::formula_ir::{Compiler, Expression, LiteralIr, Operation, Variables};
use crate::{FormulaFailure, ProfileFeature};

impl Compiler<'_> {
    pub(super) fn binding_comparison(
        &mut self,
        left: &Term,
        relation: Relation,
        right: &Term,
        variables: &mut Variables,
    ) -> Result<LiteralIr, FormulaFailure> {
        let range = match (left, right) {
            (Term::Variable(target), Term::Interval { lower, upper })
            | (Term::Interval { lower, upper }, Term::Variable(target))
                if relation == Relation::Eq =>
            {
                Some((target, lower, upper))
            }
            _ => None,
        };
        if let Some((target, lower, upper)) = range {
            let target = variables.slot(target);
            self.variable_limit(variables)?;
            return Ok(LiteralIr::Range {
                target,
                lower: self.expression(lower, variables)?,
                upper: self.expression(upper, variables)?,
                binder: false,
            });
        }
        self.comparison(left, relation, right, variables)
    }

    pub(super) fn generated_head(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<Pattern, FormulaFailure> {
        use themelios_program::program::DefaultNegation;
        if literal.negation != DefaultNegation::None {
            return Err(unsupported(ProfileFeature::NegatedHead, self.location).into());
        }
        let LiteralInner::Atom(atom) = &literal.inner else {
            return Err(unsupported(ProfileFeature::Head, self.location).into());
        };
        self.generated_atom(atom.get(), variables, body)
    }

    /// One finite value occurrence, with data bindings kept in its caller's scope.
    pub(super) fn generated_term(
        &mut self,
        term: &Term,
        variables: &mut Variables,
        bindings: &mut Vec<LiteralIr>,
    ) -> Result<CoreTerm, FormulaFailure> {
        if matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
            return self.objective_term(term, variables);
        }
        let value = self.ranged_expression(term, variables, bindings)?;
        let target = self.consequent_slot(variables)?;
        self.budget.charge(
            crate::ExpansionResource::ScalarBytes,
            (3 * std::mem::size_of::<LiteralIr>() + std::mem::size_of::<Operation>()) as u128,
            self.location,
        )?;
        bindings.push(LiteralIr::Compare(
            Expression {
                nodes: vec![Operation::Variable(target)],
            },
            Relation::Eq,
            value,
        ));
        Ok(CoreTerm::Variable(target))
    }

    pub(super) fn range_comparison(
        &mut self,
        literal: &Literal,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<bool, FormulaFailure> {
        use themelios_program::program::DefaultNegation;
        let LiteralInner::Comparison(comparison) = &literal.inner else {
            return Ok(false);
        };
        let comparison = comparison.get();
        let has_range = std::iter::once(comparison.first())
            .chain(comparison.steps().map(|(_, term)| term))
            .flat_map(Term::subterms)
            .any(|term| matches!(term, Term::Interval { .. }));
        if !has_range {
            return Ok(false);
        }
        let mut steps = comparison.steps();
        let (relation, right) = steps.next().expect("comparison step");
        if steps.next().is_none() && relation == Relation::Eq {
            if let (value, Term::Interval { lower, upper })
            | (Term::Interval { lower, upper }, value) = (comparison.first(), right)
            {
                let nested = lower
                    .subterms()
                    .chain(upper.subterms())
                    .chain(value.subterms())
                    .any(|term| matches!(term, Term::Interval { .. }));
                if literal.negation == DefaultNegation::None
                    && let Term::Variable(target) = value
                {
                    if !nested {
                        return Ok(false);
                    }
                    let target = variables.slot(target);
                    self.variable_limit(variables)?;
                    let lower = self.ranged_expression(lower, variables, body)?;
                    let upper = self.ranged_expression(upper, variables, body)?;
                    self.budget.charge(
                        crate::ExpansionResource::ScalarBytes,
                        (3 * std::mem::size_of::<LiteralIr>()) as u128,
                        self.location,
                    )?;
                    body.push(LiteralIr::Range {
                        target,
                        lower,
                        upper,
                        binder: false,
                    });
                    return Ok(true);
                }
                if literal.negation == DefaultNegation::NotNot {
                    if !nested {
                        return Ok(false);
                    }
                    let value = self.ranged_expression(value, variables, body)?;
                    let lower = self.ranged_expression(lower, variables, body)?;
                    let upper = self.ranged_expression(upper, variables, body)?;
                    self.budget.charge(
                        crate::ExpansionResource::ScalarBytes,
                        (3 * std::mem::size_of::<LiteralIr>()
                            + std::mem::size_of::<crate::formula_guard::GuardComparison>())
                            as u128,
                        self.location,
                    )?;
                    body.push(LiteralIr::Guard(crate::formula_guard::Guard::Comparisons {
                        negation: literal.negation,
                        comparisons: vec![crate::formula_guard::GuardComparison::Range(
                            value, lower, upper,
                        )],
                    }));
                    return Ok(true);
                }
            }
            if literal.negation == DefaultNegation::None {
                let left = self.ranged_expression(comparison.first(), variables, body)?;
                let right = self.ranged_expression(right, variables, body)?;
                body.push(LiteralIr::Compare(left, relation, right));
                return Ok(true);
            }
        }
        let guard = self.ranged_guard(comparison, literal.negation, variables, body)?;
        body.push(LiteralIr::Guard(guard));
        Ok(true)
    }

    /// Bind arguments independently of the enclosing head literal's polarity.
    pub(super) fn generated_atom(
        &mut self,
        atom: &themelios_program::program::Atom,
        variables: &mut Variables,
        body: &mut Vec<LiteralIr>,
    ) -> Result<Pattern, FormulaFailure> {
        let Arguments::Single(arguments) = &atom.arguments else {
            return Err(unsupported(ProfileFeature::PooledArguments, self.location).into());
        };
        crate::formula::ceiling(
            crate::FormulaResource::Arity,
            arguments.len() as u128,
            self.options.core_limits.max_predicate_arity as u128,
            self.location,
        )?;
        let mut terms = Vec::new();
        for term in arguments {
            if matches!(term, Term::Variable(_) | Term::Symbolic(_)) {
                terms.push(self.domain_term(term, variables)?);
            } else {
                crate::formula::ceiling(
                    crate::FormulaResource::Variables,
                    variables.count as u128 + 1,
                    self.options.core_limits.max_variables_per_template as u128,
                    self.location,
                )?;
                let target = variables.slot(&Variable::Anonymous);
                let binding = if let Term::Interval { lower, upper } = term {
                    LiteralIr::Range {
                        target,
                        lower: self.ranged_expression(lower, variables, body)?,
                        upper: self.ranged_expression(upper, variables, body)?,
                        binder: false,
                    }
                } else {
                    LiteralIr::Compare(
                        Expression {
                            nodes: vec![Operation::Variable(target)],
                        },
                        Relation::Eq,
                        self.ranged_expression(term, variables, body)?,
                    )
                };
                body.push(binding);
                terms.push(CoreTerm::Variable(target));
            }
        }
        let predicate = self.predicate(
            atom.name.as_str(),
            arguments.len(),
            crate::coherence::core_sign(atom.sign),
        )?;
        self.pattern_from_parts(predicate, &terms)
    }
}
