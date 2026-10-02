//! Producer-local conjunctions retain all alternatives from other producers.

use std::collections::BTreeSet;
use std::fmt::Write;

use super::{numbers, source};
use crate::support::signatures::signature;
use zetesis_domain::{Domain, Limits, Resource, Status, Widening, analyze};

#[test]
fn repeated_input_visits_are_charged_per_value() {
    let work = |width, occurrences| {
        let mut text = String::new();
        for value in 0..width {
            write!(text, "d({value},0).").unwrap();
        }
        text.push_str("out(X):-");
        for occurrence in 0..occurrences {
            if occurrence != 0 {
                text.push(',');
            }
            // Distinct constants keep these body occurrences distinct in the
            // program layer while each X reads the same predicate column.
            write!(text, "d(X,{occurrence})").unwrap();
        }
        text.push('.');
        let program = source(&text);
        let result = analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::FixedPoint);
        result.statistics().work
    };
    // Repeated columns still incur metadata visits for every candidate value,
    // even though iteration establishes their set membership without a probe.
    let narrow_overhead = work(1, 4) - work(1, 1);
    let wide_overhead = work(8, 4) - work(8, 1);
    assert!(wide_overhead > narrow_overhead);
}

#[test]
fn each_producer_intersects_its_own_bindings() {
    let program = source(
        "left(1).left(2).middle(2).middle(3).right(3).right(4).\
         out(X):-left(X),middle(X).out(X):-middle(X),right(X).out(7).",
    );
    let result = analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2, 3, 7]));
}

#[test]
fn repeated_body_positions_intersect_argument_domains() {
    let program = source("pair(1,2).pair(2,3).out(X):-pair(X,X).");
    let result = analyze(&program, Limits::default());
    // The columns overlap at 2 although no complete pair has equal fields.
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
}

#[test]
fn intersections_preserve_typed_symbol_identity() {
    let program = source(
        "left(1).left(\"1\").left(a).left(f(1)).left((1,a)).\
         right(\"1\").right(f(1)).right((1,a)).right(f(2)).\
         expected(\"1\").expected(f(1)).expected((1,a)).\
         out(X):-left(X),right(X).",
    );
    let result = analyze(&program, Limits::default());
    let Domain::Finite(expected) = result.domain(&signature("expected", 1), 0) else {
        panic!("closed symbolic fixtures must stay finite");
    };
    assert_eq!(expected.len(), 3);
    assert_eq!(
        result.domain(&signature("out", 1), 0),
        result.domain(&signature("expected", 1), 0)
    );
}

#[test]
fn unknown_dependencies_preserve_other_finite_bounds() {
    for body in ["open(X),finite(X)", "finite(X),open(X)"] {
        let program = source(&format!("{{open(1)}}.finite(2).out(X):-{body}."));
        let result = analyze(&program, Limits::default());
        assert_eq!(result.domain(&signature("open", 1), 0), &Domain::Unknown);
        assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
        assert_eq!(
            result.argument(&signature("out", 1), 0).unwrap().widening(),
            None
        );
    }
}

#[test]
fn an_empty_finite_dependency_bounds_an_unknown_one() {
    for body in ["open(X),empty(X)", "empty(X),open(X)"] {
        let program = source(&format!("{{open(1)}}.empty(X):-empty(X).out(X):-{body}."));
        let result = analyze(&program, Limits::default());
        assert!(numbers(&result, "out", 0, 1).is_empty());
    }
}

#[test]
fn all_unknown_dependencies_widen_their_output() {
    let program = source("out(X):-left(X),right(X).{left(1)}.{right(2)}.");
    let result = analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(result.domain(&signature("out", 1), 0), &Domain::Unknown);
    assert_eq!(
        result.argument(&signature("out", 1), 0).unwrap().widening(),
        Some(Widening::Dependency)
    );
}

#[test]
fn width_widening_leaves_independent_binding_bounds_usable() {
    let program = source("out(X):-wide(X),finite(X).wide(1).wide(2).finite(2).");
    let result = analyze(
        &program,
        Limits {
            max_values_per_argument: 1,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(
        result
            .argument(&signature("wide", 1), 0)
            .unwrap()
            .widening(),
        Some(Widening::ValueWidth)
    );
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
}

#[test]
fn producer_union_still_widens_when_its_width_is_exceeded() {
    let program = source("left(1).right(2).out(X):-left(X).out(X):-right(X).");
    let result = analyze(
        &program,
        Limits {
            max_values_per_argument: 1,
            ..Limits::default()
        },
    );
    assert_eq!(
        result.argument(&signature("out", 1), 0).unwrap().widening(),
        Some(Widening::ValueWidth)
    );
}

#[test]
fn recursive_intersections_reach_the_same_source_order_fixed_point() {
    for text in [
        "out(X):-left(X),right(X).left(X):-out(X).right(X):-seed(X).seed(2).left(1).left(2).right(3).",
        "right(3).left(2).left(1).seed(2).right(X):-seed(X).left(X):-out(X).out(X):-left(X),right(X).",
    ] {
        let program = source(text);
        let result = analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::FixedPoint);
        assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
        assert_eq!(numbers(&result, "left", 0, 1), BTreeSet::from([1, 2]));
        assert_eq!(numbers(&result, "right", 0, 1), BTreeSet::from([2, 3]));
    }
}

#[test]
fn arithmetic_conditions_remain_unevaluated() {
    let program = source(
        "left(1).left(2).right(2).right(3).\
         out(X):-left(X),right(X),X/(X-X)=0.",
    );
    let result = analyze(&program, Limits::default());
    // Source evaluation would divide by zero; domain analysis only uses binders.
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
}

#[test]
fn negative_conditions_do_not_narrow_argument_domains() {
    let program = source(
        "left(1).left(2).right(2).right(3).blocked(2).\
         out(X):-left(X),right(X),not blocked(X).",
    );
    let result = analyze(&program, Limits::default());
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([2]));
}

#[test]
fn every_intersection_work_cutoff_discards_partial_domains() {
    let program = source("left(1).left(2).right(2).right(3).out(X):-left(X),right(X).");
    let complete = analyze(&program, Limits::default());
    assert_eq!(complete.status(), Status::FixedPoint);
    let work = complete.statistics().work;
    for max_work in 0..work {
        let result = analyze(
            &program,
            Limits {
                max_work,
                ..Limits::default()
            },
        );
        assert!(
            matches!(result.status(), Status::Stopped(stop) if stop.resource == Resource::Work)
        );
        assert!(result.statistics().work <= max_work);
        assert_eq!(result.arguments().count(), 0);
        assert_eq!(result.domain(&signature("out", 1), 0), &Domain::Unknown);
    }
    let exact = analyze(
        &program,
        Limits {
            max_work: work,
            ..Limits::default()
        },
    );
    assert_eq!(exact.status(), Status::FixedPoint);
    assert_eq!(numbers(&exact, "out", 0, 1), BTreeSet::from([2]));
}
