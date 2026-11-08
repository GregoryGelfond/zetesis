use super::*;
use crate::FormulaLimits;
use crate::formula_support::{Evaluation, testing};
use crate::test_support::given_site;
use themelios_program::term::BinaryOp;
use zetesis_core::{Value, ValueNodeRef};

#[test]
fn repeated_assignment_references_share_expression_nodes() {
    let depth = 16;
    let expressions: Vec<_> = (0..depth)
        .map(|slot| {
            let input = if slot == 0 { depth } else { slot - 1 };
            Expression::new(vec![
                Operation::Variable(input),
                Operation::Variable(input),
                Operation::Binary(BinaryOp::Add, 0, 1),
            ])
        })
        .collect();
    let definitions: Vec<_> = expressions
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
        .collect();
    let root = Expression::new(vec![Operation::Variable(depth - 1)]);
    let limits = FormulaLimits::default();
    testing::Fixture::default().with(given_site(), |_, computation, counters| {
        let expanded = expand(
            &root,
            &definitions,
            &mut Context::new(&*computation, &limits, counters, given_site()),
        )
        .unwrap()
        .unwrap();
        // A tree expansion has 2^depth leaves; the source DAG has two
        // source leaves followed by exactly one addition per assignment.
        assert_eq!(expanded.expression.nodes.len(), depth + 2);
        let mut values = vec![None; depth + 1];
        values[depth] = Some(Value::Number(1));
        let binding = testing::binding(&values, computation, counters, given_site());
        let result = Evaluation::default()
            .source_expression(
                &expanded.expression,
                |slot| binding.key(slot, given_site()),
                computation,
                &limits,
                counters,
                given_site(),
            )
            .unwrap();
        assert_eq!(
            computation.read().term(&result).unwrap().descriptor(),
            ValueNodeRef::Number(65536)
        );
    });
}

#[test]
fn assignment_cycles_decline_without_unbounded_traversal() {
    let left = Expression::new(vec![Operation::Variable(1)]);
    let right = Expression::new(vec![Operation::Variable(0)]);
    let root = Expression::new(vec![Operation::Variable(0)]);
    let limits = FormulaLimits::default();
    testing::Fixture::default().with(given_site(), |_, computation, counters| {
        assert!(
            expand(
                &root,
                &[Some(&left), Some(&right)],
                &mut Context::new(&*computation, &limits, counters, given_site())
            )
            .unwrap()
            .is_none()
        );
    });
}
