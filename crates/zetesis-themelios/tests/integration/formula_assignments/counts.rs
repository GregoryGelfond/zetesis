//! Complete count families retain full tuple identity through both consumers.

use super::{expected, input, models};
use crate::support::finite_bindings::{exhaustive, native};

#[test]
fn count_assignment_uses_full_tuple_identity() {
    let admitted = input(include_str!(
        "../../fixtures/aggregate-count/full-tuples.lp"
    ));
    assert_eq!(
        models(&admitted),
        expected(&["d(a).d(b).q(2).", "d(a).d(b).p.q(2)."])
    );
    assert_eq!(native(&admitted), exhaustive(&admitted));
}

#[test]
fn empty_tuple_is_one_optional_contribution() {
    let admitted = input(include_str!(
        "../../fixtures/aggregate-count/empty-tuples.lp"
    ));
    assert_eq!(models(&admitted), expected(&["q(1).z(0).", "p.q(1).z(0)."]));
    assert_eq!(native(&admitted), exhaustive(&admitted));
}
