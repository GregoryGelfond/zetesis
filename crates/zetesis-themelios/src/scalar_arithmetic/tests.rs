//! Independent pinned-evaluator and machine-boundary contracts.
use super::{absolute, binary, unary};
use themelios_program::symbol::Symbol;
use themelios_program::term::{BinaryOp, EvalError, Term, UnaryOp};

const BINARY_OPERATORS: [BinaryOp; 9] = [
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Mul,
    BinaryOp::Div,
    BinaryOp::Mod,
    BinaryOp::Pow,
    BinaryOp::BitAnd,
    BinaryOp::BitOr,
    BinaryOp::BitXor,
];
const BOUNDARY_VALUES: [i32; 17] = [
    i32::MIN,
    i32::MIN + 1,
    -65_536,
    -46_341,
    -32,
    -2,
    -1,
    0,
    1,
    2,
    30,
    31,
    32,
    46_340,
    46_341,
    i32::MAX - 1,
    i32::MAX,
];
fn term(value: i32) -> Term {
    Term::Symbolic(Symbol::Number(value))
}
fn reference_binary(operator: BinaryOp, left: i32, right: i32) -> Result<Symbol, EvalError> {
    Term::BinaryOperation {
        operator,
        left: Box::new(term(left)),
        right: Box::new(term(right)),
    }
    .evaluate()
}
fn reference_unary(operator: UnaryOp, operand: i32) -> Result<Symbol, EvalError> {
    Term::UnaryOperation {
        operator,
        argument: Box::new(term(operand)),
    }
    .evaluate()
}

#[test]
fn unary_boundaries_match_the_pinned_evaluator() {
    for operator in [UnaryOp::Negate, UnaryOp::BitwiseNot] {
        for operand in BOUNDARY_VALUES {
            assert_eq!(
                unary(operator, operand).map(Symbol::Number),
                reference_unary(operator, operand),
                "{operator:?} {operand}"
            );
        }
    }
}

#[test]
fn binary_boundaries_match_the_pinned_evaluator() {
    for operator in BINARY_OPERATORS {
        for left in BOUNDARY_VALUES {
            for right in BOUNDARY_VALUES {
                assert_eq!(
                    binary(operator, left, right).map(Symbol::Number),
                    reference_binary(operator, left, right),
                    "{left} {operator:?} {right}"
                );
            }
        }
    }
}

#[test]
fn absolute_boundaries_match_the_pinned_evaluator() {
    for operand in BOUNDARY_VALUES {
        assert_eq!(
            absolute(operand).map(Symbol::Number),
            Term::Absolute(Box::new(term(operand))).evaluate()
        );
    }
}

#[test]
fn checked_results_never_wrap() {
    for (operator, left, right) in [
        (BinaryOp::Add, i32::MAX, 1),
        (BinaryOp::Sub, i32::MIN, 1),
        (BinaryOp::Mul, 46_341, 46_341),
        (BinaryOp::Pow, 2, 31),
        (BinaryOp::Div, i32::MIN, -1),
        (BinaryOp::Mod, i32::MIN, -1),
    ] {
        assert_eq!(binary(operator, left, right), Err(EvalError::Overflow));
    }
    assert_eq!(unary(UnaryOp::Negate, i32::MIN), Err(EvalError::Overflow));
    assert_eq!(absolute(i32::MIN), Err(EvalError::Overflow));
}

#[test]
fn zero_divisors_are_undefined() {
    for left in BOUNDARY_VALUES {
        for operator in [BinaryOp::Div, BinaryOp::Mod] {
            assert_eq!(binary(operator, left, 0), Err(EvalError::Undefined));
        }
    }
}

#[test]
fn negative_exponents_are_undefined() {
    for left in BOUNDARY_VALUES {
        for right in [i32::MIN, -2, -1] {
            assert_eq!(
                binary(BinaryOp::Pow, left, right),
                Err(EvalError::Undefined)
            );
        }
    }
}

#[test]
fn division_truncates_toward_zero() {
    for (left, right, quotient) in [(-7, 3, -2), (7, -3, -2), (-7, -3, 2)] {
        assert_eq!(binary(BinaryOp::Div, left, right), Ok(quotient));
    }
}

#[test]
fn remainder_has_the_dividend_sign() {
    for (left, right, remainder) in [(-7, 3, -1), (7, -3, 1), (-7, -3, -1)] {
        assert_eq!(binary(BinaryOp::Mod, left, right), Ok(remainder));
    }
}

#[test]
fn zero_to_zero_is_one() {
    assert_eq!(binary(BinaryOp::Pow, 0, 0), Ok(1));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_260_907), ..Default::default() })]
    #[test]
    fn numeric_pairs_match_the_pinned_evaluator(left in proptest::num::i32::ANY, right in proptest::num::i32::ANY) {
        for operator in BINARY_OPERATORS {
            proptest::prop_assert_eq!(binary(operator, left, right).map(Symbol::Number), reference_binary(operator, left, right));
        }
    }
    #[test]
    fn numeric_unaries_match_the_pinned_evaluator(operand in proptest::num::i32::ANY) {
        for operator in [UnaryOp::Negate, UnaryOp::BitwiseNot] {
            proptest::prop_assert_eq!(unary(operator, operand).map(Symbol::Number), reference_unary(operator, operand));
        }
        proptest::prop_assert_eq!(absolute(operand).map(Symbol::Number), Term::Absolute(Box::new(term(operand))).evaluate());
    }
}
