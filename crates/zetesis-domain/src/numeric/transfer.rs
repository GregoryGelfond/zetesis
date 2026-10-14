//! Complete-binding transfer and the separate single pass over produced values.
use super::{
    AggregateFunction, BinaryOp, Binding, Error, Expression, ExpressionId, Guard, Input,
    Integrality, Interval, Relation, Rule, UnaryOp, Values, Widening, lattice::Abstract,
};

pub(super) fn expression<'a, R: Values, C: Copy>(
    input: &Input<'a, R, C>,
    root: ExpressionId,
    slots: &[Abstract<'a>],
    refinements: &[(ExpressionId, Interval)],
) -> Result<Abstract<'a>, Error<R::Error>> {
    let mut pending = Vec::new();
    let mut values: Vec<Abstract<'a>> = Vec::new();
    pending.try_reserve(1)?;
    pending.push((root, false));
    while let Some((id, ready)) = pending.pop() {
        input.budget.work(1)?;
        let node = input.expressions[id.0];
        if !ready {
            match node {
                Expression::Unary(_, value) | Expression::Absolute(value) => {
                    pending.try_reserve(2)?;
                    pending.extend([(id, true), (value, false)]);
                    continue;
                }
                Expression::Binary(_, left, right) => {
                    pending.try_reserve(3)?;
                    pending.extend([(id, true), (right, false), (left, false)]);
                    continue;
                }
                Expression::Constant(_) | Expression::Slot(_) | Expression::Unknown { .. } => {}
            }
        }
        let mut result = match node {
            Expression::Constant(value) => {
                Abstract::constant(value.0, &input.constants[value.0], input.budget)?
            }
            Expression::Slot(slot) => slots[slot].copied()?,
            Expression::Unknown { integer, reason } => {
                let mut value = if integer {
                    Abstract::numeric(Interval::Unbounded, input.budget)
                } else {
                    Abstract::top(Some(reason), input.budget)
                };
                value.widening = Some(reason);
                value
            }
            Expression::Unary(operator, _) => {
                let operand = values.pop().expect("postorder unary operand");
                let interval = if operand.bound.interval == Interval::Bottom {
                    Interval::Bottom
                } else if operator == UnaryOp::Negate {
                    range(operand.bound.interval)
                        .and_then(|value| value.unary(operator))
                        .map_or(Interval::Unbounded, interval)
                } else {
                    Interval::Unbounded
                };
                Abstract::numeric(interval, input.budget)
            }
            Expression::Absolute(_) => {
                let operand = values.pop().expect("postorder absolute operand");
                // This operator remains explicit for certificate consumers; its
                // general interval law is not part of this static proof boundary.
                Abstract::numeric(
                    if operand.bound.interval == Interval::Bottom {
                        Interval::Bottom
                    } else {
                        Interval::Unbounded
                    },
                    input.budget,
                )
            }
            Expression::Binary(operator, left, right) => {
                let b = values.pop().expect("postorder right operand");
                let a = values.pop().expect("postorder left operand");
                let bound = if a.bound.interval == Interval::Bottom
                    || b.bound.interval == Interval::Bottom
                {
                    Interval::Bottom
                } else if operator == BinaryOp::Sub && left == right && known_identity(input, left)?
                {
                    Interval::Bounded { low: 0, high: 0 }
                } else if covered(input, operator, left, right) {
                    range(a.bound.interval)
                        .zip(range(b.bound.interval))
                        .and_then(|(a, b)| a.binary(operator, b))
                        .map_or(Interval::Unbounded, interval)
                } else {
                    Interval::Unbounded
                };
                Abstract::numeric(bound, input.budget)
            }
        };
        for &(expression, restriction) in refinements {
            input.budget.work(1)?;
            if expression == id {
                if result.bound.integrality == Integrality::Integer {
                    result = result.refine(restriction, &input.constants)?;
                }
                break;
            }
        }
        values.try_reserve(1)?;
        values.push(result);
    }
    Ok(values.pop().expect("one root value"))
}
// Opaque summaries do not identify one denotation. A reused coordinate or
// two equal descriptors cannot justify correlation or guard-result reuse.
fn known_identity<R: Values, C: Copy>(
    input: &Input<'_, R, C>,
    root: ExpressionId,
) -> Result<bool, Error<R::Error>> {
    let mut pending = Vec::new();
    pending.try_reserve(1)?;
    pending.push(root);
    while let Some(id) = pending.pop() {
        input.budget.work(1)?;
        match input.expressions[id.0] {
            Expression::Unknown { .. } => return Ok(false),
            Expression::Unary(_, value) | Expression::Absolute(value) => {
                pending.try_reserve(1)?;
                pending.push(value);
            }
            Expression::Binary(_, left, right) => {
                pending.try_reserve(2)?;
                pending.extend([left, right]);
            }
            Expression::Constant(_) | Expression::Slot(_) => {}
        }
    }
    Ok(true)
}
fn range(value: Interval) -> Option<crate::Range> {
    match value {
        Interval::Bottom | Interval::Unbounded => None,
        Interval::Bounded { low, high } => crate::Range::new(low, high),
    }
}
fn interval(value: crate::Range) -> Interval {
    let (low, high) = value.bounds();
    Interval::Bounded { low, high }
}
fn literal<R: Values, C: Copy>(input: &Input<'_, R, C>, id: ExpressionId) -> Option<i32> {
    match input.expressions[id.0] {
        Expression::Constant(value) => input.constants[value.0].number,
        _ => None,
    }
}
fn covered<R: Values, C: Copy>(
    input: &Input<'_, R, C>,
    operator: BinaryOp,
    left: ExpressionId,
    right: ExpressionId,
) -> bool {
    match operator {
        BinaryOp::Add | BinaryOp::Sub => true,
        BinaryOp::Mul => literal(input, left).is_some() || literal(input, right).is_some(),
        BinaryOp::Div | BinaryOp::Mod => literal(input, right).is_some_and(|value| value != 0),
        BinaryOp::Pow | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => false,
    }
}
pub(super) struct Local<'a> {
    pub domains: Vec<Abstract<'a>>,
    pub produced: Vec<Abstract<'a>>,
    refinements: Vec<(ExpressionId, Interval)>,
}
pub(super) fn rule<'a, R: Values, C: Copy>(
    input: &Input<'a, R, C>,
    rule: &Rule,
    arguments: &[Abstract<'a>],
) -> Result<Local<'a>, Error<R::Error>> {
    let mut domains = Vec::new();
    domains.try_reserve(rule.slots)?;
    for _ in 0..rule.slots {
        input.budget.work(1)?;
        domains.push(Abstract::top(None, input.budget));
    }
    // Positive occurrences constrain complete bindings, regardless of join order.
    for binding in &rule.bindings {
        input.budget.work(1)?;
        if let Binding::Positive { slot, argument } = *binding {
            domains[slot] = domains[slot].meet(&arguments[argument.0], &input.constants)?;
        }
    }
    for binding in &rule.bindings {
        input.budget.work(1)?;
        if let Binding::Positive { slot, .. } = *binding
            && domains[slot].values.is_none()
        {
            domains[slot].widening = Some(Widening::Dependency);
        }
    }
    for binding in &rule.bindings {
        input.budget.work(1)?;
        let (slot, value) = match *binding {
            Binding::Positive { .. } => continue,
            Binding::Assign { slot, value } => (slot, expression(input, value, &domains, &[])?),
            Binding::Range { slot, low, high } => {
                let low = expression(input, low, &domains, &[])?;
                let high = expression(input, high, &domains, &[])?;
                let bound = low
                    .bound
                    .interval
                    .endpoints()
                    .zip(high.bound.interval.endpoints())
                    .map_or(Interval::Bottom, |((low, _), (_, high))| {
                        Interval::checked(i64::from(low), i64::from(high))
                    });
                (slot, Abstract::numeric(bound, input.budget))
            }
            Binding::Aggregate { slot, function } => {
                let value = match function {
                    AggregateFunction::Count
                    | AggregateFunction::Sum
                    | AggregateFunction::SumPlus => {
                        Abstract::numeric(Interval::Unbounded, input.budget)
                    }
                    AggregateFunction::Min | AggregateFunction::Max => {
                        Abstract::top(Some(Widening::HeadTerm), input.budget)
                    }
                };
                (slot, value)
            }
        };
        domains[slot] = domains[slot].meet(&value, &input.constants)?;
    }
    let mut produced = Vec::new();
    produced.try_reserve(domains.len())?;
    for value in &domains {
        produced.push(value.copied()?);
    }
    let mut local = Local {
        domains,
        produced,
        refinements: Vec::new(),
    };
    local.refinements.try_reserve(
        rule.guards
            .len()
            .checked_mul(2)
            .ok_or(Error::InvalidInput)?,
    )?;
    for guard in &rule.guards {
        input.budget.work(1)?;
        match *guard {
            Guard::Compare {
                left,
                relation,
                right,
            } => refine_comparison(input, &mut local, left, relation, right)?,
            Guard::Membership { slot, low, high } => {
                let lower = expression(input, low, &local.produced, &local.refinements)?;
                let upper = expression(input, high, &local.produced, &local.refinements)?;
                if local.produced[slot].bound.integrality == Integrality::Integer
                    && lower.bound.integrality == Integrality::Integer
                    && upper.bound.integrality == Integrality::Integer
                {
                    let interval = lower
                        .bound
                        .interval
                        .endpoints()
                        .zip(upper.bound.interval.endpoints())
                        .map_or(Interval::Bottom, |((low, _), (_, high))| {
                            Interval::checked(i64::from(low), i64::from(high))
                        });
                    local.produced[slot] =
                        local.produced[slot].refine(interval, &input.constants)?;
                }
            }
        }
    }
    Ok(local)
}
fn refine_comparison<'a, R: Values, C: Copy>(
    input: &Input<'a, R, C>,
    local: &mut Local<'a>,
    left: ExpressionId,
    relation: Relation,
    right: ExpressionId,
) -> Result<(), Error<R::Error>> {
    let a = expression(input, left, &local.produced, &local.refinements)?;
    let b = expression(input, right, &local.produced, &local.refinements)?;
    if a.bound.integrality != Integrality::Integer
        || b.bound.integrality != Integrality::Integer
        || relation == Relation::Neq
    {
        return Ok(());
    }
    let restrictions = match (a.bound.interval.endpoints(), b.bound.interval.endpoints()) {
        (Some((al, ah)), Some((bl, bh))) => {
            let (al, ah, bl, bh) = (i64::from(al), i64::from(ah), i64::from(bl), i64::from(bh));
            let (min, max) = (i64::from(i32::MIN), i64::from(i32::MAX));
            match relation {
                Relation::Lt => (
                    Interval::checked(min, bh - 1),
                    Interval::checked(al + 1, max),
                ),
                Relation::Le => (Interval::checked(min, bh), Interval::checked(al, max)),
                Relation::Gt => (
                    Interval::checked(bl + 1, max),
                    Interval::checked(min, ah - 1),
                ),
                Relation::Ge => (Interval::checked(bl, max), Interval::checked(min, ah)),
                Relation::Eq => (Interval::checked(bl, bh), Interval::checked(al, ah)),
                Relation::Neq => unreachable!("inequality does not refine"),
            }
        }
        _ => (Interval::Bottom, Interval::Bottom),
    };
    // Both bounds come from this guard's current operands; the next guard sees
    // their refinements. No local fixed point is silently computed.
    refine_operand(input, local, left, restrictions.0)?;
    refine_operand(input, local, right, restrictions.1)
}
fn refine_operand<'a, R: Values, C: Copy>(
    input: &Input<'a, R, C>,
    local: &mut Local<'a>,
    id: ExpressionId,
    restriction: Interval,
) -> Result<(), Error<R::Error>> {
    if !known_identity(input, id)? {
        return Ok(());
    }
    let mut recorded = false;
    for (expression, old) in &mut local.refinements {
        input.budget.work(1)?;
        if *expression == id {
            *old = old.meet(restriction);
            recorded = true;
            break;
        }
    }
    if !recorded {
        local.refinements.try_reserve(1)?;
        local.refinements.push((id, restriction));
    }
    let affine = match input.expressions[id.0] {
        Expression::Slot(slot) => Some((slot, 0_i64, false)),
        Expression::Binary(operator, left, right) => match (
            input.expressions[left.0],
            input.expressions[right.0],
            operator,
        ) {
            (Expression::Slot(slot), _, BinaryOp::Add) => {
                literal(input, right).map(|c| (slot, i64::from(c), false))
            }
            (Expression::Slot(slot), _, BinaryOp::Sub) => {
                literal(input, right).map(|c| (slot, -i64::from(c), false))
            }
            (_, Expression::Slot(slot), BinaryOp::Sub) => {
                literal(input, left).map(|c| (slot, i64::from(c), true))
            }
            _ => None,
        },
        _ => None,
    };
    if let Some((slot, constant, reversed)) = affine {
        let bound = restriction
            .endpoints()
            .map_or(Interval::Bottom, |(low, high)| {
                if reversed {
                    Interval::checked(constant - i64::from(high), constant - i64::from(low))
                } else {
                    Interval::checked(i64::from(low) - constant, i64::from(high) - constant)
                }
            });
        local.produced[slot] = local.produced[slot].refine(bound, &input.constants)?;
    }
    Ok(())
}
pub(super) fn head<'a, R: Values, C: Copy>(
    input: &Input<'a, R, C>,
    local: &Local<'a>,
    value: ExpressionId,
) -> Result<Abstract<'a>, Error<R::Error>> {
    expression(input, value, &local.produced, &local.refinements)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Key, Reader, assert_work_stop, leave_work, test_budget};
    use super::*;
    #[test]
    fn expression_refinement_lookup_charges_every_examined_record() {
        let budget = test_budget();
        let mut input = Input::<_, ()>::new(&Reader, &budget);
        let mut refinements = Vec::new();
        for number in 0..4 {
            let key = input.constant(Key::Number(number)).unwrap();
            let id = input.expression(Expression::Constant(key)).unwrap();
            refinements.push((id, Interval::Unbounded));
        }
        let root = input.expression(Expression::Slot(0)).unwrap();
        let slots = [Abstract::top(None, &budget)];
        leave_work(&budget, 3);
        assert_work_stop(&expression(&input, root, &slots, &refinements), &budget);
    }
    #[test]
    fn recording_refinements_charges_every_examined_record() {
        let budget = test_budget();
        let mut input = Input::<_, ()>::new(&Reader, &budget);
        let mut refinements = Vec::new();
        for number in 0..4 {
            let key = input.constant(Key::Number(number)).unwrap();
            let id = input.expression(Expression::Constant(key)).unwrap();
            refinements.push((id, Interval::Unbounded));
        }
        let root = refinements[3].0;
        let mut local = Local {
            domains: Vec::new(),
            produced: Vec::new(),
            refinements,
        };
        leave_work(&budget, 3);
        assert_work_stop(
            &refine_operand(&input, &mut local, root, Interval::Unbounded),
            &budget,
        );
    }
    #[test]
    fn identity_walk_charges_every_expression_node() {
        let budget = test_budget();
        let mut input = Input::<_, ()>::new(&Reader, &budget);
        let mut root = input.expression(Expression::Slot(0)).unwrap();
        for _ in 0..3 {
            root = input
                .expression(Expression::Unary(UnaryOp::Negate, root))
                .unwrap();
        }
        leave_work(&budget, 2);
        assert_work_stop(&known_identity(&input, root), &budget);
    }
}
