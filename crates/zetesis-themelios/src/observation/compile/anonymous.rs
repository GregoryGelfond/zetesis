//! Anonymous cardinality keys denote one existential atom pattern. A tagged
//! structural key keeps wildcard positions distinct from every ordinary value.

use super::{Compiler, Error, Feature, Sign, Symbol, Template, Term, UnaryOp, Variable};

pub(super) fn contains(term: &Term) -> bool {
    match term {
        Term::Variable(Variable::Anonymous) => true,
        Term::Function { arguments, .. } | Term::Tuple(arguments) | Term::Pool(arguments) => {
            arguments.iter().any(contains)
        }
        Term::UnaryOperation { argument, .. } | Term::Absolute(argument) => contains(argument),
        Term::BinaryOperation { left, right, .. }
        | Term::Interval {
            lower: left,
            upper: right,
        } => contains(left) || contains(right),
        _ => false,
    }
}
impl Compiler<'_> {
    fn tagged(&mut self, tag: i32, value: Option<Template>) -> Result<Template, Error> {
        self.node(1)?;
        self.node(1)?;
        let mut items = vec![Template::Value(self.symbol(
            &Symbol::Number(tag),
            1,
            false,
        )?)];
        items.extend(value);
        Ok(Template::Tuple(items))
    }
    pub(super) fn anonymous_term(&mut self, term: &Term, depth: usize) -> Result<Template, Error> {
        self.node(depth)?;
        match term {
            Term::Variable(Variable::Anonymous) => self.tagged(0, None),
            Term::Pool(items) => {
                let items = items
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<_, _>>()?;
                Ok(Template::Pool(items))
            }
            Term::Function { name, arguments } if arguments.iter().any(contains) => {
                self.text(name.as_str())?;
                let arguments = arguments
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<_, _>>()?;
                self.tagged(
                    2,
                    Some(Template::Function(Sign::Positive, name.clone(), arguments)),
                )
            }
            Term::Tuple(items) if items.iter().any(contains) => {
                let items = items
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<_, _>>()?;
                self.tagged(3, Some(Template::Tuple(items)))
            }
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } if contains(argument) => {
                let mut encoded = self.anonymous_term(argument, depth + 1)?;
                let Template::Tuple(items) = &mut encoded else {
                    return Err(self.unsupported(Feature::Comparison));
                };
                let Some(Template::Function(sign, _, _)) = items.get_mut(1) else {
                    return Err(self.unsupported(Feature::Comparison));
                };
                *sign = match sign {
                    Sign::Positive => Sign::Negative,
                    Sign::Negative => Sign::Positive,
                };
                Ok(encoded)
            }
            _ if contains(term) => Err(self.unsupported(Feature::UnsafeVariable)),
            _ => {
                let value = self.template(term, depth)?;
                self.tagged(1, Some(value))
            }
        }
    }
}
