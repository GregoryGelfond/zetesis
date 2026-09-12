//! Deterministic construction beneath the existing flat expression plan.
//!
//! Child operations are already evaluated. Preflight the complete copied tree,
//! spelling and construction frames before allocating any output nodes. The
//! resulting value is data; it creates neither support nor a semantic atom.

use themelios_base::span::Location;
use zetesis_core::{
    ConstructionError, Sign, Value, ValueError, ValueLimits, ValueNode, ValueResource,
};

use crate::expansion::Budget;
use crate::formula_support::Counters;
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaLimits};

pub(crate) struct Constructor {
    /// None denotes a tuple; its identity differs from every named function.
    pub name: Option<String>,
    pub sign: Sign,
    pub arguments: Vec<usize>,
}

impl Constructor {
    pub(super) fn copy(
        &self,
        budget: &mut Budget,
        location: Location,
    ) -> Result<Box<Self>, FormulaFailure> {
        budget.charge(
            ExpansionResource::ScalarBytes,
            std::mem::size_of::<Self>() as u128
                + self.name.as_ref().map_or(0, |name| name.len() as u128)
                + self.arguments.len() as u128 * std::mem::size_of::<usize>() as u128,
            location,
        )?;
        Ok(Box::new(Self {
            name: self.name.clone(),
            sign: self.sign,
            arguments: self.arguments.clone(),
        }))
    }

    pub(super) fn evaluate(
        &self,
        values: &[Value],
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        let value_limits = ValueLimits::default();
        self.construct(values, value_limits, limits, budget, counters, location)
    }

    fn construct(
        &self,
        values: &[Value],
        value_limits: ValueLimits,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Value, FormulaFailure> {
        let layout = self.layout(values, value_limits, limits, counters, location)?;
        budget.charge(ExpansionResource::ScalarBytes, layout.bytes, location)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(layout.nodes)
            .map_err(|_| failure(ValueError::Allocation, location))?;
        check(
            ValueResource::Bytes,
            result.capacity() as u128 * std::mem::size_of::<ValueNode>() as u128
                + layout.auxiliary_bytes,
            value_limits.max_bytes,
            location,
        )?;
        result.push(match &self.name {
            Some(name) => ValueNode::Function {
                name: name.clone(),
                sign: self.sign,
                arity: self.arguments.len(),
            },
            None => ValueNode::Tuple {
                arity: self.arguments.len(),
            },
        });
        for &argument in &self.arguments {
            let value = &values[argument];
            if let Value::Structured(value) = value {
                for node in value.nodes() {
                    counters.work(limits, location)?;
                    result.push(node.clone());
                }
            } else {
                counters.work(limits, location)?;
                result.push(match value {
                    Value::Infimum => ValueNode::Infimum,
                    Value::Supremum => ValueNode::Supremum,
                    Value::Number(n) => ValueNode::Number(*n),
                    Value::String(text) => ValueNode::String(text.clone()),
                    Value::Symbol(name) => ValueNode::Symbol(name.clone()),
                    Value::Structured(_) => unreachable!("handled whole structure"),
                });
            }
        }
        Value::from_nodes(result, value_limits).map_err(|error| failure(error, location))
    }
    /// Exact logical payload plus conservative validation/render reservations.
    fn layout(
        &self,
        values: &[Value],
        value_limits: ValueLimits,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Layout, FormulaFailure> {
        let mut nodes = 1_u128;
        let mut depth = 1_u128;
        let mut text = self.name.as_ref().map_or(0, |name| name.len() as u128);
        let arity = self.arguments.len() as u128;
        let mut spelling = match &self.name {
            Some(name) => {
                name.len() as u128
                    + u128::from(self.sign == Sign::Negative)
                    + if arity == 0 { 0 } else { arity + 1 }
            }
            None => {
                if arity == 0 {
                    2
                } else {
                    arity + 1 + u128::from(arity == 1)
                }
            }
        };
        // Each finite child is traversed once for preflight and once for copying.
        // No partial tree escapes either a resource refusal or allocation failure.
        for &argument in &self.arguments {
            counters.work(limits, location)?;
            let value = &values[argument];
            if let Value::Structured(value) = value {
                nodes += value.nodes().len() as u128;
                depth = depth.max(value.depth() as u128 + 1);
                spelling += value.rendered_bytes() as u128;
                for node in value.nodes() {
                    counters.work(limits, location)?;
                    text += node_text(node) as u128;
                }
            } else {
                nodes += 1;
                depth = depth.max(2);
                text += value.payload_bytes() as u128;
                spelling += scalar_spelling(value);
            }
        }
        check(
            ValueResource::Nodes,
            nodes,
            value_limits.max_nodes,
            location,
        )?;
        check(
            ValueResource::Depth,
            depth,
            value_limits.max_depth,
            location,
        )?;
        let frames = nodes.min(value_limits.max_depth as u128)
            * (std::mem::size_of::<usize>() + std::mem::size_of::<(usize, bool, bool)>()) as u128;
        let bytes = nodes * std::mem::size_of::<ValueNode>() as u128 + text + spelling + frames;
        check(
            ValueResource::Bytes,
            bytes,
            value_limits.max_bytes,
            location,
        )?;
        Ok(Layout {
            nodes: usize::try_from(nodes).expect("node ceiling proves usize"),
            bytes,
            auxiliary_bytes: text + spelling + frames,
        })
    }
}

struct Layout {
    nodes: usize,
    bytes: u128,
    auxiliary_bytes: u128,
}

fn node_text(node: &ValueNode) -> usize {
    match node {
        ValueNode::String(text)
        | ValueNode::Symbol(text)
        | ValueNode::Function { name: text, .. } => text.len(),
        _ => 0,
    }
}
fn scalar_spelling(value: &Value) -> u128 {
    match value {
        Value::Number(n) => {
            u128::from(n.unsigned_abs().checked_ilog10().unwrap_or(0) + 1) + u128::from(*n < 0)
        }
        Value::String(text) => {
            2 + text.len() as u128
                + text
                    .bytes()
                    .filter(|c| matches!(c, b'"' | b'\\' | b'\n'))
                    .count() as u128
        }
        Value::Symbol(name) => name.len() as u128,
        Value::Infimum | Value::Supremum => 4,
        Value::Structured(value) => value.rendered_bytes() as u128,
    }
}
fn check(
    resource: ValueResource,
    observed: u128,
    limit: usize,
    location: Location,
) -> Result<(), FormulaFailure> {
    if observed > limit as u128 {
        Err(failure(
            ValueError::Limit {
                resource,
                observed,
                limit,
            },
            location,
        ))
    } else {
        Ok(())
    }
}
fn failure(error: ValueError, location: Location) -> FormulaFailure {
    AdmissionFailure::Construction {
        error: ConstructionError::Value(error),
        location,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExpansionLimits;
    use crate::formula_ir::{Expression, Operation};
    use crate::formula_support::expression;
    use themelios_base::source::SourceId;

    fn location() -> Location {
        Location {
            source: SourceId::new(0),
            span: themelios_base::span::Span::empty(themelios_base::span::ByteOffset::new(0)),
        }
    }
    fn run(
        constructor: &Constructor,
        values: &[Value],
        value_limits: ValueLimits,
    ) -> Result<Value, FormulaFailure> {
        constructor.construct(
            values,
            value_limits,
            &FormulaLimits::default(),
            &mut Budget::new(ExpansionLimits::default(), 100),
            &mut Counters::default(),
            location(),
        )
    }
    fn pair() -> Constructor {
        Constructor {
            name: Some("f".into()),
            sign: Sign::Positive,
            arguments: vec![0, 1],
        }
    }
    #[test]
    fn constructor_children_keep_order() {
        for left in -3..=3 {
            for right in -3..=3 {
                let result = run(
                    &pair(),
                    &[Value::Number(left), Value::Number(right)],
                    ValueLimits::default(),
                )
                .unwrap();
                let expected = Value::from_nodes(
                    vec![
                        ValueNode::Function {
                            name: "f".into(),
                            sign: Sign::Positive,
                            arity: 2,
                        },
                        ValueNode::Number(left),
                        ValueNode::Number(right),
                    ],
                    ValueLimits::default(),
                )
                .unwrap();
                assert_eq!(result, expected);
            }
        }
    }
    #[test]
    fn node_limit_is_inclusive() {
        let values = [Value::Number(1), Value::Number(2)];
        assert!(
            run(
                &pair(),
                &values,
                ValueLimits {
                    max_nodes: 3,
                    ..ValueLimits::default()
                }
            )
            .is_ok()
        );
        assert!(matches!(
            run(
                &pair(),
                &values,
                ValueLimits {
                    max_nodes: 2,
                    ..ValueLimits::default()
                }
            ),
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Admission(AdmissionFailure::Construction {
                    error: ConstructionError::Value(ValueError::Limit {
                        resource: ValueResource::Nodes,
                        observed: 3,
                        limit: 2
                    }),
                    ..
                })
            ))
        ));
    }
    #[test]
    fn depth_limit_is_inclusive() {
        let values = [Value::Number(1), Value::Number(2)];
        assert!(
            run(
                &pair(),
                &values,
                ValueLimits {
                    max_depth: 2,
                    ..ValueLimits::default()
                }
            )
            .is_ok()
        );
        assert!(
            run(
                &pair(),
                &values,
                ValueLimits {
                    max_depth: 1,
                    ..ValueLimits::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn construction_bytes_are_preflighted() {
        let values = [Value::String("a\n\"\\".into()), Value::Number(-12)];
        let mut low = 0;
        let mut high = 10_000;
        while low < high {
            let middle = low + (high - low) / 2;
            if run(
                &pair(),
                &values,
                ValueLimits {
                    max_bytes: middle,
                    ..ValueLimits::default()
                },
            )
            .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let exact = run(
            &pair(),
            &values,
            ValueLimits {
                max_bytes: low,
                ..ValueLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            exact,
            run(&pair(), &values, ValueLimits::default()).unwrap()
        );
        assert!(
            run(
                &pair(),
                &values,
                ValueLimits {
                    max_bytes: low - 1,
                    ..ValueLimits::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn undeclared_inputs_do_not_affect_evaluation() {
        let expression_plan = Expression {
            nodes: vec![
                Operation::Variable(1),
                Operation::Constructor(Box::new(Constructor {
                    name: Some("f".into()),
                    sign: Sign::Positive,
                    arguments: vec![0],
                })),
            ],
        };
        assert_eq!(expression_plan.inputs().collect::<Vec<_>>(), vec![1]);
        for ignored in -4..=4 {
            for input in -4..=4 {
                let result = expression(
                    &expression_plan,
                    &crate::formula_binding::complete([
                        Value::Number(ignored),
                        Value::Number(input),
                    ]),
                    &FormulaLimits::default(),
                    &mut Budget::new(ExpansionLimits::default(), 100),
                    &mut Counters::default(),
                    location(),
                )
                .unwrap();
                let expected = run(
                    &Constructor {
                        name: Some("f".into()),
                        sign: Sign::Positive,
                        arguments: vec![0],
                    },
                    &[Value::Number(input)],
                    ValueLimits::default(),
                )
                .unwrap();
                assert_eq!(result, expected);
            }
        }
    }
}
