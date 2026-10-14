use super::*;
use themelios_program::term::BinaryOp;

#[test]
fn literal_transfers_enclose_checked_scalar_results() {
    for lower in -5..=5 {
        for upper in lower..=5 {
            let mut input = Range::point(lower);
            input.include(upper);
            for literal in -5..=5 {
                for operator in [BinaryOp::Mul, BinaryOp::Div, BinaryOp::Mod] {
                    let result = input.binary(operator, Range::point(literal));
                    if matches!(operator, BinaryOp::Div | BinaryOp::Mod) && literal == 0 {
                        assert!(result.is_none());
                        continue;
                    }
                    let result = result.expect("small nonzero-divisor arithmetic is total");
                    for value in lower..=upper {
                        let actual = crate::scalar_arithmetic::binary(operator, value, literal)
                            .expect("bounded scalar arithmetic");
                        assert!(
                            result.bounds().0 <= actual && actual <= result.bounds().1,
                            "{lower}..={upper} {operator:?} {literal}: {actual}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn exceptional_division_declines_both_operators() {
    for operator in [BinaryOp::Div, BinaryOp::Mod] {
        assert!(
            Range::point(i32::MIN)
                .binary(operator, Range::point(-1))
                .is_none()
        );
    }
}

#[test]
fn an_overflowing_product_declines_its_range() {
    let mut input = Range::point(0);
    input.include(1_073_741_824);
    assert!(input.binary(BinaryOp::Mul, Range::point(2)).is_none());
}

#[test]
fn variable_transfers_enclose_checked_scalar_results() {
    for left_low in -3..=3 {
        for left_high in left_low..=3 {
            let mut left = Range::point(left_low);
            left.include(left_high);
            for right_low in -3..=3 {
                for right_high in right_low..=3 {
                    let mut right = Range::point(right_low);
                    right.include(right_high);
                    for operator in [BinaryOp::Mul, BinaryOp::Div, BinaryOp::Mod] {
                        let bound = left.binary(operator, right);
                        if matches!(operator, BinaryOp::Div | BinaryOp::Mod)
                            && right_low <= 0
                            && right_high >= 0
                        {
                            assert!(bound.is_none());
                            continue;
                        }
                        let bound = bound.expect("small total arithmetic has a range");
                        for x in left_low..=left_high {
                            for y in right_low..=right_high {
                                let actual =
                                    crate::scalar_arithmetic::binary(operator, x, y).unwrap();
                                assert!(
                                    bound.bounds().0 <= actual && actual <= bound.bounds().1,
                                    "{left_low}..={left_high} {operator:?} {right_low}..={right_high}: {actual}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn absolute_transfers_enclose_every_checked_result() {
    for low in -10..=10 {
        for high in low..=10 {
            let range = Range::new(low, high).unwrap().absolute().unwrap();
            for value in low..=high {
                let absolute = crate::scalar_arithmetic::absolute(value).unwrap();
                let (lower, upper) = range.bounds();
                assert!(lower <= absolute && absolute <= upper);
            }
        }
    }
}

#[test]
fn absolute_declines_the_unrepresentable_endpoint() {
    assert_eq!(Range::new(i32::MIN, 1).unwrap().absolute(), None);
    assert_eq!(
        Range::new(i32::MIN + 1, 0)
            .unwrap()
            .absolute()
            .unwrap()
            .bounds(),
        (0, i32::MAX)
    );
}
