//! Proposal carriers follow source dependencies under one immutable support.
mod restart;
use super::*;
use crate::formula_support::testing;

const INDEPENDENT: &str = "{p(1..3)}. q(A,B) :- A=#count{X:p(X)}, B=#count{X:p(X)}.";

fn with_cursor<T>(
    source: &str,
    conservative: bool,
    run: impl FnOnce(
        &mut Cursor<'_, '_>,
        &mut Evaluation,
        &mut Budget,
        Context<'_, &mut Computation<'_, '_>>,
    ) -> T,
) -> T {
    let mut result = None;
    testing::with_completed_source(source, |prepared, support, computation, counters| {
        let rule = prepared
            .rules
            .iter()
            .find(|rule| rule.bindings.is_some())
            .unwrap();
        let limits = FormulaLimits::default();
        let location = rule.location;
        let targets: Vec<_> = rule
            .bindings
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .filter(|step| step.produced < rule.body_variables)
            .map(|step| step.produced)
            .collect();
        let mut values = Binding::new(computation, &limits, counters, location).unwrap();
        values
            .extend_scope(
                rule.body_variables,
                computation,
                &limits,
                counters,
                location,
            )
            .unwrap();
        let mut cursor = Cursor::new(
            &rule.body,
            values,
            support,
            rule.bindings.as_ref(),
            0..rule.body_variables,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        if conservative {
            // For defined-only states, treating every predecessor as an input
            // reproduces the prior recomputation boundary. Do not use this
            // reference for unavailable inputs: required also governs their
            // propagation and would make independent proposals unavailable.
            for (index, generator) in cursor.generators.iter_mut().enumerate() {
                generator.required = Some(&targets[..index]);
            }
        }
        result = Some(run(
            &mut cursor,
            &mut Evaluation::default(),
            &mut testing::budget(),
            Context::new(computation, &limits, counters, location),
        ));
    });
    result.unwrap()
}

type Rows = Vec<Result<Vec<i32>, String>>;

fn rows(source: &str, conservative: bool) -> (Rows, u64) {
    with_cursor(
        source,
        conservative,
        |cursor, evaluation, budget, context| {
            let Context {
                computation,
                work:
                    GroundingWork {
                        limits,
                        counters,
                        location,
                    },
            } = context;
            let start = counters.accounting.work;
            let mut rows = Vec::new();
            loop {
                match cursor.next(evaluation, computation, limits, budget, counters, location) {
                    Ok(Some(binding)) => rows.push(Ok(cursor
                        .generators
                        .iter()
                        .map(|generator| {
                            let variable = target(generator.literal).unwrap();
                            let ValueNodeRef::Number(value) = binding
                                .read(variable, computation.read(), location)
                                .unwrap()
                                .descriptor()
                            else {
                                panic!("numeric proposal fixture")
                            };
                            value
                        })
                        .collect())),
                    Ok(None) => break,
                    Err(
                        error @ FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
                            ..
                        }),
                    ) => {
                        assert!(evaluation.zero_divisor());
                        rows.push(Err(error.to_string()));
                        cursor.reject(location).unwrap();
                    }
                    Err(error) => panic!("unexpected cursor failure: {error}"),
                }
            }
            (rows, counters.accounting.work - start)
        },
    )
}

#[test]
fn unrelated_proposals_reuse_their_carrier() {
    let (shared, shared_work) = rows(INDEPENDENT, false);
    let (prior, prior_work) = rows(INDEPENDENT, true);
    let expected: Rows = (0..4)
        .flat_map(|left| (0..4).map(move |right| Ok(vec![left, right])))
        .collect();
    assert_eq!(shared, expected);
    assert_eq!(shared, prior);
    assert!(shared_work < prior_work);
}

#[test]
fn changed_inputs_refresh_dependent_carriers() {
    for (source, expected) in [
        (
            "{a}. q(A,B) :- A=#count{1:a}, B=#sum{A:a}.",
            vec![vec![0, 0], vec![1, 0], vec![1, 1]],
        ),
        (
            "{a}. q(A,C,B) :- A=#count{1:a}, C=A+1, B=#sum{C:a}.",
            vec![vec![0, 1, 0], vec![0, 1, 1], vec![1, 2, 0], vec![1, 2, 2]],
        ),
    ] {
        let (shared, _) = rows(source, false);
        let expected: Rows = expected.into_iter().map(Ok).collect();
        assert_eq!(shared, expected, "{source}");
        assert_eq!(shared, rows(source, true).0, "{source}");
    }
}

#[test]
fn unavailable_inputs_discard_dependent_carriers() {
    let source = "{a}. q(A,C,B,D) :- A=#count{1:a;2:a}, C=1/(A-1), B=#sum{C:a}, D=#count{1:a}.";
    let preparation = testing::prepare(source);
    let location = preparation
        .program
        .rules
        .iter()
        .find(|rule| rule.bindings.is_some())
        .unwrap()
        .location;
    let undefined = crate::formula_support::undefined(location).to_string();
    // The independent D carrier still has two proposals while C and B are
    // unavailable. Both produce the established typed arithmetic refusal;
    // the following defined A branch must rebuild B from its new input.
    let expected = vec![
        Ok(vec![0, -1, -1, 0]),
        Ok(vec![0, -1, -1, 1]),
        Ok(vec![0, -1, 0, 0]),
        Ok(vec![0, -1, 0, 1]),
        Err(undefined.clone()),
        Err(undefined),
        Ok(vec![2, 1, 0, 0]),
        Ok(vec![2, 1, 0, 1]),
        Ok(vec![2, 1, 1, 0]),
        Ok(vec![2, 1, 1, 1]),
    ];
    assert_eq!(rows(source, false).0, expected);
}

#[test]
fn rewound_carriers_fit_their_existing_storage() {
    with_cursor(INDEPENDENT, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        let first = cursor
            .next(evaluation, computation, limits, budget, counters, location)
            .unwrap()
            .unwrap();
        let observer = computation.lease();
        let admitted =
            limits.max_support_bytes - computation.allowance(&observer, limits, location).unwrap();
        let bounded = FormulaLimits {
            max_support_bytes: admitted,
            ..*limits
        };
        drop(first);
        let retained = counters.workspace_bytes();
        let mut count = 1;
        while let Some(row) = cursor
            .next(
                evaluation,
                computation,
                &bounded,
                budget,
                counters,
                location,
            )
            .unwrap()
        {
            count += 1;
            drop(row);
            assert_eq!(counters.workspace_bytes(), retained);
        }
        assert_eq!(count, 16);
        assert_eq!(cursor.value_frames, 2);
    });
}

#[test]
fn unplanned_consumers_are_invalidated() {
    with_cursor(INDEPENDENT, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        drop(
            cursor
                .next(evaluation, computation, limits, budget, counters, location)
                .unwrap()
                .unwrap(),
        );
        assert!(matches!(cursor.states[1], State::Values { .. }));
        cursor.generators[1].required = None;
        cursor.depth = 0;
        cursor
            .invalidate_dependents(
                target(cursor.generators[0].literal).unwrap(),
                limits,
                counters,
                location,
            )
            .unwrap();
        assert!(matches!(cursor.states[1], State::Fresh));
        assert_eq!(cursor.value_frames, 1);
    });
}

#[test]
fn changed_relational_inputs_refresh_proposals() {
    testing::with_completed_source(
        "d(0;1). {a}. q(X,N) :- d(X), N=#sum{X:a}.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules
                .iter()
                .find(|rule| rule.bindings.is_some())
                .unwrap();
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let mut join = crate::formula_support::Join::rule(
                rule,
                support,
                computation,
                &limits,
                &mut budget,
                counters,
            )
            .unwrap();
            let mut actual = Vec::new();
            while let Some(binding) = join
                .next(computation, &limits, &mut budget, counters, rule.location)
                .unwrap()
            {
                actual.push(
                    (0..rule.body_variables)
                        .map(|slot| {
                            let ValueNodeRef::Number(value) = binding
                                .read(slot, computation.read(), rule.location)
                                .unwrap()
                                .descriptor()
                            else {
                                panic!("numeric relational fixture")
                            };
                            value
                        })
                        .collect::<Vec<_>>(),
                );
            }
            actual.sort();
            assert_eq!(actual, [vec![0, 0], vec![1, 0], vec![1, 1]]);
        },
    );
}

#[test]
fn support_rounds_recompute_proposal_carriers() {
    // n(1) first becomes possible after p(1); only then can p(2) be added.
    // A carrier retained beyond its snapshot would miss that second witness.
    let (actual, _) = rows("p(1). p(2) :- n(1). n(N) :- N=#count{X:p(X)}.", false);
    assert_eq!(actual, [Ok(vec![0]), Ok(vec![1]), Ok(vec![2])]);
}
