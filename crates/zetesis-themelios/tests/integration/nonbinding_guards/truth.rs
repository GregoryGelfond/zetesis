//! Direct formulas for a one-atom count, independent of aggregate lowering.

use crate::support::frozen_truth::Truth;

fn compare(left: i32, relation: &str, right: i32) -> bool {
    match relation {
        "=" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        _ => panic!("the fixture supplies one of the six relations"),
    }
}

/// For the singleton count, the four numeric truth tables denote exactly
/// false, true, p, or not p. Default negation is applied afterwards.
pub(super) fn singleton(p: Truth, bound: i32, relation: &str, negations: usize) -> Truth {
    let mut result = match (compare(bound, relation, 0), compare(bound, relation, 1)) {
        (false, false) => Truth::constant(false),
        (true, true) => Truth::constant(true),
        (false, true) => p,
        (true, false) => p.negate(),
    };
    for _ in 0..negations {
        result = result.negate();
    }
    result
}
