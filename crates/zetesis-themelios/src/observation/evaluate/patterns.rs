//! Structural matching borrows input subtrees and captures generated local IDs.

use super::{Binding, Error, Interpreter, Metric, Operand, Resource, TermRef, Work};
use zetesis_core::{
    ValueNodeRef,
    catalog::{TermKey, TermRead},
};

pub(super) mod read;

pub(super) fn slots(operand: &Operand) -> usize {
    match operand {
        Operand::Variable(_) | Operand::Inverse { .. } => 1,
        Operand::Construct(_, arguments) => arguments.iter().map(slots).sum(),
        _ => 0,
    }
}

#[derive(Clone, Copy)]
enum Target<'input, 'key> {
    Input(TermRef<'input>),
    Generated(&'key TermKey),
}
enum Child<'input> {
    Input(TermRef<'input>),
    Generated(TermKey),
}
impl<'input> Child<'input> {
    fn target(&self) -> Target<'input, '_> {
        match self {
            Self::Input(value) => Target::Input(*value),
            Self::Generated(key) => Target::Generated(key),
        }
    }
}
impl<'input> Target<'input, '_> {
    fn value<'read>(self, read: TermRead<'read>) -> TermRef<'read>
    where
        'input: 'read,
    {
        match self {
            Self::Input(value) => value,
            Self::Generated(key) => read
                .term(key)
                .expect("interpreter target belongs to its arena"),
        }
    }
    fn child(
        self,
        index: usize,
        context: &mut Interpreter<'input, '_, '_>,
    ) -> Result<Child<'input>, Error> {
        context.work.step(1)?;
        Ok(match self {
            Self::Input(value) => {
                Child::Input(value.child(index).expect("validated pattern child"))
            }
            Self::Generated(key) => Child::Generated(
                context
                    .child(key, index)?
                    .expect("validated generated child"),
            ),
        })
    }
    fn equal_key(
        self,
        expected: &TermKey,
        context: &mut Interpreter<'input, '_, '_>,
    ) -> Result<bool, Error> {
        let reader = context.terms.read();
        let expected = reader.term(expected).expect("constructed comparison key");
        read::equal(expected, self.value(reader), context.work)
    }
}

fn resolved<'input: 'read, 'read>(
    operand: &Operand,
    binding: &'read Binding<'input>,
    metadata: crate::metadata::Read<'input>,
    reader: TermRead<'read>,
    work: &mut Work<'_>,
) -> Result<Option<TermRef<'read>>, Error> {
    match operand {
        Operand::Constant(scalar) => {
            work.step(1)?;
            Ok(Some(
                metadata.scalar(*scalar).expect("compiled pattern scalar"),
            ))
        }
        Operand::Variable(slot) => binding.value(*slot, reader, work),
        _ => Ok(None),
    }
}

fn children<'pattern, 'input>(
    pattern: &'pattern Operand,
    target: Target<'input, '_>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<Option<&'pattern [Operand]>, Error> {
    let Operand::Construct(shape, arguments) = pattern else {
        return Ok(None);
    };
    context.work.step(1)?;
    let expected = context
        .metadata
        .constructor(*shape)
        .expect("compiled pattern constructor");
    let actual = read::descriptor(target.value(context.terms.read()), context.work)?;
    Ok(read::constructor(expected, actual, context.work)?.then_some(arguments.as_slice()))
}

fn expression_matches<'input>(
    expression: &super::Template,
    target: Target<'input, '_>,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    if expression.multiple() {
        let mut found = false;
        super::values::each(expression, binding, context, |expected, _, context| {
            found |= target.equal_key(&expected, context)?;
            Ok(())
        })?;
        return Ok(found);
    }
    let mut metric = Metric::default();
    context.measure(expression, binding, 1, &mut metric)?;
    context.work.construction_check(metric)?;
    let expected = context.construct(expression, binding)?;
    target.equal_key(&expected, context)
}

fn capture<'input>(
    pattern: &Operand,
    target: Target<'input, '_>,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    context.work.step(1)?;
    let reader = context.terms.read();
    if let Some(expected) = resolved(pattern, binding, context.metadata, reader, context.work)? {
        return read::equal(expected, target.value(reader), context.work);
    }
    match pattern {
        Operand::Variable(slot) => {
            // An anonymous atom-pattern key is not an ordinary term value.
            if binding.is_bound(*slot) {
                return Ok(false);
            }
            match target {
                Target::Input(value) => binding.capture(*slot, value),
                Target::Generated(key) => {
                    let metric = context.metric(key)?;
                    context.work.check(
                        Resource::LocalBytes,
                        context.work.local_bytes + metric.payload(),
                        context.work.limits.max_local_bytes as u128,
                    )?;
                    binding.bind_term(*slot, key, metric, context.work)?;
                }
            }
            undo.push(*slot);
            Ok(true)
        }
        Operand::Inverse { slot, expression } => {
            let ValueNodeRef::Number(number) =
                read::descriptor(target.value(context.terms.read()), context.work)?
            else {
                return Ok(false);
            };
            super::inverse::bind(*slot, expression, number, binding, undo, context)
        }
        Operand::Any | Operand::Expression(_) => Ok(true),
        Operand::Construct(_, _) => {
            let Some(arguments) = children(pattern, target, context)? else {
                return Ok(false);
            };
            for (index, argument) in arguments.iter().enumerate() {
                context.work.step(1)?;
                if matches!(argument, Operand::Any | Operand::Expression(_)) {
                    continue;
                }
                let child = target.child(index, context)?;
                if !capture(argument, child.target(), binding, undo, context)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Operand::Constant(_) => unreachable!("compiled constant was resolved"),
    }
}

fn test<'input>(
    pattern: &Operand,
    target: Target<'input, '_>,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    context.work.step(1)?;
    let reader = context.terms.read();
    if let Some(expected) = resolved(pattern, binding, context.metadata, reader, context.work)? {
        return read::equal(expected, target.value(reader), context.work);
    }
    match pattern {
        Operand::Any => Ok(true),
        Operand::Expression(expression) | Operand::Inverse { expression, .. } => {
            expression_matches(expression, target, binding, context)
        }
        Operand::Construct(_, _) => {
            let Some(arguments) = children(pattern, target, context)? else {
                return Ok(false);
            };
            for (index, argument) in arguments.iter().enumerate() {
                context.work.step(1)?;
                if matches!(argument, Operand::Any) {
                    continue;
                }
                let child = target.child(index, context)?;
                if !test(argument, child.target(), binding, context)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Operand::Variable(_) => Ok(false),
        Operand::Constant(_) => unreachable!("compiled constant was resolved"),
    }
}

/// Only the admitted pattern is recursive. Captured input subtrees stay borrowed
/// regardless of depth and are not registered or measured merely for matching.
/// The caller releases every slot in undo, including after mismatch or refusal.
pub(super) fn matches_value<'input>(
    pattern: &Operand,
    value: TermRef<'input>,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    bind: bool,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    if bind {
        capture(pattern, Target::Input(value), binding, undo, context)
    } else {
        test_value(pattern, value, binding, context)
    }
}

pub(super) fn test_value<'input>(
    pattern: &Operand,
    value: TermRef<'input>,
    binding: &Binding<'input>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    test(pattern, Target::Input(value), binding, context)
}

/// Capture structural positions in a generated alternative, then test arithmetic
/// consumers after all captures exist. Captures retain IDs and logical charges.
pub(super) fn bind_key<'input>(
    pattern: &Operand,
    key: &TermKey,
    binding: &mut Binding<'input>,
    undo: &mut Vec<usize>,
    context: &mut Interpreter<'input, '_, '_>,
) -> Result<bool, Error> {
    let target = Target::Generated(key);
    if !capture(pattern, target, binding, undo, context)? {
        return Ok(false);
    }
    test(pattern, target, binding, context)
}

#[cfg(test)]
mod tests;
