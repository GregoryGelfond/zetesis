//! Bounded structural alternatives over the already admitted template owner.

use super::{Compiler, Error, Operand, Resource, Template};

fn width(term: &Template) -> u128 {
    match term {
        Template::Pool(children) => children
            .iter()
            .fold(0_u128, |sum, term| sum.saturating_add(width(term))),
        Template::Function(_, _, children) | Template::Tuple(children) => children
            .iter()
            .fold(1_u128, |n, term| n.saturating_mul(width(term))),
        Template::Unary(_, inner) | Template::Absolute(inner) => width(inner),
        Template::Binary(_, left, right) | Template::Interval(left, right) => {
            width(left).saturating_mul(width(right))
        }
        _ => 1,
    }
}
impl Compiler<'_> {
    fn selected_template(
        &mut self,
        term: &Template,
        mut position: u128,
        depth: usize,
    ) -> Result<Template, Error> {
        self.node(depth)?;
        Ok(match term {
            Template::Pool(children) => {
                let selected = children
                    .iter()
                    .find(|term| {
                        let n = width(term);
                        if position < n {
                            true
                        } else {
                            position -= n;
                            false
                        }
                    })
                    .expect("position admitted against the structural alternative count");
                return self.selected_template(selected, position, depth + 1);
            }
            Template::Function(sign, name, children) => {
                self.text(name.as_str())?;
                Template::Function(
                    *sign,
                    name.clone(),
                    self.selected_children(children, position, depth + 1)?,
                )
            }
            Template::Tuple(children) => {
                Template::Tuple(self.selected_children(children, position, depth + 1)?)
            }
            Template::Unary(operator, inner) => Template::Unary(
                *operator,
                Box::new(self.selected_template(inner, position, depth + 1)?),
            ),
            Template::Value(symbol) => Template::Value(self.symbol(symbol, depth, false)?),
            Template::Variable(slot) => Template::Variable(*slot),
            Template::Binary(operator, left, right) => Template::Binary(
                *operator,
                Box::new(self.selected_template(left, position / width(right), depth + 1)?),
                Box::new(self.selected_template(right, position % width(right), depth + 1)?),
            ),
            Template::Absolute(inner) => Template::Absolute(Box::new(self.selected_template(
                inner,
                position,
                depth + 1,
            )?)),
            Template::Interval(left, right) => Template::Interval(
                Box::new(self.selected_template(left, position / width(right), depth + 1)?),
                Box::new(self.selected_template(right, position % width(right), depth + 1)?),
            ),
        })
    }
    fn selected_children(
        &mut self,
        children: &[Template],
        mut position: u128,
        depth: usize,
    ) -> Result<Vec<Template>, Error> {
        let mut result = Vec::new();
        for child in children.iter().rev() {
            let n = width(child);
            result.push(self.selected_template(child, position % n, depth)?);
            position /= n;
        }
        result.reverse();
        Ok(result)
    }
    pub(super) fn structural_alternatives(
        &mut self,
        term: Template,
    ) -> Result<Vec<Operand>, Error> {
        let count = width(&term);
        if count == 1 && !term.multiple() {
            return Ok(vec![self.structural_pattern(term)?]);
        }
        self.check(
            Resource::Nodes,
            self.nodes
                .saturating_add(usize::try_from(count).unwrap_or(usize::MAX)),
            self.limits.max_nodes as usize,
        )?;
        let mut result = Vec::new();
        for position in 0..count {
            let selected = self.selected_template(&term, position, 1)?;
            result.push(self.structural_pattern(selected)?);
        }
        Ok(result)
    }
}
