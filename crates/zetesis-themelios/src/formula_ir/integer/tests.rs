use super::*;
use crate::ExpansionLimits;
use crate::expansion::Budget;
use crate::formula_ir::testing::with_compiler;
use crate::formula_ir::{Compiler, Expression};
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::{Sign, Value, ValueNodeRef};

const X: Operation = Operation::Variable(0);
const Y: Operation = Operation::Variable(1);

/// A form over `nodes` whose every constant is the number 1.
fn formed(nodes: Vec<Operation>) -> Expression {
    Expression::with_integer_form(nodes, |_| Some(1))
}

fn reason(expression: &Expression) -> Option<WithoutForm> {
    expression.integer().err()
}

/// `X + 1` and `X * Y - 1` over the compiler's own constant 1.
fn arithmetic(compiler: &mut Compiler<'_>) -> [Vec<Operation>; 2] {
    let one = compiler.scalar(&Value::Number(1)).unwrap();
    [
        vec![
            X,
            Operation::Constant(one),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
        vec![
            X,
            Y,
            Operation::Binary(BinaryOp::Mul, 0, 1),
            Operation::Constant(one),
            Operation::Binary(BinaryOp::Sub, 2, 3),
        ],
    ]
}

#[test]
fn variables_numbers_and_earlier_arguments_have_a_form() {
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
        assert!(formed(nodes).integer().is_ok());
    }
}

#[test]
fn operands_beyond_one_to_sixteen_nodes_have_no_form() {
    let negations = |count: usize| {
        let mut nodes = vec![X];
        nodes.extend((0..count).map(|argument| Operation::Unary(UnaryOp::Negate, argument)));
        nodes
    };
    assert!(formed(negations(MAX_NODES - 1)).integer().is_ok());
    assert_eq!(
        reason(&formed(negations(MAX_NODES))),
        Some(WithoutForm::Bound)
    );
    assert_eq!(reason(&formed(Vec::new())), Some(WithoutForm::Bound));
}

#[cfg(target_pointer_width = "64")]
#[test]
fn a_slot_beyond_u32_has_no_form() {
    let slot = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
    let expression = formed(vec![Operation::Variable(slot)]);
    assert_eq!(reason(&expression), Some(WithoutForm::Bound));
}

#[test]
fn a_constructor_operand_has_no_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
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
        let constructor = Operation::Constructor(Box::new(crate::formula_value::Constructor {
            shape,
            arguments: vec![0],
        }));
        let expression = compiler.integer_expression(vec![X, constructor]).unwrap();
        assert_eq!(reason(&expression), Some(WithoutForm::Constructor));
    });
}

#[test]
fn a_forward_or_self_argument_has_no_form() {
    let forward = formed(vec![Operation::Absolute(1), X]);
    assert_eq!(reason(&forward), Some(WithoutForm::ArgumentPosition));
    let itself = formed(vec![X, Operation::Binary(BinaryOp::Add, 0, 1)]);
    assert_eq!(reason(&itself), Some(WithoutForm::ArgumentPosition));
}

#[test]
fn a_nonnumeric_constant_has_no_form() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let symbol = compiler.scalar(&Value::Symbol("a".into())).unwrap();
        let expression = compiler
            .integer_expression(vec![Operation::Constant(symbol)])
            .unwrap();
        assert_eq!(reason(&expression), Some(WithoutForm::NonnumericConstant));
    });
}

#[test]
fn the_plain_constructor_never_makes_a_form() {
    let expression = Expression::new(vec![X]);
    assert_eq!(reason(&expression), Some(WithoutForm::NotCompiled));
}

#[test]
fn compiling_a_form_charges_its_steps_and_bytes() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let [successor, _] = arithmetic(compiler);
        let before = compiler.budget.usage();
        let expression = compiler.integer_expression(successor).unwrap();
        let after = compiler.budget.usage();
        assert!(expression.integer().is_ok());
        assert_eq!(after.term_work - before.term_work, 3);
        assert_eq!(after.scalar_bytes - before.scalar_bytes, 24);
    });
}

#[test]
fn a_nonnumeric_constant_charges_its_steps_and_no_bytes() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let symbol = compiler.scalar(&Value::Symbol("a".into())).unwrap();
        let before = compiler.budget.usage();
        compiler
            .integer_expression(vec![X, Operation::Constant(symbol)])
            .unwrap();
        let after = compiler.budget.usage();
        assert_eq!(after.term_work - before.term_work, 2);
        assert_eq!(after.scalar_bytes, before.scalar_bytes);
    });
}

#[test]
fn an_operand_the_shape_rule_excludes_is_charged_nothing() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let before = compiler.budget.usage();
        compiler
            .integer_expression(vec![Operation::Absolute(1), X])
            .unwrap();
        assert_eq!(compiler.budget.usage(), before);
    });
}

#[test]
fn a_refused_step_charge_refuses_the_operand() {
    let limits = ExpansionLimits {
        max_term_work: 2,
        ..ExpansionLimits::default()
    };
    with_compiler(limits, |compiler| {
        let [successor, _] = arithmetic(compiler);
        assert!(compiler.integer_expression(successor).is_err());
    });
}

#[test]
fn steps_take_eight_bytes_each() {
    assert_eq!(size_of::<Step>(), 8);
    assert_eq!(IntegerOperand::bytes(3), 24);
}

#[test]
fn the_constant_charge_counts_nodes_and_variable_steps() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let [_, product] = arithmetic(compiler);
        let expression = compiler.integer_expression(product).unwrap();
        let form = expression.integer().unwrap();
        assert_eq!((form.nodes(), form.charge()), (5, 7));
    });
}

#[test]
fn evaluation_follows_the_checked_scalar_operations() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let [successor, product] = arithmetic(compiler);
        let successor = compiler.integer_expression(successor).unwrap();
        let product = compiler.integer_expression(product).unwrap();
        let successor = successor.integer().unwrap();
        let product = product.integer().unwrap();
        assert_eq!(successor.evaluate(|_| Some(41)), Some(42));
        assert_eq!(product.evaluate(|slot| Some([6, 7][slot])), Some(41));
        assert_eq!(successor.evaluate(|_| Some(i32::MAX)), None);
        assert_eq!(successor.evaluate(|_| None), None);
    });
}

#[test]
fn a_copy_keeps_the_form_and_charges_its_bytes() {
    let location = crate::ProgramSite::source(Location {
        source: SourceId::new(7),
        span: Span::empty(ByteOffset::new(0)),
    });
    let original = formed(vec![X, Operation::Unary(UnaryOp::Negate, 0)]);
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let copy = original
        .copy_over(
            vec![X, Operation::Unary(UnaryOp::Negate, 0)],
            &mut budget,
            location,
        )
        .unwrap();
    assert_eq!(copy.integer().unwrap().evaluate(|_| Some(3)), Some(-3));
    assert_eq!(budget.usage().scalar_bytes, 16);
    let plain = Expression::new(vec![X])
        .copy_over(vec![X], &mut budget, location)
        .unwrap();
    assert_eq!(reason(&plain), Some(WithoutForm::NotCompiled));
    assert_eq!(budget.usage().scalar_bytes, 16);
}
