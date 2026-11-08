use super::*;
use themelios_program::term::BinaryOp;
use zetesis_core::Value;

fn remainder(fixture: &mut Fixture, dividend: usize, divisor: usize) -> LiteralIr {
    let zero = fixture.scalar(&Value::Number(0), given_site());
    LiteralIr::Compare(fixture.comparison(
        vec![
            Operation::Variable(dividend),
            Operation::Variable(divisor),
            Operation::Binary(BinaryOp::Mod, 0, 1),
        ],
        Relation::Eq,
        vec![Operation::Constant(zero)],
        given_site(),
    ))
}

#[test]
fn tied_remainder_sources_bind_the_divisor_first() {
    for reverse in [false, true] {
        let mut fixture = Fixture::default();
        let mut literals = vec![
            atom(&mut fixture, "number", &[0]),
            atom(&mut fixture, "divisor", &[1]),
        ];
        if reverse {
            literals.reverse();
        }
        literals.push(less(&mut fixture, 1, 0));
        literals.push(remainder(&mut fixture, 0, 1));
        assert_eq!(
            order(&mut fixture, &literals, &[20, 20], 2),
            ["divisor", "number"]
        );
    }
}

#[test]
fn remainder_readiness_does_not_displace_a_smaller_source() {
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "number", &[0]),
        atom(&mut fixture, "divisor", &[1]),
        remainder(&mut fixture, 0, 1),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[2, 20], 2),
        ["number", "divisor"]
    );
}

#[test]
fn a_joint_binding_does_not_claim_prior_divisor_readiness() {
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "joint", &[0, 1]),
        atom(&mut fixture, "divisor", &[1]),
        remainder(&mut fixture, 0, 1),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[20, 20], 2),
        ["joint", "divisor"]
    );
}
