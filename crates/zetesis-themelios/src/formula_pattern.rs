//! Positive tuple patterns match support rows transactionally.
//!
//! Whole argument captures retain the original atom in the emitted formula.
//! Nested variables expose subvalues; anonymous nodes neither equate occurrences
//! nor introduce named slots. Plans and matching traverse flat preorder storage.

use themelios_base::span::Location;
use zetesis_core::{
    Atom, AtomPattern, ConstructionError, Term, Value, ValueError, ValueLimits, ValueNode,
};

use crate::expansion::Budget;
use crate::formula_support::{Counters, copy};
use crate::{AdmissionFailure, ExpansionResource, FormulaFailure, FormulaLimits};

pub(crate) struct PatternAtom {
    pub atom: AtomPattern,
    pub arguments: Vec<ArgumentPattern>,
}
pub(crate) struct ArgumentPattern {
    pub position: usize,
    pub nodes: Vec<PatternNode>,
}
pub(crate) enum PatternNode {
    Tuple(usize),
    Constant(Value),
    Slot(usize),
    Wildcard,
}
impl PatternAtom {
    pub(super) fn node_count(&self) -> usize {
        self.atom.terms().len()
            + self
                .arguments
                .iter()
                .map(|argument| argument.nodes.len())
                .sum::<usize>()
    }
    pub(super) fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.atom
            .terms()
            .iter()
            .filter_map(|term| match term {
                Term::Variable(slot) => Some(*slot),
                Term::Constant(_) => None,
            })
            .chain(self.arguments.iter().flat_map(|argument| {
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
    pub(super) fn matches(
        &self,
        atom: &Atom,
        values: &[Option<Value>],
        context: &mut MatchContext<'_>,
    ) -> Result<Option<Vec<(usize, Value)>>, FormulaFailure> {
        context.work()?;
        if self.atom.predicate() != atom.predicate() {
            return Ok(None);
        }
        for _ in 0..self.node_count() {
            context.work()?;
        }
        let slots = self.slots().count();
        let mut delta = Vec::new();
        reserve(&mut delta, slots, context.budget, context.location)?;
        for (term, value) in self.atom.terms().iter().zip(atom.values()) {
            context.value_work(value)?;
            let agrees = match term {
                Term::Constant(expected) => expected == value,
                Term::Variable(slot) => {
                    bind(*slot, Borrowed::Whole(value), values, &mut delta, context)?
                }
            };
            if !agrees {
                return Ok(None);
            }
        }
        for argument in &self.arguments {
            let Value::Structured(value) = &atom.values()[argument.position] else {
                return Ok(None);
            };
            let mut offset = 0;
            for node in &argument.nodes {
                context.work()?;
                let Some(actual) = value.nodes().get(offset) else {
                    return Ok(None);
                };
                if let PatternNode::Tuple(arity) = node {
                    if !matches!(actual, ValueNode::Tuple { arity: found } if found == arity) {
                        return Ok(None);
                    }
                    offset += 1;
                    continue;
                }
                let end = subtree_end(value.nodes(), offset, context)?;
                let subtree = Borrowed::Nodes(&value.nodes()[offset..end]);
                let agrees = match node {
                    PatternNode::Slot(slot) => bind(*slot, subtree, values, &mut delta, context)?,
                    PatternNode::Constant(expected) => subtree.equals(expected),
                    PatternNode::Wildcard => true,
                    PatternNode::Tuple(_) => unreachable!("tuple consumed above"),
                };
                if !agrees {
                    return Ok(None);
                }
                offset = end;
            }
            debug_assert_eq!(offset, value.nodes().len());
        }
        Ok(Some(delta))
    }
}

pub(super) struct MatchContext<'a> {
    pub limits: FormulaLimits,
    pub budget: &'a mut Budget,
    pub counters: &'a mut Counters,
    pub location: Location,
}
impl MatchContext<'_> {
    fn work(&mut self) -> Result<(), FormulaFailure> {
        self.counters.work(self.limits, self.location)
    }
    fn value_work(&mut self, value: &Value) -> Result<(), FormulaFailure> {
        self.work()?;
        for _ in 0..crate::formula_ir::value_bytes(value) {
            self.work()?;
        }
        Ok(())
    }
}

fn bind(
    slot: usize,
    value: Borrowed<'_>,
    values: &[Option<Value>],
    delta: &mut Vec<(usize, Value)>,
    context: &mut MatchContext<'_>,
) -> Result<bool, FormulaFailure> {
    if let Some(bound) = &values[slot] {
        return Ok(value.equals(bound));
    }
    // The delta is deliberately private until every argument agrees. Repeated
    // names in this row therefore see staged bindings without changing the join.
    for (target, bound) in delta.iter() {
        context.work()?;
        if *target == slot {
            return Ok(value.equals(bound));
        }
    }
    let owned = match value {
        Borrowed::Whole(value) => copy(value, context.budget, context.location)?,
        Borrowed::Nodes(nodes) => extract(nodes, context)?,
    };
    delta.push((slot, owned));
    Ok(true)
}

#[derive(Clone, Copy)]
enum Borrowed<'a> {
    Whole(&'a Value),
    Nodes(&'a [ValueNode]),
}
impl Borrowed<'_> {
    fn equals(self, value: &Value) -> bool {
        match self {
            Self::Whole(actual) => actual == value,
            Self::Nodes(nodes) => match (nodes, value) {
                (_, Value::Structured(value)) => nodes == value.nodes(),
                ([ValueNode::Infimum], Value::Infimum)
                | ([ValueNode::Supremum], Value::Supremum) => true,
                ([ValueNode::Number(a)], Value::Number(b)) => a == b,
                ([ValueNode::String(a)], Value::String(b))
                | ([ValueNode::Symbol(a)], Value::Symbol(b)) => a == b,
                _ => false,
            },
        }
    }
}

fn subtree_end(
    nodes: &[ValueNode],
    start: usize,
    context: &mut MatchContext<'_>,
) -> Result<usize, FormulaFailure> {
    let mut remaining = 1;
    let mut end = start;
    while remaining != 0 {
        context.work()?;
        let node = &nodes[end];
        for _ in 0..text_bytes(node) {
            context.work()?;
        }
        remaining -= 1;
        remaining += match node {
            ValueNode::Function { arity, .. } | ValueNode::Tuple { arity } => *arity,
            _ => 0,
        };
        end += 1;
    }
    Ok(end)
}
fn text_bytes(node: &ValueNode) -> usize {
    match node {
        ValueNode::String(text)
        | ValueNode::Symbol(text)
        | ValueNode::Function { name: text, .. } => text.len(),
        _ => 0,
    }
}
fn extract(nodes: &[ValueNode], context: &mut MatchContext<'_>) -> Result<Value, FormulaFailure> {
    // Selected construction payload: node cells, cloned text, canonical spelling
    // and validation/render frames. The 16-byte spelling allowance is amortized
    // over the tree: total child arities equal node count minus one. Twice the
    // text covers the worst spelling escape expansion.
    // Source nodes and allocator overhead are not newly owned payload here.
    const SPELLING_BYTES_PER_NODE_ALLOWANCE: usize = 16;
    const TEXT_WITH_SPELLING_MULTIPLIER: u128 = 3;
    let cell = std::mem::size_of::<ValueNode>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<(usize, bool, bool)>()
        + SPELLING_BYTES_PER_NODE_ALLOWANCE;
    let bytes = nodes.len() as u128 * cell as u128
        + TEXT_WITH_SPELLING_MULTIPLIER
            * nodes
                .iter()
                .map(|node| text_bytes(node) as u128)
                .sum::<u128>();
    context
        .budget
        .charge(ExpansionResource::ScalarBytes, bytes, context.location)?;
    context.budget.charge(
        ExpansionResource::Values,
        nodes.len() as u128,
        context.location,
    )?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(nodes.len())
        .map_err(|_| allocation(context.location))?;
    owned.extend_from_slice(nodes);
    Value::from_nodes(owned, ValueLimits::default()).map_err(|error| {
        AdmissionFailure::Construction {
            error: ConstructionError::Value(error),
            location: context.location,
        }
        .into()
    })
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
    use crate::{ExpansionFailure, ExpansionLimits, FormulaResource};
    use themelios_base::span::{ByteOffset, Span};
    use zetesis_core::Predicate;

    fn location() -> Location {
        Location {
            source: themelios_base::source::SourceId::new(0),
            span: Span::new(ByteOffset::new(0), ByteOffset::new(1)).unwrap(),
        }
    }
    fn fixture() -> (PatternAtom, Atom, Vec<Option<Value>>) {
        let predicate = Predicate::new("q", 1).unwrap();
        let pattern = PatternAtom {
            atom: AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap(),
            arguments: vec![ArgumentPattern {
                position: 0,
                nodes: vec![
                    PatternNode::Tuple(2),
                    PatternNode::Slot(1),
                    PatternNode::Constant(Value::Number(2)),
                ],
            }],
        };
        let value = Value::from_nodes(
            vec![
                ValueNode::Tuple { arity: 2 },
                ValueNode::Tuple { arity: 1 },
                ValueNode::Symbol("a".into()),
                ValueNode::Number(2),
            ],
            ValueLimits::default(),
        )
        .unwrap();
        (
            pattern,
            Atom::new(predicate, vec![value]).unwrap(),
            vec![None, None],
        )
    }
    #[test]
    fn work_refusals_discard_the_entire_delta() {
        let (pattern, atom, incoming) = fixture();
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut counters = Counters::default();
        let delta = pattern
            .matches(
                &atom,
                &incoming,
                &mut MatchContext {
                    limits: FormulaLimits::default(),
                    budget: &mut budget,
                    counters: &mut counters,
                    location: location(),
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(delta.len(), 2);
        let exact = counters.work;
        for limit in 0..exact {
            let mut budget = Budget::new(ExpansionLimits::default(), 100);
            let mut counters = Counters::default();
            let error = pattern
                .matches(
                    &atom,
                    &incoming,
                    &mut MatchContext {
                        limits: FormulaLimits {
                            max_work: limit,
                            ..FormulaLimits::default()
                        },
                        budget: &mut budget,
                        counters: &mut counters,
                        location: location(),
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
            assert_eq!(incoming, vec![None, None]);
        }
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut counters = Counters::default();
        assert!(
            pattern
                .matches(
                    &atom,
                    &incoming,
                    &mut MatchContext {
                        limits: FormulaLimits {
                            max_work: exact,
                            ..FormulaLimits::default()
                        },
                        budget: &mut budget,
                        counters: &mut counters,
                        location: location()
                    }
                )
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn delta_storage_is_admitted_before_a_value_is_copied() {
        let (pattern, atom, incoming) = fixture();
        let mut budget = Budget::new(
            ExpansionLimits {
                max_scalar_bytes: 0,
                ..ExpansionLimits::default()
            },
            100,
        );
        let mut counters = Counters::default();
        let error = pattern
            .matches(
                &atom,
                &incoming,
                &mut MatchContext {
                    limits: FormulaLimits::default(),
                    budget: &mut budget,
                    counters: &mut counters,
                    location: location(),
                },
            )
            .unwrap_err();
        assert!(
            matches!(error,FormulaFailure::Expansion(ExpansionFailure::Limit {resource:ExpansionResource::ScalarBytes,observed,..}) if observed==2*std::mem::size_of::<(usize,Value)>() as u128)
        );
        assert_eq!(incoming, vec![None, None]);
    }
    #[test]
    fn extraction_obeys_its_inclusive_construction_payload_ceiling() {
        let nodes = [ValueNode::Tuple { arity: 1 }, ValueNode::Symbol("a".into())];
        let exact = 2
            * (std::mem::size_of::<ValueNode>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<(usize, bool, bool)>()
                + 16)
            + 3;
        for limit in [exact - 1, exact] {
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_scalar_bytes: limit,
                    ..ExpansionLimits::default()
                },
                100,
            );
            let mut counters = Counters::default();
            let result = extract(
                &nodes,
                &mut MatchContext {
                    limits: FormulaLimits::default(),
                    budget: &mut budget,
                    counters: &mut counters,
                    location: location(),
                },
            );
            if limit == exact {
                let Value::Structured(value) = result.unwrap() else {
                    panic!("tuple remains structural")
                };
                assert_eq!(value.to_string(), "(a,)");
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                        resource: ExpansionResource::ScalarBytes,
                        ..
                    }))
                ));
            }
        }
    }
}
