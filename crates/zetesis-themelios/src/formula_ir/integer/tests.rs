use super::*;
use crate::ExpansionLimits;
use crate::expansion::Budget;
use crate::formula_guard::GuardComparison;
use crate::formula_ir::testing::with_compiler;
use crate::formula_ir::{Comparison, Compiler, Expression, LiteralIr};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::Relation;
use zetesis_core::{Sign, Value, ValueNodeRef};

const X: Operation = Operation::Variable(0);
const Y: Operation = Operation::Variable(1);

/// The literal `left = right`, built by the compiler's comparison constructor.
fn compared(
    compiler: &mut Compiler<'_>,
    left: Vec<Operation>,
    right: Vec<Operation>,
) -> Comparison {
    let LiteralIr::Compare(comparison) = compiler
        .compare(Expression::new(left), Relation::Eq, Expression::new(right))
        .unwrap()
    else {
        unreachable!("the constructor builds a comparison literal")
    };
    comparison
}

fn reason(comparison: &Comparison) -> Option<WithoutForm> {
    comparison.form().err()
}

fn one(compiler: &mut Compiler<'_>) -> Operation {
    Operation::Constant(compiler.scalar(&Value::Number(1)).unwrap())
}

fn symbol(compiler: &mut Compiler<'_>) -> Operation {
    Operation::Constant(compiler.scalar(&Value::Symbol("a".into())).unwrap())
}

/// `X + 1`, three nodes over the compiler's own constant 1.
fn successor(compiler: &mut Compiler<'_>) -> Vec<Operation> {
    vec![X, one(compiler), Operation::Binary(BinaryOp::Add, 0, 1)]
}

/// `X * Y - 1`, five nodes over the compiler's own constant 1.
fn product(compiler: &mut Compiler<'_>) -> Vec<Operation> {
    vec![
        X,
        Y,
        Operation::Binary(BinaryOp::Mul, 0, 1),
        one(compiler),
        Operation::Binary(BinaryOp::Sub, 2, 3),
    ]
}

/// `X` negated `count` times.
fn negations(count: usize) -> Vec<Operation> {
    let mut nodes = vec![X];
    nodes.extend((0..count).map(|argument| Operation::Unary(UnaryOp::Negate, argument)));
    nodes
}

fn constructor(compiler: &mut Compiler<'_>) -> Operation {
    let shape = compiler
        .source
        .constructor(
            ValueNodeRef::Function {
                name: "f",
                sign: Sign::Positive,
                arity: 1,
            },
            compiler.limits,
            compiler.counters,
            compiler.location,
        )
        .unwrap();
    Operation::Constructor(Box::new(crate::formula_value::Constructor {
        shape,
        arguments: vec![0],
    }))
}

#[test]
fn variables_numbers_and_earlier_arguments_have_a_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        for nodes in [
            vec![X],
            vec![X, Operation::Unary(UnaryOp::BitwiseNot, 0)],
            vec![
                X,
                Operation::Absolute(0),
                Y,
                Operation::Binary(BinaryOp::Pow, 1, 2),
            ],
        ] {
            assert!(compared(compiler, nodes, vec![Y]).form().is_ok());
        }
    });
}

#[test]
fn sixteen_nodes_have_a_form_and_seventeen_none() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let sixteen = compared(compiler, negations(MAX_NODES - 1), vec![Y]);
        assert!(sixteen.form().is_ok());
        let left = compared(compiler, negations(MAX_NODES), vec![Y]);
        assert_eq!(reason(&left), Some(WithoutForm::LongOperand));
        let right = compared(compiler, vec![Y], negations(MAX_NODES));
        assert_eq!(reason(&right), Some(WithoutForm::LongOperand));
    });
}

#[test]
fn an_empty_operand_counts_as_long() {
    assert_eq!(shape(&[]), Err(WithoutForm::LongOperand));
}

#[cfg(target_pointer_width = "64")]
#[test]
fn a_slot_beyond_u32_has_no_form() {
    let slot = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
    with_compiler(ExpansionLimits::default(), |compiler| {
        let comparison = compared(compiler, vec![Operation::Variable(slot)], vec![X]);
        assert_eq!(reason(&comparison), Some(WithoutForm::WideSlot));
    });
}

#[test]
fn a_constructor_operand_has_no_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let constructor = constructor(compiler);
        let comparison = compared(compiler, vec![X, constructor], vec![Y]);
        assert_eq!(reason(&comparison), Some(WithoutForm::Constructor));
    });
}

#[test]
fn a_forward_or_self_argument_has_no_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let forward = compared(compiler, vec![Operation::Absolute(1), X], vec![Y]);
        assert_eq!(reason(&forward), Some(WithoutForm::ArgumentPosition));
        let itself = compared(
            compiler,
            vec![Y],
            vec![X, Operation::Binary(BinaryOp::Add, 0, 1)],
        );
        assert_eq!(reason(&itself), Some(WithoutForm::ArgumentPosition));
    });
}

#[test]
fn a_nonnumeric_constant_has_no_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let symbol = symbol(compiler);
        let comparison = compared(compiler, vec![X], vec![symbol]);
        assert_eq!(reason(&comparison), Some(WithoutForm::NonnumericConstant));
    });
}

#[test]
fn a_shape_failure_outranks_a_nonnumeric_constant() {
    // `a < f(X)`: the right operand's constructor is counted, not the left
    // operand's symbol, because shapes are judged before constants are read.
    with_compiler(ExpansionLimits::default(), |compiler| {
        let symbol = symbol(compiler);
        let constructor = constructor(compiler);
        let comparison = compared(compiler, vec![symbol], vec![X, constructor]);
        assert_eq!(reason(&comparison), Some(WithoutForm::Constructor));
    });
}

#[test]
fn the_left_operand_shape_outranks_the_right() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let constructor = constructor(compiler);
        let comparison = compared(compiler, negations(MAX_NODES), vec![X, constructor]);
        assert_eq!(reason(&comparison), Some(WithoutForm::LongOperand));
    });
}

#[test]
fn compiling_charges_steps_bytes_and_constant_reads() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let successor = successor(compiler);
        let budget = compiler.budget.usage();
        let work = compiler.counters.accounting.work;
        let comparison = compared(compiler, successor, vec![Y]);
        assert!(comparison.form().is_ok());
        let after = compiler.budget.usage();
        assert_eq!(after.term_work - budget.term_work, 4);
        assert_eq!(after.scalar_bytes - budget.scalar_bytes, 32);
        // One constant read, five formula work units.
        assert_eq!(compiler.counters.accounting.work - work, 5);
    });
}

#[test]
fn each_constant_read_charges_five_work_units() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let (left, right) = (successor(compiler), product(compiler));
        let work = compiler.counters.accounting.work;
        compared(compiler, left, right);
        assert_eq!(compiler.counters.accounting.work - work, 10);
    });
}

#[test]
fn a_nonnumeric_constant_charges_steps_and_no_bytes() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let symbol = symbol(compiler);
        let before = compiler.budget.usage();
        compared(compiler, vec![X, symbol], vec![Y]);
        let after = compiler.budget.usage();
        assert_eq!(after.term_work - before.term_work, 3);
        assert_eq!(after.scalar_bytes, before.scalar_bytes);
    });
}

#[test]
fn a_literal_the_shape_rule_excludes_is_charged_nothing() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let before = compiler.budget.usage();
        let work = compiler.counters.accounting.work;
        compared(compiler, vec![Y], vec![Operation::Absolute(1), X]);
        assert_eq!(compiler.budget.usage(), before);
        assert_eq!(compiler.counters.accounting.work, work);
    });
}

#[test]
fn the_step_charge_is_admitted_at_its_total_and_refused_below() {
    // `X + 1 = Y` has four steps; the compiler's own scalar admission charges
    // no term work, so the ceiling is the form's alone.
    for (ceiling, admitted) in [(4, true), (3, false)] {
        let limits = ExpansionLimits {
            max_term_work: ceiling,
            ..ExpansionLimits::default()
        };
        with_compiler(limits, |compiler| {
            let successor = successor(compiler);
            let result = compiler.compare(
                Expression::new(successor),
                Relation::Eq,
                Expression::new(vec![Y]),
            );
            assert_eq!(result.is_ok(), admitted, "ceiling {ceiling}");
        });
    }
}

#[test]
fn the_byte_charge_is_admitted_at_its_total_and_refused_below() {
    // The constant's component admission charges no expansion bytes, so the
    // ceiling is the form's alone: eight bytes for each of `X + 1 = Y`'s four
    // steps.
    for (ceiling, admitted) in [(32, true), (31, false)] {
        let limits = ExpansionLimits {
            max_scalar_bytes: ceiling,
            ..ExpansionLimits::default()
        };
        with_compiler(limits, |compiler| {
            let successor = successor(compiler);
            let result = compiler.compare(
                Expression::new(successor),
                Relation::Eq,
                Expression::new(vec![Y]),
            );
            assert_eq!(result.is_ok(), admitted, "ceiling {ceiling}");
        });
    }
}

#[test]
fn steps_take_eight_bytes_each() {
    assert_eq!(size_of::<Step>(), 8);
    assert_eq!(IntegerComparison::bytes(3), 24);
}

#[test]
fn an_expression_carries_no_form() {
    assert_eq!(size_of::<Expression>(), 24);
}

#[test]
fn comparison_literal_sizes_are_pinned() {
    // Charges that count the size of compiled literals read these sizes;
    // pinning them makes any change visible.
    assert_eq!(size_of::<Comparison>(), 80);
    assert_eq!(size_of::<LiteralIr>(), 104);
    assert_eq!(size_of::<GuardComparison>(), 72);
}

#[test]
fn the_constant_charge_counts_steps_and_variable_steps() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let (left, right) = (successor(compiler), product(compiler));
        let comparison = compared(compiler, left, right);
        let form = comparison.form().unwrap();
        // Eight steps, three of them variables.
        assert_eq!((form.steps(), form.charge()), (8, 11));
    });
}

#[test]
fn a_decided_attempt_performs_every_step() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let (left, right) = (successor(compiler), product(compiler));
        let comparison = compared(compiler, left, right);
        let form = comparison.form().unwrap();
        // X + 1 against X * Y - 1 at X = 6, Y = 7: 7 against 41.
        let attempt = form.attempt(|slot| Some([6, 7][slot]));
        assert_eq!(
            attempt,
            Attempt {
                order: Some(Ordering::Less),
                entered: 2,
                steps: 8,
                units: 11,
            }
        );
    });
}

#[test]
fn an_attempt_stops_at_the_left_operand_first_failing_read() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let (left, right) = (successor(compiler), product(compiler));
        let comparison = compared(compiler, left, right);
        let attempt = comparison.form().unwrap().attempt(|_| None);
        assert_eq!(
            attempt,
            Attempt {
                order: None,
                entered: 1,
                steps: 1,
                units: 2,
            }
        );
    });
}

#[test]
fn an_attempt_stops_at_the_right_operand_failing_operation() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let (left, right) = (successor(compiler), product(compiler));
        let comparison = compared(compiler, left, right);
        // X * Y overflows at the right operand's third step: the left
        // operand's three steps and four units, then X, Y and the failing
        // product, five units.
        let attempt = comparison.form().unwrap().attempt(|_| Some(i32::MAX / 2));
        assert_eq!(
            attempt,
            Attempt {
                order: None,
                entered: 2,
                steps: 6,
                units: 9,
            }
        );
    });
}

#[test]
fn an_attempt_follows_the_checked_scalar_operations() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let successor = successor(compiler);
        let comparison = compared(compiler, successor, vec![Y]);
        let form = comparison.form().unwrap();
        assert_eq!(
            form.attempt(|slot| Some([41, 42][slot])).order,
            Some(Ordering::Equal)
        );
        assert_eq!(form.attempt(|_| Some(i32::MAX)).order, None);
    });
}

#[test]
fn a_copy_keeps_the_form_and_charges_its_bytes() {
    let location = crate::ProgramSite::source(Location {
        source: SourceId::new(7),
        span: Span::empty(ByteOffset::new(0)),
    });
    with_compiler(ExpansionLimits::default(), |compiler| {
        let original = compared(compiler, negations(1), vec![Y]);
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut nodes = 0;
        let copy = original
            .copy(
                || {
                    nodes += 1;
                    Ok(())
                },
                &mut budget,
                location,
            )
            .unwrap();
        assert_eq!(nodes, 3);
        assert_eq!(
            copy.form().unwrap().attempt(|_| Some(3)).order,
            Some(Ordering::Less)
        );
        assert_eq!(budget.usage().scalar_bytes, 24);
    });
}
