//! Positive structural patterns match support rows transactionally.
//!
//! Whole argument captures retain the original atom in the emitted formula.
//! Nested variables expose subvalues; anonymous nodes neither equate occurrences
//! nor introduce named slots. Plans and matching traverse flat preorder storage.

use themelios_base::span::Location;
use zetesis_core::catalog::TermRef;
use zetesis_core::{
    BindingView, ConstructionError, PatternRef, TemplateTerm, ValueError, ValueNodeRef,
};

use crate::expansion::Budget;
use crate::formula_support::{Computation, GroundingWork, components};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure};

pub(crate) struct PatternAtom {
    pub atom: components::Pattern,
    pub arguments: Vec<ArgumentPattern>,
}
pub(crate) struct ArgumentPattern {
    pub position: usize,
    pub nodes: Vec<PatternNode>,
}
pub(crate) enum PatternNode {
    Constructor(components::Constructor),
    Constant(components::Scalar),
    Slot(usize),
    Wildcard,
}

/// A retained plan paired with its source's resolved flat atom.
#[derive(Clone, Copy)]
pub(crate) struct Pattern<'a> {
    source: &'a PatternAtom,
    atom: PatternRef<'a>,
}
impl PatternAtom {
    pub(crate) fn bind<'a>(&'a self, atom: PatternRef<'a>) -> Pattern<'a> {
        Pattern { source: self, atom }
    }
}
impl<'a> Pattern<'a> {
    pub(crate) fn atom(self) -> PatternRef<'a> {
        self.atom
    }
    pub(crate) fn node_count(self) -> usize {
        self.atom.terms().len()
            + self
                .source
                .arguments
                .iter()
                .map(|argument| argument.nodes.len())
                .sum::<usize>()
    }
    pub(crate) fn slots(self) -> impl Iterator<Item = usize> + 'a {
        self.atom
            .terms()
            .iter()
            .filter_map(|term| match term {
                TemplateTerm::Variable(slot) => Some(slot),
                TemplateTerm::Constant(_) => None,
            })
            .chain(self.source.arguments.iter().flat_map(|argument| {
                argument.nodes.iter().filter_map(|node| {
                    if let PatternNode::Slot(slot) = node {
                        Some(*slot)
                    } else {
                        None
                    }
                })
            }))
    }

    /// No incoming slot changes unless the caller commits the complete delta.
    /// Captures borrow the support prefix; no whole value or subtree is copied.
    pub(crate) fn matches<'source>(
        self,
        atom: zetesis_core::relation::Row<'_, 'source>,
        values: BindingView<'_>,
        computation: &Computation<'_, '_>,
        context: &mut MatchContext<'_>,
    ) -> Result<Option<Vec<(usize, TermRef<'source>)>>, FormulaFailure> {
        context.work()?;
        if atom
            .predicate()
            .compare_ref_with(self.atom.predicate(), || context.work())?
            .is_ne()
        {
            return Ok(None);
        }
        context.charge(self.node_count() as u128)?;
        let slots = self.slots().count();
        let mut delta = Vec::new();
        reserve(&mut delta, slots, context.budget, context.work.location)?;
        for (column, term) in self.atom.terms().iter().enumerate() {
            let value = atom.value(column).expect("checked pattern arity");
            let agrees = match term {
                TemplateTerm::Constant(expected) => context.equal(value, expected)?,
                TemplateTerm::Variable(slot) => bind(slot, value, values, &mut delta, context)?,
            };
            if !agrees {
                return Ok(None);
            }
        }
        for argument in &self.source.arguments {
            let value = atom
                .value(argument.position)
                .expect("checked argument position");
            let mut offset = 0;
            for node in &argument.nodes {
                let Some(actual) = value.subterm_with(offset, || context.work())? else {
                    return Ok(None);
                };
                // Equal constructor arities preserve preorder alignment. A
                // capture consumes its complete subtree, retaining its identity.
                context.work()?;
                let descriptor = actual.descriptor();
                context.charge(descriptor_bytes(descriptor))?;
                if let PatternNode::Constructor(constructor) = node {
                    let expected = computation.static_constructor(
                        *constructor,
                        context.work.limits,
                        context.work.counters,
                        context.work.location,
                    )?;
                    if expected != descriptor {
                        return Ok(None);
                    }
                    offset += 1;
                    continue;
                }
                let agrees = match node {
                    PatternNode::Slot(slot) => bind(*slot, actual, values, &mut delta, context)?,
                    PatternNode::Constant(expected) => {
                        let expected = computation.static_scalar(
                            *expected,
                            context.work.limits,
                            context.work.counters,
                            context.work.location,
                        )?;
                        context.equal(actual, expected)?
                    }
                    PatternNode::Wildcard => true,
                    PatternNode::Constructor(_) => unreachable!("shape consumed above"),
                };
                if !agrees {
                    return Ok(None);
                }
                context.work()?;
                offset += actual.expanded_nodes();
            }
            debug_assert_eq!(offset, value.expanded_nodes());
        }
        Ok(Some(delta))
    }
}

pub(super) struct MatchContext<'a> {
    pub work: GroundingWork<'a>,
    pub budget: &'a mut Budget,
}
impl MatchContext<'_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.work
            .counters
            .work(self.work.limits, self.work.location)
    }
    /// Charge `amount` units at once, so a refusal states that requirement.
    fn charge(&mut self, amount: u128) -> Result<(), FormulaFailure> {
        self.work
            .counters
            .charge_work(amount, self.work.limits, self.work.location)
    }
    fn equal(&mut self, left: TermRef<'_>, right: TermRef<'_>) -> Result<bool, FormulaFailure> {
        left.compare_ref_with(right, || self.work())
            .map(std::cmp::Ordering::is_eq)
    }
}

fn bind<'source>(
    slot: usize,
    value: TermRef<'source>,
    values: BindingView<'_>,
    delta: &mut Vec<(usize, TermRef<'source>)>,
    context: &mut MatchContext<'_>,
) -> Result<bool, FormulaFailure> {
    context.work()?;
    if let Some(bound) = values.get(slot) {
        return context.equal(value, bound);
    }
    // Repeated names see staged bindings without changing the incoming frame.
    for (target, bound) in delta.iter() {
        context.work()?;
        if *target == slot {
            return context.equal(value, *bound);
        }
    }
    delta.push((slot, value));
    Ok(true)
}

fn descriptor_bytes(node: ValueNodeRef<'_>) -> u128 {
    match node {
        ValueNodeRef::String(text)
        | ValueNodeRef::Symbol(text)
        | ValueNodeRef::Function { name: text, .. } => text.len() as u128,
        _ => 0,
    }
}

/// Reserve selected owned cells before growing a buffer; allocator overhead is excluded.
pub(super) fn reserve<T>(
    buffer: &mut Vec<T>,
    additional: usize,
    budget: &mut Budget,
    location: Location,
) -> Result<(), FormulaFailure> {
    let required = buffer
        .len()
        .checked_add(additional)
        .ok_or_else(|| allocation(location))?;
    if required > buffer.capacity() {
        budget.charge(
            ExpansionResource::ScalarBytes,
            (required - buffer.capacity()) as u128 * std::mem::size_of::<T>() as u128,
            location,
        )?;
        buffer
            .try_reserve_exact(additional)
            .map_err(|_| allocation(location))?;
    }
    Ok(())
}
pub(super) fn push<T>(
    buffer: &mut Vec<T>,
    value: T,
    budget: &mut Budget,
    location: Location,
) -> Result<(), FormulaFailure> {
    if buffer.len() == buffer.capacity() {
        let additional = buffer.len().max(1);
        reserve(buffer, additional, budget, location)?;
    }
    buffer.push(value);
    Ok(())
}
fn allocation(location: Location) -> FormulaFailure {
    AdmissionFailure::Construction {
        error: ConstructionError::Value(ValueError::Allocation),
        location,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FormulaLimits;
    use crate::formula_support::Counters;
    use crate::formula_support::testing::Fixture;
    use crate::{ExpansionFailure, ExpansionLimits, FormulaResource};
    use themelios_base::span::{ByteOffset, Span};
    use zetesis_core::{Atom, Predicate, Sign, Value, ValueLimits, ValueNode};

    fn location() -> Location {
        Location {
            source: themelios_base::source::SourceId::new(0),
            span: Span::new(ByteOffset::new(0), ByteOffset::new(1)).unwrap(),
        }
    }
    fn fixture(root: ValueNodeRef<'_>) -> (Fixture, PatternAtom, Predicate) {
        let predicate = Predicate::new("q", 1).unwrap();
        let value = Value::from_nodes(
            vec![
                root.into_owned(),
                ValueNode::Tuple { arity: 1 },
                ValueNode::Symbol("a".into()),
                ValueNode::Number(2),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        let atom = Atom::new(predicate.clone(), vec![value]).unwrap();
        let mut fixture = Fixture::from_atoms([atom], location());
        let scalar = fixture.scalar(&Value::Number(2), location());
        let shape = fixture.constructor(root, location());
        let atom = fixture.admit(location(), |source, counters| {
            let limits = FormulaLimits::default();
            let predicate = source
                .predicate((&predicate).into(), &limits, counters, location())
                .unwrap();
            source
                .pattern(
                    predicate,
                    &[components::Term::Variable(0)],
                    &limits,
                    counters,
                    location(),
                )
                .unwrap()
        });
        (
            fixture,
            PatternAtom {
                atom,
                arguments: vec![ArgumentPattern {
                    position: 0,
                    nodes: vec![
                        PatternNode::Constructor(shape),
                        PatternNode::Slot(1),
                        PatternNode::Constant(scalar),
                    ],
                }],
            },
            predicate,
        )
    }

    #[test]
    fn a_refused_charge_states_the_whole_requirement() {
        // A batched pattern charge reports the whole requested amount, even
        // when only its first unit would exceed the ceiling.
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut counters = Counters::default();
        let limits = FormulaLimits {
            max_work: 1,
            ..FormulaLimits::default()
        };
        let mut context = MatchContext {
            work: GroundingWork::new(&limits, &mut counters, location()),
            budget: &mut budget,
        };
        context.work().unwrap();
        let error = context.charge(4).unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::Work,
                observed: 5,
                limit: 1,
                ..
            }
        ));
    }
    #[test]
    fn work_refusals_discard_the_entire_delta() {
        assert_work_boundary(ValueNodeRef::Tuple { arity: 2 });
    }
    #[test]
    fn function_matching_obeys_the_work_ceiling() {
        assert_work_boundary(ValueNodeRef::Function {
            name: "constructor_name",
            sign: Sign::Negative,
            arity: 2,
        });
    }
    fn assert_work_boundary(root: ValueNodeRef<'_>) {
        let (mut fixture, source, predicate) = fixture(root);
        fixture.with(location(), |support, computation, preparation| {
            let flat = computation
                .static_pattern(
                    source.atom,
                    &FormulaLimits::default(),
                    preparation,
                    location(),
                )
                .unwrap();
            let pattern = source.bind(flat);
            let atom = support.rows(&predicate).next().unwrap();
            let incoming: [Option<TermRef<'_>>; 2] = [None, None];
            let mut budget = Budget::new(ExpansionLimits::default(), 100);
            let mut counters = Counters::default();
            let delta = pattern
                .matches(
                    atom,
                    incoming.as_slice().into(),
                    computation,
                    &mut MatchContext {
                        work: GroundingWork::new(
                            &FormulaLimits::default(),
                            &mut counters,
                            location(),
                        ),
                        budget: &mut budget,
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(delta.len(), 2);
            let exact = counters.accounting.work;
            for limit in 0..exact {
                let mut budget = Budget::new(ExpansionLimits::default(), 100);
                let mut counters = Counters::default();
                let error = pattern
                    .matches(
                        atom,
                        incoming.as_slice().into(),
                        computation,
                        &mut MatchContext {
                            work: GroundingWork::new(
                                &FormulaLimits {
                                    max_work: limit,
                                    ..FormulaLimits::default()
                                },
                                &mut counters,
                                location(),
                            ),
                            budget: &mut budget,
                        },
                    )
                    .unwrap_err();
                assert!(matches!(
                    error,
                    FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    }
                ));
                assert_eq!(incoming, [None, None]);
            }
            let mut budget = Budget::new(ExpansionLimits::default(), 100);
            let mut counters = Counters::default();
            assert!(
                pattern
                    .matches(
                        atom,
                        incoming.as_slice().into(),
                        computation,
                        &mut MatchContext {
                            work: GroundingWork::new(
                                &FormulaLimits {
                                    max_work: exact,
                                    ..FormulaLimits::default()
                                },
                                &mut counters,
                                location()
                            ),
                            budget: &mut budget
                        }
                    )
                    .unwrap()
                    .is_some()
            );
        });
    }
    #[test]
    fn capture_storage_is_admitted_before_matching() {
        let (mut fixture, source, predicate) = fixture(ValueNodeRef::Tuple { arity: 2 });
        fixture.with(location(), |support, computation, preparation| {
            let flat = computation
                .static_pattern(
                    source.atom,
                    &FormulaLimits::default(),
                    preparation,
                    location(),
                )
                .unwrap();
            let incoming: [Option<TermRef<'_>>; 2] = [None, None];
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_scalar_bytes: 0,
                    ..ExpansionLimits::default()
                },
                100,
            );
            let mut counters = Counters::default();
            let error = source
                .bind(flat)
                .matches(
                    support.rows(&predicate).next().unwrap(),
                    incoming.as_slice().into(),
                    computation,
                    &mut MatchContext {
                        work: GroundingWork::new(
                            &FormulaLimits::default(),
                            &mut counters,
                            location(),
                        ),
                        budget: &mut budget,
                    },
                )
                .unwrap_err();
            assert!(
                matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit {
                resource: ExpansionResource::ScalarBytes, observed, ..
            }) if observed == 2 * std::mem::size_of::<(usize, TermRef<'_>)>() as u128)
            );
            assert_eq!(incoming, [None, None]);
        });
    }
    #[test]
    fn nested_captures_borrow_the_support_value() {
        let (mut fixture, source, predicate) = fixture(ValueNodeRef::Tuple { arity: 2 });
        fixture.with(location(), |support, computation, preparation| {
            let flat = computation
                .static_pattern(
                    source.atom,
                    &FormulaLimits::default(),
                    preparation,
                    location(),
                )
                .unwrap();
            let row = support.rows(&predicate).next().unwrap();
            let incoming: [Option<TermRef<'_>>; 2] = [None, None];
            let mut budget = Budget::new(ExpansionLimits::default(), 100);
            let mut counters = Counters::default();
            let delta = source
                .bind(flat)
                .matches(
                    row,
                    incoming.as_slice().into(),
                    computation,
                    &mut MatchContext {
                        work: GroundingWork::new(
                            &FormulaLimits::default(),
                            &mut counters,
                            location(),
                        ),
                        budget: &mut budget,
                    },
                )
                .unwrap()
                .unwrap();
            let nested = delta.iter().find(|(slot, _)| *slot == 1).unwrap().1;
            assert_eq!(nested.to_string(), "(a,)");
            let expected = row.value(0).unwrap().child(0).unwrap();
            let ValueNodeRef::Symbol(actual) = nested.child(0).unwrap().descriptor() else {
                panic!("symbol capture")
            };
            let ValueNodeRef::Symbol(source) = expected.child(0).unwrap().descriptor() else {
                panic!("source symbol")
            };
            assert!(std::ptr::eq(actual.as_ptr(), source.as_ptr()));
        });
    }
}
