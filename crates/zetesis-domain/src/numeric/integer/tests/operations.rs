//! New certificates agree with checked scalar arithmetic over their whole domain.

use super::*;

fn binary_range(operator: BinaryOp, left: (i32, i32), right: (i32, i32)) -> Option<Range> {
    let (result, id) = analyze_ranges(Limits::default(), &[left, right], |input, slots| {
        Ok(vec![input.expression(Expression::Binary(
            operator, slots[0], slots[1],
        ))?])
    });
    result
        .rule(id)
        .unwrap()
        .integer(0)
        .map(crate::numeric::IntegerCertificate::range)
}

#[test]
fn variable_products_and_quotients_cover_checked_results() {
    // Small complete intervals include both signs, zero and representation edges.
    let intervals = [
        (i32::MIN, i32::MIN + 2),
        (-9, -2),
        (-1, -1),
        (-2, 3),
        (0, 0),
        (1, 1),
        (2, 7),
        (i32::MAX - 2, i32::MAX),
    ];
    for operator in [BinaryOp::Mul, BinaryOp::Div] {
        for left in intervals {
            for right in intervals {
                let certified = binary_range(operator, left, right);
                let mut total = true;
                for a in left.0..=left.1 {
                    for b in right.0..=right.1 {
                        let checked = if operator == BinaryOp::Mul {
                            a.checked_mul(b)
                        } else {
                            a.checked_div(b)
                        };
                        total &= checked.is_some();
                        if let Some(range) = certified {
                            let value = checked.expect("a certified operation is defined");
                            let (low, high) = range.bounds();
                            assert!((low..=high).contains(&value));
                        }
                    }
                }
                assert_eq!(
                    certified.is_some(),
                    total,
                    "{operator:?} over {left:?} and {right:?}"
                );
            }
        }
    }
}

#[test]
fn absolute_certificates_cover_checked_results() {
    for bounds in [
        (i32::MIN, i32::MIN + 2),
        (i32::MIN + 1, i32::MIN + 3),
        (-9, -2),
        (-4, 5),
        (0, 0),
        (2, 9),
        (i32::MAX - 2, i32::MAX),
    ] {
        let (result, id) = analyze_ranges(Limits::default(), &[bounds], |input, slots| {
            Ok(vec![input.expression(Expression::Absolute(slots[0]))?])
        });
        let certificate = result.rule(id).unwrap().integer(0);
        assert_eq!(certificate.is_some(), bounds.0 != i32::MIN);
        if let Some(certificate) = certificate {
            let (low, high) = certificate.range().bounds();
            for value in bounds.0..=bounds.1 {
                assert!((low..=high).contains(&value.checked_abs().unwrap()));
            }
        }
    }
}

#[test]
fn fixed_power_certificates_cover_checked_results() {
    // The exponent comes from a slot's complete domain, not a syntactic constant.
    for base in [
        (i32::MIN, i32::MIN + 1),
        (-9, -2),
        (-4, 5),
        (0, 0),
        (1, 1),
        (2, 9),
        (i32::MAX - 1, i32::MAX),
    ] {
        for exponent in [0, 1, 2, 3, 8, 31, i32::MAX] {
            let certificate = binary_range(BinaryOp::Pow, base, (exponent, exponent));
            let mut total = true;
            for value in base.0..=base.1 {
                let checked = value.checked_pow(u32::try_from(exponent).unwrap());
                total &= checked.is_some();
                if let Some(range) = certificate {
                    let (low, high) = range.bounds();
                    assert!((low..=high).contains(&checked.expect("certified power")));
                }
            }
            assert_eq!(certificate.is_some(), total, "{base:?} ** {exponent}");
        }
    }
}

#[test]
fn negative_or_varying_exponents_decline() {
    for exponent in [(-3, -3), (-1, 0), (0, 1), (2, 3)] {
        for base in [(0, 0), (1, 1), (2, 5)] {
            assert!(binary_range(BinaryOp::Pow, base, exponent).is_none());
        }
    }
}

#[test]
fn constant_results_do_not_hide_failed_intermediates() {
    let (result, id) = analyze_ranges(
        Limits::default(),
        &[(i32::MIN, i32::MIN), (-1, -1)],
        |input, slots| {
            let zero = number(input, 0)?;
            let quotient =
                input.expression(Expression::Binary(BinaryOp::Div, slots[0], slots[1]))?;
            let absolute = input.expression(Expression::Absolute(slots[0]))?;
            let product =
                input.expression(Expression::Binary(BinaryOp::Mul, slots[0], slots[1]))?;
            let mut requests = Vec::new();
            for child in [quotient, absolute, product] {
                requests.push(input.expression(Expression::Binary(BinaryOp::Pow, child, zero))?);
                requests.push(input.expression(Expression::Binary(BinaryOp::Mul, zero, child))?);
                requests.push(input.expression(Expression::Binary(BinaryOp::Sub, child, child))?);
            }
            Ok(requests)
        },
    );
    let rule = result.rule(id).unwrap();
    assert_eq!(rule.integer_query_count(), 9);
    for query in 0..rule.integer_query_count() {
        assert!(rule.integer(query).is_none(), "failed intermediate {query}");
    }
}

#[test]
fn a_zero_power_does_not_certify_a_symbolic_base() {
    let (result, id) = analyze_ranges(Limits::default(), &[], |input, _| {
        let symbol = input.constant(Key::Symbol("a"))?;
        let symbol = input.expression(Expression::Constant(symbol))?;
        let zero = number(input, 0)?;
        Ok(vec![input.expression(Expression::Binary(
            BinaryOp::Pow,
            symbol,
            zero,
        ))?])
    });
    assert!(result.rule(id).unwrap().integer(0).is_none());
}

#[test]
fn an_exponent_guard_does_not_certify_its_original_domain() {
    let mut id = None;
    let result = crate::numeric::analyze(&Reader, Limits::default(), |input| {
        let predicate = input.predicate(1)?;
        let argument = input.argument(predicate, 0)?;
        input.seed_interval(argument, -1, 1)?;
        let exponent = input.expression(Expression::Slot(0))?;
        let zero = number(input, 0)?;
        let one = number(input, 1)?;
        let power = input.expression(Expression::Binary(BinaryOp::Pow, one, exponent))?;
        id = Some(input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive { slot: 0, argument }],
            guards: vec![Guard::Compare {
                left: exponent,
                relation: Relation::Eq,
                right: zero,
            }],
            integer_queries: vec![power],
            ..Rule::default()
        })?);
        Ok(())
    })
    .unwrap();
    let rule = result.rule(id.unwrap()).unwrap();
    assert_eq!(
        rule.produced()[0].interval,
        Interval::Bounded { low: 0, high: 0 }
    );
    assert!(rule.integer(0).is_none());
}
