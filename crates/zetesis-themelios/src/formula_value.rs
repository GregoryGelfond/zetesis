//! Constructors publish a typed DAG node over existing canonical child IDs.
//!
//! Argument order and repeated occurrences determine expanded logical measures.
//! No flat child tree or copied text is retained by expression execution.

use themelios_base::span::Location;
use zetesis_core::catalog::{AssignmentSlice, Limits as TermLimits, TermKey};
use zetesis_core::{ConstructionError, ValueError};

use crate::expansion::Budget;
use crate::formula_support::{Computation, Counters};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaLimits};

pub(crate) struct Constructor {
    /// Canonical shape metadata; only child-operation positions live here.
    pub shape: crate::formula_support::components::Constructor,
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
            size_of::<Self>() as u128 + self.arguments.len() as u128 * size_of::<usize>() as u128,
            location,
        )?;
        Ok(Box::new(Self {
            shape: self.shape,
            arguments: self.arguments.clone(),
        }))
    }

    pub(super) fn evaluate(
        &self,
        values: AssignmentSlice<'_>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        // This is the canonical logical ingress policy: expanded nodes/depth
        // and encoded/rendered length plus ID scratch. Actual retained DAG and
        // workspace capacities use the independent shared SupportBytes ceiling.
        // It is not ValueLimits' former flat-owned-value allocation measure.
        self.construct(
            values,
            computation,
            TermLimits::default(),
            limits,
            counters,
            location,
        )
    }

    fn construct(
        &self,
        values: AssignmentSlice<'_>,
        computation: &mut Computation<'_, '_>,
        term_limits: TermLimits,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<TermKey, FormulaFailure> {
        let descriptor = computation.static_constructor(self.shape, limits, counters, location)?;
        computation
            .construct(
                descriptor,
                values,
                &self.arguments,
                term_limits,
                crate::formula_support::GroundingWork::new(limits, counters, location),
            )
            .map_err(|error| match error {
                FormulaFailure::AtomCatalog {
                    error: zetesis_core::catalog::Error::Value(error),
                    ..
                } => failure(error, location),
                error => error,
            })
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
    use crate::formula_support::testing::{Fixture, binding};
    use themelios_base::{
        source::SourceId,
        span::{ByteOffset, Span},
    };
    use zetesis_core::{Sign, Value, ValueLimits, ValueNode, ValueNodeRef, ValueResource};

    fn location() -> Location {
        Location {
            source: SourceId::new(0),
            span: Span::empty(ByteOffset::new(0)),
        }
    }
    fn pair(fixture: &mut Fixture) -> Constructor {
        Constructor {
            shape: fixture.constructor(
                ValueNodeRef::Function {
                    name: "f",
                    sign: Sign::Positive,
                    arity: 2,
                },
                location(),
            ),
            arguments: vec![0, 1],
        }
    }
    // Explicit export is only the assertion boundary; the execution under test
    // constructs a DAG node over the fixture's existing canonical child IDs.
    fn run(values: &[Value], term_limits: TermLimits) -> Result<Value, FormulaFailure> {
        let mut fixture = Fixture::default();
        let constructor = pair(&mut fixture);
        fixture.with(location(), |_, computation, counters| {
            let source: Vec<_> = values.iter().cloned().map(Some).collect();
            let fields = binding(&source, computation, counters, location());
            let key = constructor.construct(
                fields.slots(),
                computation,
                term_limits,
                &FormulaLimits::default(),
                counters,
                location(),
            )?;
            Ok(computation
                .read()
                .term(&key)
                .unwrap()
                .to_value(ValueLimits::default())
                .unwrap())
        })
    }
    #[test]
    fn constructor_children_keep_order() {
        for left in -3..=3 {
            for right in -3..=3 {
                let actual = run(
                    &[Value::Number(left), Value::Number(right)],
                    TermLimits::default(),
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
                assert_eq!(actual, expected);
            }
        }
    }
    #[test]
    fn node_limit_is_inclusive() {
        let values = [Value::Number(1), Value::Number(2)];
        assert!(
            run(
                &values,
                TermLimits {
                    max_nodes: 3,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        assert!(matches!(
            run(
                &values,
                TermLimits {
                    max_nodes: 2,
                    ..Default::default()
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
                &values,
                TermLimits {
                    max_depth: 2,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        assert!(
            run(
                &values,
                TermLimits {
                    max_depth: 1,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn canonical_ingress_bytes_are_preflighted() {
        let values = [Value::String("a\n\"\\".into()), Value::Number(-12)];
        let (mut low, mut high) = (0, 10_000);
        while low < high {
            let middle = low + (high - low) / 2;
            if run(
                &values,
                TermLimits {
                    max_bytes: middle,
                    ..Default::default()
                },
            )
            .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert_eq!(
            run(
                &values,
                TermLimits {
                    max_bytes: low,
                    ..Default::default()
                }
            )
            .unwrap(),
            run(&values, TermLimits::default()).unwrap()
        );
        assert!(
            run(
                &values,
                TermLimits {
                    max_bytes: low - 1,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn undeclared_inputs_do_not_affect_evaluation() {
        use crate::formula_ir::{Expression, Operation};
        let mut fixture = Fixture::default();
        let expression = Expression {
            nodes: vec![
                Operation::Variable(1),
                Operation::Constructor(Box::new(Constructor {
                    shape: fixture.constructor(
                        ValueNodeRef::Function {
                            name: "f",
                            sign: Sign::Positive,
                            arity: 1,
                        },
                        location(),
                    ),
                    arguments: vec![0],
                })),
            ],
        };
        assert_eq!(expression.inputs().collect::<Vec<_>>(), vec![1]);
        fixture.with(location(), |_, computation, counters| {
            for ignored in -4..=4 {
                for input in -4..=4 {
                    let fields = binding(
                        &[Some(Value::Number(ignored)), Some(Value::Number(input))],
                        computation,
                        counters,
                        location(),
                    );
                    let key = crate::formula_support::expression(
                        &expression,
                        &fields,
                        computation,
                        &FormulaLimits::default(),
                        counters,
                        location(),
                    )
                    .unwrap();
                    let result = computation.read().term(&key).unwrap();
                    assert_eq!(
                        result.child(0).unwrap().descriptor(),
                        ValueNodeRef::Number(input)
                    );
                }
            }
        });
    }
}
