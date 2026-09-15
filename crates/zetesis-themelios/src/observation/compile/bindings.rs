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
        Template::Function(_, _, children)
        | Template::Tuple(children)
        | Template::Pool(children) => Some(children),
        Template::Unary(UnaryOp::Negate, argument) => match argument.as_ref() {
            Template::Function(_, _, children) => Some(children),
            _ => structural_children(argument),
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

fn operand(condition: &Condition, index: usize) -> &Template {
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
    fn inverse_captures(&self, term: &Template, provided: &mut BTreeSet<usize>) {
        if let Some(children) = structural_children(term) {
            for child in children {
                self.inverse_captures(child, provided);
            }
        } else if let Some(slot) = self.inverse_slot(term, provided) {
            provided.insert(slot);
        }
    }
    pub(super) fn capture_slots(&self, term: &Template) -> BTreeSet<usize> {
        let mut provided = BTreeSet::new();
        captures(term, &mut provided);
        self.inverse_captures(term, &mut provided);
        provided
    }

    fn structural_inputs(&self, term: &Template, captures: &BTreeSet<usize>) -> bool {
        if let Some(children) = structural_children(term) {
            children
                .iter()
                .all(|child| self.structural_inputs(child, captures))
        } else {
            self.ready_with(term, captures)
        }
    }

    pub(super) fn structural_capture(&self, pattern: &Template) -> bool {
        let provided = self.capture_slots(pattern);
        provided.iter().any(|slot| !self.safe.contains(slot))
            && self.structural_inputs(pattern, &provided)
    }

    fn structural_source(&self, pattern: &Template, value: &Template) -> bool {
        self.structural_capture(pattern) && self.ready_with(value, &BTreeSet::new())
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

    pub(super) fn structural_pattern(&self, term: Template) -> Result<Operand, Error> {
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
                if structural_children(&argument).is_some() =>
            {
                let Operand::Function(sign, name, children) = self.structural_pattern(*argument)?
                else {
                    return Err(self.unsupported(Feature::Comparison));
                };
                Operand::Function(
                    match sign {
                        Sign::Positive => Sign::Negative,
                        Sign::Negative => Sign::Positive,
                    },
                    name,
                    children,
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

        // Both moved templates remain owned by the binding instruction. Charge
        // the two newly retained guard references before replacing either one.
        self.node(1)?;
        self.node(1)?;
        let pattern = take_operand(&mut conditions[index], pattern, complete);
        let value = take_operand(&mut conditions[index], value, complete);
        let mut patterns = self.structural_alternatives(pattern)?;
        for pattern in &mut patterns {
            self.prepare_inverses(std::slice::from_mut(pattern));
        }
        let mut provided = None;
        for pattern in &patterns {
            let mut slots = BTreeSet::new();
            super::patterns::captures(pattern, &mut slots);
            provided = Some(provided.map_or(slots.clone(), |previous: BTreeSet<usize>| {
                previous.intersection(&slots).copied().collect()
            }));
        }
        binders.push(Binder::Match {
            patterns,
            value,
            complete,
        });
        self.safe.extend(provided.unwrap_or_default());
        self.safe.insert(complete);
        Ok(true)
    }
}
