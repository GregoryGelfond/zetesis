//! Anonymous cardinality keys denote one existential atom pattern. A tagged
//! structural key keeps wildcard positions distinct from every ordinary value.

use super::{Compiler, Error, Feature, Sign, Term, UnaryOp, Variable};
use crate::observation::{AtomKeyTemplate, KeyTemplate};

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
    // Preserve historical synthesized-key source charges without encoding any
    // control tag as an ordinary numeric value in the canonical vocabulary.
    fn key_tag(&mut self) -> Result<(), Error> {
        self.node(1)?;
        self.node(1)?;
        self.node(1)
    }
    pub(super) fn key_ready(&self, key: &KeyTemplate) -> bool {
        match key {
            KeyTemplate::Any => true,
            KeyTemplate::Value(value) => self.ready(value),
            KeyTemplate::Construct(_, children) | KeyTemplate::Pool(children) => {
                children.iter().all(|child| self.key_ready(child))
            }
        }
    }
    pub(super) fn generate_key(&mut self, key: AtomKeyTemplate) -> Result<usize, Error> {
        let slot = self.slot()?;
        self.generated.push((slot, super::Generated::AtomKey(key)));
        self.used.insert(slot);
        Ok(slot)
    }
    pub(super) fn anonymous_term(
        &mut self,
        term: &Term,
        depth: usize,
    ) -> Result<KeyTemplate, Error> {
        self.node(depth)?;
        match term {
            Term::Variable(Variable::Anonymous) => {
                self.key_tag()?;
                Ok(KeyTemplate::Any)
            }
            Term::Pool(items) => Ok(KeyTemplate::Pool(
                items
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            Term::Function { name, arguments } if arguments.iter().any(contains) => {
                self.text(name.as_str())?;
                let children = arguments
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.key_tag()?;
                Ok(KeyTemplate::Construct(
                    self.shape(Some(name.as_str()), Sign::Positive, children.len())?,
                    children,
                ))
            }
            Term::Tuple(items) if items.iter().any(contains) => {
                let children = items
                    .iter()
                    .map(|term| self.anonymous_term(term, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.key_tag()?;
                Ok(KeyTemplate::Construct(
                    self.shape(None, Sign::Positive, children.len())?,
                    children,
                ))
            }
            Term::UnaryOperation {
                operator: UnaryOp::Negate,
                argument,
            } if contains(argument) => {
                let KeyTemplate::Construct(shape, children) =
                    self.anonymous_term(argument, depth + 1)?
                else {
                    return Err(self.unsupported(Feature::Comparison));
                };
                Ok(KeyTemplate::Construct(self.negate(shape)?, children))
            }
            _ if contains(term) => Err(self.unsupported(Feature::UnsafeVariable)),
            _ => {
                let value = self.template(term, depth)?;
                self.key_tag()?;
                Ok(KeyTemplate::Value(value))
            }
        }
    }
}
