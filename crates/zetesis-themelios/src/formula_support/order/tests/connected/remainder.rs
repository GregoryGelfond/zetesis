use super::*;

fn fixture(divisors: &[i32], empty: bool, overflow: bool) -> (Fixture, Vec<LiteralIr>) {
    let atoms = [row("n", &[2]), row("n", &[4])]
        .into_iter()
        .chain(divisors.iter().map(|&d| row("d", &[d])))
        .chain((!empty).then(|| row("last", &[1])));
    let mut fixture = Fixture::from_atoms(atoms, given_site());
    let zero = fixture.scalar(&Value::Number(0), given_site());
    let one = fixture.scalar(&Value::Number(1), given_site());
    let largest = fixture.scalar(&Value::Number(i32::MAX), given_site());
    let mut literals = vec![
        atom(&mut fixture, "n", &[0]),
        atom(&mut fixture, "d", &[1]),
        atom(&mut fixture, "last", &[2]),
        LiteralIr::Compare(fixture.comparison(
            vec![
                Operation::Variable(0),
                Operation::Variable(1),
                Operation::Binary(BinaryOp::Mod, 0, 1),
            ],
            Relation::Eq,
            vec![Operation::Constant(zero)],
            given_site(),
        )),
    ];
    if overflow {
        literals.push(LiteralIr::Compare(fixture.comparison(
            vec![
                Operation::Variable(0),
                Operation::Constant(largest),
                Operation::Binary(BinaryOp::Add, 0, 1),
                Operation::Constant(one),
                Operation::Binary(BinaryOp::Div, 2, 3),
            ],
            Relation::Gt,
            vec![Operation::Constant(zero)],
            given_site(),
        )));
    }
    (fixture, literals)
}

#[test]
fn reordered_remainders_keep_undefined_complete_families() {
    let (mut fixture, literals) = fixture(&[0], false, false);
    for permutation in [None, Some(&[0, 1, 2][..]), Some(&[1, 0, 2][..])] {
        assert!(matches!(
            execute(&mut fixture, &literals, 3, permutation).result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Undefined,
                    ..
                }
            ))
        ));
    }
}

#[test]
fn empty_later_relations_do_not_publish_prefix_arithmetic_errors() {
    let (mut fixture, literals) = fixture(&[0], true, true);
    for permutation in [None, Some(&[0, 1, 2][..]), Some(&[1, 0, 2][..])] {
        assert!(
            execute(&mut fixture, &literals, 3, permutation)
                .result
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn reordered_remainders_keep_independent_retained_overflow() {
    let (mut fixture, literals) = fixture(&[2, 4], false, true);
    for permutation in [None, Some(&[0, 1, 2][..]), Some(&[1, 0, 2][..])] {
        assert!(matches!(
            execute(&mut fixture, &literals, 3, permutation).result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Overflow,
                    ..
                }
            ))
        ));
    }
}
