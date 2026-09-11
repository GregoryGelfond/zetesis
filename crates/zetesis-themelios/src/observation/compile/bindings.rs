//! Structural equalities read one finite value and capture its exact components.
//!
//! A complete-value slot keeps the chosen operand available to every original
//! comparison edge. Reconstructing it from captures would repeat evaluation and
//! lose the identity of a selected finite alternative. The slot and its captured
//! subvalues are data owned by this query, not additions to the logical carrier.

use super::{
    BTreeSet, Binder, Compiler, Condition, DefaultNegation, Error, Feature, Operand, Relation,
    Sign, Template, UnaryOp,
};

fn structural_children(term: &Template) -> Option<&[Template]> {
    match term {
        Template::Function(_, _, children) | Template::Tuple(children) => Some(children),
        Template::Unary(UnaryOp::Negate, argument) => match argument.as_ref() {
            Template::Function(_, _, children) => Some(children),
            _ => None,
        },
        _ => None,
    }
}

fn captures(term: &Template, slots: &mut BTreeSet<usize>) {
    if let Template::Variable(slot) = term {
        slots.insert(*slot);
    } else if let Some(children) = structural_children(term) {
        for child in children {
            captures(child, slots);
        }
    }
}

pub(super) fn operand(condition: &Condition, index: usize) -> &Template {
    let Condition::Compare(_, first, steps) = condition else {
        unreachable!("selected a comparison")
    };
    if index == 0 {
        first
    } else {
        &steps[index - 1].1
    }
}

pub(super) fn take_operand(condition: &mut Condition, index: usize, complete: usize) -> Template {
    let Condition::Compare(_, first, steps) = condition else {
        unreachable!("selected a comparison")
    };
    let term = if index == 0 {
        first
    } else {
        &mut steps[index - 1].1
    };
    std::mem::replace(term, Template::Variable(complete))
}

impl Compiler<'_> {
    fn structural_inputs(&self, term: &Template, captures: &BTreeSet<usize>) -> bool {
        if let Some(children) = structural_children(term) {
            children
                .iter()
                .all(|child| self.structural_inputs(child, captures))
        } else {
            self.ready_with(term, captures)
        }
    }

    fn structural_source(&self, pattern: &Template, value: &Template) -> bool {
        let mut provided = BTreeSet::new();
        captures(pattern, &mut provided);
        provided.iter().any(|slot| !self.safe.contains(slot))
            && self.ready_with(value, &BTreeSet::new())
            && self.structural_inputs(pattern, &provided)
    }

    fn structural_edge(&self, condition: &Condition) -> Option<(usize, usize)> {
        let Condition::Compare(DefaultNegation::None, _, steps) = condition else {
            return None;
        };
        for (index, (relation, _)) in steps.iter().enumerate() {
            if *relation != Relation::Eq {
                continue;
            }
            let left = operand(condition, index);
            let right = operand(condition, index + 1);
            if self.structural_source(left, right) {
                return Some((index, index + 1));
            }
            if self.structural_source(right, left) {
                return Some((index + 1, index));
            }
        }
        None
    }

    fn structural_pattern(&self, term: Template) -> Result<Operand, Error> {
        Ok(match term {
            Template::Variable(slot) => Operand::Variable(slot),
            Template::Value(symbol) => Operand::Value(
                crate::structural_value::from_symbol(&symbol)
                    .map_err(|_| self.unsupported(Feature::Comparison))?,
            ),
            Template::Function(sign, name, children) => Operand::Function(
                sign,
                name,
                children
                    .into_iter()
                    .map(|child| self.structural_pattern(child))
                    .collect::<Result<_, _>>()?,
            ),
            Template::Tuple(children) => Operand::Tuple(
                children
                    .into_iter()
                    .map(|child| self.structural_pattern(child))
                    .collect::<Result<_, _>>()?,
            ),
            Template::Unary(UnaryOp::Negate, argument)
                if matches!(argument.as_ref(), Template::Function(_, _, _)) =>
            {
                let Template::Function(sign, name, children) = *argument else {
                    unreachable!()
                };
                Operand::Function(
                    match sign {
                        Sign::Positive => Sign::Negative,
                        Sign::Negative => Sign::Positive,
                    },
                    name,
                    children
                        .into_iter()
                        .map(|child| self.structural_pattern(child))
                        .collect::<Result<_, _>>()?,
                )
            }
            expression => Operand::Expression(expression),
        })
    }

    pub(super) fn bind_structure(
        &mut self,
        conditions: &mut [Condition],
        binders: &mut Vec<Binder>,
    ) -> Result<bool, Error> {
        let Some((index, pattern, value)) =
            conditions
                .iter()
                .enumerate()
                .find_map(|(index, condition)| {
                    self.structural_edge(condition)
                        .map(|(pattern, value)| (index, pattern, value))
                })
        else {
            return Ok(false);
        };
        let complete = self.slot()?;
        let mut provided = BTreeSet::new();
        captures(operand(&conditions[index], pattern), &mut provided);
        // Both moved templates remain owned by the binding instruction. Charge
        // the two newly retained guard references before replacing either one.
        self.node(1)?;
        self.node(1)?;
        let pattern = take_operand(&mut conditions[index], pattern, complete);
        let value = take_operand(&mut conditions[index], value, complete);
        binders.push(Binder::Match {
            pattern: self.structural_pattern(pattern)?,
            value,
            complete,
        });
        self.safe.extend(provided);
        self.safe.insert(complete);
        Ok(true)
    }
}
