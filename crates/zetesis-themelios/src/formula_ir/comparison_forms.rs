//! The preparation receipt of integer comparison forms.

use crate::formula_conditional_ir::{Consequent, ConsequentOperand};

use super::integer::WithoutForm;
use super::{Expression, HeadIr, LiteralIr, ObjectiveIr, Projection, RuleIr};

/// How the comparison literals of a prepared program were compiled: with an
/// integer comparison form, which decides a row with machine integers when the
/// row's operand values are numbers, or without one, by reason.
///
/// Each comparison and argument check of the prepared program is counted once,
/// in rule bodies, head and aggregate conditions, conditional alternatives,
/// projections and objective conditions alike. A literal has a form when both
/// its operands do; one without counts under its left operand's reason, or
/// its right operand's when the left has a form. The counts describe
/// compilation; they establish no verdict, and every literal decides the same
/// truth value on every row with or without a form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ComparisonForms {
    /// Literals both of whose operands have a form.
    pub with_form: usize,
    /// Literals with an operand of more than sixteen nodes, or with a variable
    /// slot beyond the form's index width.
    pub bound: usize,
    /// Literals with an operand that constructs a function or tuple term.
    pub constructor: usize,
    /// Literals with an operand constant that is not a number.
    pub nonnumeric_constant: usize,
    /// Literals with an operation whose argument names itself or a later node.
    pub argument_position: usize,
    /// Literals with an operand the compiler built without compiling a form.
    pub built_without_form: usize,
}

impl ComparisonForms {
    /// Literals without a form, for every reason.
    #[must_use]
    pub fn without_form(&self) -> usize {
        self.bound
            + self.constructor
            + self.nonnumeric_constant
            + self.argument_position
            + self.built_without_form
    }

    /// Count every comparison literal of a prepared program's rules,
    /// projection rules and objectives.
    ///
    /// The count reads each literal once, a pass linear in the intermediate
    /// representation whose compilation preparation charged; it allocates
    /// nothing and charges nothing itself. Its recursion follows the
    /// representation's nesting, whose depth the admitted source's syntax
    /// depth bounds.
    pub(crate) fn of(rules: &[RuleIr], projection: &[RuleIr], objectives: &[ObjectiveIr]) -> Self {
        let mut forms = Self::default();
        for rule in rules.iter().chain(projection) {
            forms.literals(&rule.body);
            match &rule.head {
                HeadIr::Choice(group) => {
                    for element in &group.elements {
                        forms.literals(&element.condition);
                    }
                }
                HeadIr::ConditionalDisjunction { elements, .. } => {
                    for element in elements {
                        forms.literals(&element.condition);
                    }
                }
                HeadIr::Normal(_) | HeadIr::Disjunction(_) => {}
            }
        }
        for objective in objectives {
            forms.literals(objective.condition.literals());
        }
        forms
    }

    fn literals(&mut self, literals: &[LiteralIr]) {
        for literal in literals {
            match literal {
                LiteralIr::Compare(left, _, right) => self.comparison(left, right),
                LiteralIr::ArgumentCheck { captured, value } => self.comparison(captured, value),
                LiteralIr::ProjectedAtom(_, projection) => self.projection(projection),
                LiteralIr::Conditional(conditional) => {
                    self.literals(&conditional.condition);
                    match &conditional.consequent {
                        Consequent::Atoms(_, alternatives) => {
                            for alternative in alternatives {
                                self.literals(&alternative.bindings);
                                if let ConsequentOperand::Projection(projection) =
                                    &alternative.operand
                                {
                                    self.projection(projection);
                                }
                            }
                        }
                        Consequent::Guards(alternatives) => {
                            for alternative in alternatives {
                                self.literals(&alternative.bindings);
                            }
                        }
                        Consequent::Guard(_) => {}
                    }
                }
                LiteralIr::Aggregate(aggregate) => {
                    for element in &aggregate.elements {
                        self.literals(&element.condition);
                    }
                }
                LiteralIr::Atom(..)
                | LiteralIr::PatternAtom(_)
                | LiteralIr::TupleCompare(..)
                | LiteralIr::Guard(_)
                | LiteralIr::HeadGuard(_)
                | LiteralIr::Bind { .. }
                | LiteralIr::Range { .. } => {}
            }
        }
    }

    fn projection(&mut self, projection: &Projection) {
        if let Projection::Witnesses { bindings, .. } = projection {
            self.literals(bindings);
        }
    }

    fn comparison(&mut self, left: &Expression, right: &Expression) {
        let count = match left.integer().and(right.integer()) {
            Ok(_) => &mut self.with_form,
            Err(WithoutForm::Bound) => &mut self.bound,
            Err(WithoutForm::Constructor) => &mut self.constructor,
            Err(WithoutForm::NonnumericConstant) => &mut self.nonnumeric_constant,
            Err(WithoutForm::ArgumentPosition) => &mut self.argument_position,
            Err(WithoutForm::NotCompiled) => &mut self.built_without_form,
        };
        *count += 1;
    }
}
