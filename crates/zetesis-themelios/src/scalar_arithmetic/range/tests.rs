use super::*;

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
                            result.low <= i64::from(actual) && i64::from(actual) <= result.high,
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
