//! The preparation receipt of integer comparison forms.

use crate::formula_conditional_ir::{Consequent, ConsequentOperand};

use super::integer::WithoutForm;
use super::{Comparison, HeadIr, LiteralIr, ObjectiveIr, Projection, RuleIr};

/// How the comparison literals of a prepared program were compiled: with an
/// integer comparison form, which decides a row with machine integers when the
/// row's operand values are numbers, or without one, by reason.
///
/// Each comparison and argument check of the prepared program is counted once,
/// in rule bodies, head and aggregate conditions, conditional alternatives,
/// projections and objective conditions alike. A literal without a form counts
/// under the first failure the compiler met: the left operand's shape, the
/// right operand's shape, then the constants, left operand before right;
/// within an operand's shape, its node count before the first failing node.
/// Equalities the binding planner turned into binders, and written constraints
/// that constraint replacement dropped, were compiled and charged but are not
/// literals of the prepared program, so they are not counted. The counts
/// describe compilation; they establish no verdict, and every literal decides
/// the same truth value on every row with or without a form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ComparisonForms {
    /// Literals with a form.
    pub with_form: usize,
    /// Literals with an operand of more than sixteen nodes, the most an
    /// operand's form evaluates in its fixed array of integers.
    pub long_operand: usize,
    /// Literals with a variable slot beyond 32 bits, reachable only when the
    /// variables per template are configured above 2³².
    pub wide_slot: usize,
    /// Literals with an operand that constructs a function or tuple term.
    pub constructor: usize,
    /// Literals with an operand constant that is not a number.
    pub nonnumeric_constant: usize,
    /// Literals with an operation whose argument names itself or a later node.
    pub argument_position: usize,
}

impl ComparisonForms {
    /// Literals without a form, for every reason.
    #[must_use]
    pub fn without_form(&self) -> usize {
        self.long_operand
            + self.wide_slot
            + self.constructor
            + self.nonnumeric_constant
            + self.argument_position
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
                LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => {
                    self.comparison(comparison);
                }
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

    fn comparison(&mut self, comparison: &Comparison) {
        let count = match comparison.form() {
            Ok(_) => &mut self.with_form,
            Err(WithoutForm::LongOperand) => &mut self.long_operand,
            Err(WithoutForm::WideSlot) => &mut self.wide_slot,
            Err(WithoutForm::Constructor) => &mut self.constructor,
            Err(WithoutForm::NonnumericConstant) => &mut self.nonnumeric_constant,
            Err(WithoutForm::ArgumentPosition) => &mut self.argument_position,
        };
        *count += 1;
    }
}
