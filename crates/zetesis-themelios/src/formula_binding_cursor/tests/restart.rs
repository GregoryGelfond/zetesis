//! Exhausted cursors retain only carriers identified by present equal inputs.
use super::*;
use zetesis_core::Value;

const RELATIONAL: &str = "d(1;2). {a}. q(K,N) :- d(K), N=#sum{K:a}.";

fn input(
    slots: usize,
    key: i32,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
) -> Binding<'static> {
    let mut values = vec![None; slots];
    values[0] = Some(Value::Number(key));
    testing::binding(&values, computation, counters, location)
}

fn collect(
    cursor: &mut Cursor<'_, '_>,
    evaluation: &mut Evaluation,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, '_>>,
) -> Rows {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut result = Vec::new();
    loop {
        match cursor.next(evaluation, computation, limits, budget, counters, location) {
            Ok(Some(binding)) => result.push(Ok((0..binding.len())
                .map(|slot| {
                    let ValueNodeRef::Number(value) = binding
                        .read(slot, computation.read(), location)
                        .unwrap()
                        .descriptor()
                    else {
                        panic!("numeric relational fixture")
                    };
                    value
                })
                .collect())),
            Ok(None) => return result,
            Err(error @ FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })) => {
                assert!(evaluation.zero_divisor());
                result.push(Err(error.to_string()));
                cursor.reject(location).unwrap();
            }
            Err(error) => panic!("unexpected cursor failure: {error}"),
        }
    }
}

fn relational_rows(source: &str, keys: &[i32], fresh: bool) -> (Vec<Rows>, Vec<usize>, u64) {
    with_cursor(source, false, |cursor, evaluation, budget, context| {
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
        let mut carriers = Vec::new();
        for (index, &key) in keys.iter().enumerate() {
            let values = input(cursor.values.len(), key, computation, counters, location);
            if index == 0 {
                cursor.values = values;
            } else {
                cursor
                    .restart(
                        values,
                        Context::new(computation, limits, counters, location),
                    )
                    .unwrap();
            }
            carriers.push(cursor.value_frames);
            if fresh {
                // Recompute at the old row boundary with the original required
                // inputs. Unlike broadening required, this also preserves the
                // established propagation of unavailable arithmetic inputs.
                for depth in 0..cursor.states.len() {
                    cursor.state_at(depth, State::Fresh, location).unwrap();
                }
            }
            rows.push(collect(
                cursor,
                evaluation,
                budget,
                Context::new(computation, limits, counters, location),
            ));
        }
        (rows, carriers, counters.accounting.work - start)
    })
}

#[test]
fn equal_relational_inputs_reuse_their_carrier() {
    let keys = [1, 1, 2, 1, 1];
    let (shared, carriers, work) = relational_rows(RELATIONAL, &keys, false);
    let (fresh, _, fresh_work) = relational_rows(RELATIONAL, &keys, true);
    let expected: Vec<Rows> = keys
        .iter()
        .map(|&key| vec![Ok(vec![key, 0]), Ok(vec![key, key])])
        .collect();
    assert_eq!(shared, expected);
    assert_eq!(shared, fresh);
    assert_eq!(carriers, [0, 1, 0, 0, 1]);
    assert!(work < fresh_work);
}

#[test]
fn generated_inputs_expire_at_relational_restart() {
    let source = "d(1;2). {a}. q(K,A,B) :- d(K), A=#count{1:a}, B=#sum{A,K:a}.";
    let keys = [1, 1, 2, 2];
    let (shared, carriers, _) = relational_rows(source, &keys, false);
    let (fresh, _, _) = relational_rows(source, &keys, true);
    let expected: Vec<Rows> = keys
        .iter()
        .map(|&key| {
            vec![
                Ok(vec![key, 0, 0]),
                Ok(vec![key, 1, 0]),
                Ok(vec![key, 1, 1]),
            ]
        })
        .collect();
    assert_eq!(shared, expected);
    assert_eq!(shared, fresh);
    // A has no relational inputs. B reads generated A, which is absent from
    // an exhausted frame, even when the next relational K is identical.
    assert_eq!(carriers, [0, 1, 1, 1]);
}

#[test]
fn relational_restart_preserves_arithmetic_refusals() {
    let source = "d(0;1;2). {a}. q(K,C,N,D) :- d(K), C=1/(K-1), N=#sum{C:a}, D=#count{1:a}.";
    let keys = [0, 0, 1, 1, 2, 2, 0];
    let (shared, _, _) = relational_rows(source, &keys, false);
    let (fresh, _, _) = relational_rows(source, &keys, true);
    assert_eq!(shared, fresh);
    let preparation = testing::prepare(source);
    let rule = preparation
        .program
        .rules
        .iter()
        .find(|rule| rule.bindings.is_some())
        .unwrap();
    // The normalized body orders count before sum; the existing plan runs C,
    // then independent D, then N. Keep that proposal order in the reference.
    assert_eq!(
        rule.bindings
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .map(|step| step.produced)
            .collect::<Vec<_>>(),
        [1, 3, 2],
    );
    let location = rule.location;
    let undefined = crate::formula_support::undefined(location).to_string();
    let expected: Vec<Rows> = keys
        .iter()
        .map(|&key| match key {
            0 => vec![
                Ok(vec![0, -1, -1, 0]),
                Ok(vec![0, -1, 0, 0]),
                Ok(vec![0, -1, -1, 1]),
                Ok(vec![0, -1, 0, 1]),
            ],
            1 => vec![Err(undefined.clone()), Err(undefined.clone())],
            2 => vec![
                Ok(vec![2, 1, 0, 0]),
                Ok(vec![2, 1, 1, 0]),
                Ok(vec![2, 1, 0, 1]),
                Ok(vec![2, 1, 1, 1]),
            ],
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(shared, expected);
}

#[test]
fn unplanned_inputs_expire_at_relational_restart() {
    with_cursor(RELATIONAL, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        cursor.values = input(cursor.values.len(), 1, computation, counters, location);
        collect(
            cursor,
            evaluation,
            budget,
            Context::new(computation, limits, counters, location),
        );
        assert_eq!(cursor.value_frames, 1);
        cursor.generators[0].required = None;
        let values = input(cursor.values.len(), 1, computation, counters, location);
        cursor
            .restart(
                values,
                Context::new(computation, limits, counters, location),
            )
            .unwrap();
        assert_eq!(cursor.value_frames, 0);
    });
}

#[test]
fn absent_relational_inputs_expire_their_carrier() {
    with_cursor(RELATIONAL, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        cursor.values = input(cursor.values.len(), 1, computation, counters, location);
        collect(
            cursor,
            evaluation,
            budget,
            Context::new(computation, limits, counters, location),
        );
        assert_eq!(cursor.value_frames, 1);
        let mut values = input(cursor.values.len(), 1, computation, counters, location);
        values.clear(0, limits, counters, location).unwrap();
        cursor
            .restart(
                values,
                Context::new(computation, limits, counters, location),
            )
            .unwrap();
        assert_eq!(cursor.value_frames, 0);
    });
}

#[test]
fn relational_restart_accounts_for_both_live_frames() {
    with_cursor(RELATIONAL, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        cursor.values = input(cursor.values.len(), 1, computation, counters, location);
        collect(
            cursor,
            evaluation,
            budget,
            Context::new(computation, limits, counters, location),
        );
        let before = counters.workspace_bytes();
        let values = input(cursor.values.len(), 1, computation, counters, location);
        let observer = computation.lease();
        let admitted =
            limits.max_support_bytes - computation.allowance(&observer, limits, location).unwrap();
        let short = FormulaLimits {
            max_support_bytes: admitted - 1,
            ..*limits
        };
        assert!(matches!(
            cursor.restart(values, Context::new(computation, &short, counters, location)),
            Err(FormulaFailure::Limit { resource: FormulaResource::SupportBytes, observed, limit, .. })
                if observed == admitted as u128 && limit == (admitted - 1) as u128
        ));
        assert!(cursor.finished);
        assert_eq!(cursor.value_frames, 1);
        assert_eq!(counters.workspace_bytes(), before);
        let values = input(cursor.values.len(), 1, computation, counters, location);
        let exact = FormulaLimits {
            max_support_bytes: admitted,
            ..*limits
        };
        cursor
            .restart(
                values,
                Context::new(computation, &exact, counters, location),
            )
            .unwrap();
        assert_eq!(cursor.value_frames, 1);
        // Moving in the next frame drops the old frame; the retained carrier
        // stays in its existing owner and cannot be charged a second time.
        assert_eq!(counters.workspace_bytes(), before);
        assert_eq!(
            collect(
                cursor,
                evaluation,
                budget,
                Context::new(computation, limits, counters, location)
            ),
            [Ok(vec![1, 0]), Ok(vec![1, 1])]
        );
    });
}

#[test]
fn relational_restart_observes_work_refusal() {
    with_cursor(RELATIONAL, false, |cursor, evaluation, budget, context| {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        cursor.values = input(cursor.values.len(), 1, computation, counters, location);
        collect(
            cursor,
            evaluation,
            budget,
            Context::new(computation, limits, counters, location),
        );
        let values = input(cursor.values.len(), 1, computation, counters, location);
        let short = FormulaLimits {
            max_work: counters.accounting.work,
            ..*limits
        };
        assert!(matches!(
            cursor.restart(values, Context::new(computation, &short, counters, location)),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. })
                if observed > limit && limit == u128::from(short.max_work)
        ));
        assert!(cursor.finished);
        assert_eq!(cursor.value_frames, 1);
    });
}
