//! Comparison operands through deterministic scalar assignments.
//!
//! This is a physical view only. Its caller separately proves every original
//! body expression total over one immutable support snapshot before using it
//! for selection. Original binders, head stages and family diagnostics remain.
//! Each imported source node is copied at most once per operand. Explicit DFS
//! frames and source-node memo cells bound work and space by the source DAG,
//! including repeated assignment references; cycles and constructors decline.

use themelios_program::program::Relation;

use crate::FormulaFailure;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{Buffer, Computation, Context, StorageLease, reserve};

pub(crate) struct View {
    comparisons: Buffer<Option<Entry>>,
}

struct Entry {
    left: Expanded,
    right: Expanded,
    relation: Relation,
}

struct Expanded {
    expression: Expression,
    lease: StorageLease,
}

impl Expanded {
    fn new(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<Self, FormulaFailure> {
        let mut lease = context.computation.lease();
        lease.observe(size_of::<Self>(), context.work.location)?;
        context.computation.storage_observed(
            &lease,
            0,
            size_of::<Self>(),
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        Ok(Self {
            expression: Expression::new(Vec::new()),
            lease,
        })
    }

    fn push(
        &mut self,
        operation: Operation,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<usize, FormulaFailure> {
        reserve(
            &mut self.expression.nodes,
            1,
            &mut self.lease,
            size_of::<Self>(),
            Context::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
        )?;
        tick(context)?;
        let result = self.expression.nodes.len();
        self.expression.nodes.push(operation);
        Ok(result)
    }

    fn embedded(
        mut self,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        self.lease.observe(
            self.expression.nodes.capacity() * size_of::<Operation>(),
            context.work.location,
        )?;
        Ok(self)
    }
}

#[derive(Clone, Copy)]
struct Source<'a> {
    expression: &'a Expression,
    offset: usize,
}

#[derive(Clone, Copy, Default)]
struct Memo {
    visiting: bool,
    output: Option<usize>,
}

#[derive(Clone, Copy)]
struct Frame {
    source: usize,
    node: usize,
    finish: bool,
}

impl View {
    /// Its buffer lease owns this header after the view enters a join.
    pub(crate) const fn leased_header() -> usize {
        size_of::<Buffer<Option<Entry>>>()
    }

    /// Retain only comparisons that actually read a generated body slot.
    /// Unsupported source forms decline before publishing a partial view.
    pub(crate) fn prepare(
        literals: &[LiteralIr],
        variables: usize,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        let mut definitions = buffer(&mut context)?;
        definitions.resize(
            variables,
            None,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let mut generated = false;
        for literal in literals {
            tick(&mut context)?;
            if let LiteralIr::Bind { target, value } = literal
                && *target < variables
            {
                if definitions.slice()[*target].is_some() {
                    return Ok(None);
                }
                definitions.slice_mut()[*target] = Some(value);
                generated = true;
            }
        }
        if !generated {
            return Ok(None);
        }
        let mut comparisons = buffer(&mut context)?;
        for _ in literals {
            comparisons.push(
                None,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        let mut selected = false;
        for (literal, value) in literals.iter().enumerate() {
            tick(&mut context)?;
            let (LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison)) = value
            else {
                continue;
            };
            let ([left, right], relation) = comparison.operands();
            let mut reads_generated = false;
            for slot in left.inputs().chain(right.inputs()) {
                tick(&mut context)?;
                reads_generated |= definitions.slice().get(slot).is_some_and(Option::is_some);
            }
            if !reads_generated {
                continue;
            }
            let Some(left) = expand(left, definitions.slice(), &mut context)? else {
                return Ok(None);
            };
            let Some(right) = expand(right, definitions.slice(), &mut context)? else {
                return Ok(None);
            };
            let entry = Entry {
                left: left.embedded(&mut context)?,
                right: right.embedded(&mut context)?,
                relation,
            };
            comparisons.slice_mut()[literal] = Some(entry);
            selected = true;
        }
        Ok(selected.then_some(Self { comparisons }))
    }

    /// Original literal positions remain the only comparison identities.
    pub(crate) fn operands(&self, literal: usize) -> Option<([&Expression; 2], Relation)> {
        self.comparisons
            .slice()
            .get(literal)?
            .as_ref()
            .map(|entry| {
                (
                    [&entry.left.expression, &entry.right.expression],
                    entry.relation,
                )
            })
    }
}

fn expand(
    root: &Expression,
    definitions: &[Option<&Expression>],
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<Expanded>, FormulaFailure> {
    let mut sources = buffer(context)?;
    let mut size = 0usize;
    for expression in definitions
        .iter()
        .copied()
        .chain(std::iter::once(Some(root)))
    {
        tick(context)?;
        let source = expression.map(|expression| Source {
            expression,
            offset: size,
        });
        if let Some(expression) = expression {
            size = size
                .checked_add(expression.nodes.len())
                .ok_or_else(|| invalid(context))?;
        }
        sources.push(
            source,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
    }
    let mut memo = buffer(context)?;
    memo.resize(
        size,
        Memo::default(),
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )?;
    let mut stack = buffer(context)?;
    let Some(node) = root.nodes.len().checked_sub(1) else {
        return Ok(None);
    };
    push(
        &mut stack,
        Frame {
            source: definitions.len(),
            node,
            finish: false,
        },
        context,
    )?;
    let mut result = Expanded::new(context)?;
    while let Some(frame) = stack.pop() {
        tick(context)?;
        let source = sources.slice()[frame.source].expect("scheduled expression");
        let coordinate = source.offset + frame.node;
        if memo.slice()[coordinate].output.is_some() {
            continue;
        }
        let operation = &source.expression.nodes[frame.node];
        if !frame.finish {
            if memo.slice()[coordinate].visiting {
                return Ok(None);
            }
            memo.slice_mut()[coordinate].visiting = true;
            if !schedule(
                frame,
                operation,
                &sources.slice()[..definitions.len()],
                &mut stack,
                context,
            )? {
                return Ok(None);
            }
            continue;
        }
        let Some(output) = finish(
            operation,
            source,
            &sources.slice()[..definitions.len()],
            memo.slice(),
            &mut result,
            context,
        )?
        else {
            return Ok(None);
        };
        memo.slice_mut()[coordinate] = Memo {
            visiting: false,
            output: Some(output),
        };
    }
    Ok(Some(result))
}

/// Schedule the finishing frame before its children. A source reference shares
/// the definition's memo coordinate; ordinary children retain their source.
fn schedule(
    frame: Frame,
    operation: &Operation,
    definitions: &[Option<Source<'_>>],
    stack: &mut Buffer<Frame>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    push(
        stack,
        Frame {
            finish: true,
            ..frame
        },
        context,
    )?;
    let child = |node| Frame {
        node,
        finish: false,
        ..frame
    };
    match operation {
        Operation::Variable(slot) => {
            if let Some(Some(source)) = definitions.get(*slot) {
                let Some(node) = source.expression.nodes.len().checked_sub(1) else {
                    return Ok(false);
                };
                push(
                    stack,
                    Frame {
                        source: *slot,
                        node,
                        finish: false,
                    },
                    context,
                )?;
            }
        }
        Operation::Unary(_, node) | Operation::Absolute(node) => {
            if *node >= frame.node {
                return Ok(false);
            }
            push(stack, child(*node), context)?;
        }
        Operation::Binary(_, left, right) => {
            if *left >= frame.node || *right >= frame.node {
                return Ok(false);
            }
            push(stack, child(*right), context)?;
            push(stack, child(*left), context)?;
        }
        Operation::Constant(_) => {}
        Operation::Constructor(_) => return Ok(false),
    }
    Ok(true)
}

/// A scheduled node finishes only after every child has its shared output.
/// Aliases publish that output directly; operators append one checked DAG node.
fn finish(
    operation: &Operation,
    source: Source<'_>,
    definitions: &[Option<Source<'_>>],
    memo: &[Memo],
    result: &mut Expanded,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<usize>, FormulaFailure> {
    let mapped = |node: usize| memo[source.offset + node].output.expect("completed child");
    let mut alias = None;
    let output = match operation {
        Operation::Constant(value) => Operation::Constant(*value),
        Operation::Variable(slot) => {
            if let Some(Some(source)) = definitions.get(*slot) {
                alias = memo[source.offset + source.expression.nodes.len() - 1].output;
            }
            Operation::Variable(*slot)
        }
        Operation::Unary(operator, node) => Operation::Unary(*operator, mapped(*node)),
        Operation::Binary(operator, left, right) => {
            Operation::Binary(*operator, mapped(*left), mapped(*right))
        }
        Operation::Absolute(node) => Operation::Absolute(mapped(*node)),
        Operation::Constructor(_) => return Ok(None),
    };
    Ok(Some(match alias {
        Some(output) => output,
        None => result.push(output, context)?,
    }))
}

fn buffer<T>(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<Buffer<T>, FormulaFailure> {
    Buffer::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
}
fn push<T>(
    buffer: &mut Buffer<T>,
    value: T,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    buffer.push(
        value,
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
}
fn tick(context: &mut Context<'_, &Computation<'_, '_>>) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}
fn invalid(context: &Context<'_, &Computation<'_, '_>>) -> FormulaFailure {
    crate::formula_support::components::missing(context.work.location)
}

#[cfg(test)]
mod tests;
