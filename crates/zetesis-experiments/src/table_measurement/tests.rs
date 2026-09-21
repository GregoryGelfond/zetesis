use std::io;

use zetesis_cpu::Cancellation;

use super::*;

fn config(case: Case, rows: usize) -> Configuration {
    Configuration {
        case,
        rows,
        queries: 7,
        workers: 4,
        warmups: 0,
        repetitions: 1,
        max_table_bytes: config::MAX_RETAINED_BYTES,
        max_table_work: config::MAX_WORK,
    }
}

#[test]
fn complete_domain_projections_agree_at_word_boundaries() {
    for case in [Case::Correlated, Case::Independent, Case::Aliased] {
        for rows in [1, 31, 32, 33, 65] {
            let mut observations = 0;
            assert!(
                measure(config(case, rows), &Cancellation::default(), |event| {
                    if let Event::Batch { outcomes, .. } = event {
                        assert_eq!(outcomes.len(), 7);
                        assert!(
                            outcomes
                                .iter()
                                .all(|result| matches!(result, Outcome::Complete { .. }))
                        );
                        observations += 1;
                    }
                    Ok(())
                })
                .unwrap()
            );
            assert_eq!(observations, 6);
        }
    }
}

#[test]
fn restoration_recovers_the_original_projection() {
    let mut restored = 0;
    measure(
        config(Case::Aliased, 65),
        &Cancellation::default(),
        |event| {
            if let Event::Batch { outcomes, .. } = event {
                let output = |index| match &outcomes[index] {
                    Outcome::Complete { output, .. } => output,
                    Outcome::Refused { .. } => panic!("fixture is admitted"),
                };
                assert_eq!(output(0), output(6));
                assert!(output(4).rows.is_empty());
                restored += 1;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(restored, 6);
}

#[test]
fn query_order_is_independent_of_pool_width() {
    let collect = |workers| {
        let mut config = config(Case::Independent, 33);
        config.workers = workers;
        config.queries = 19;
        let mut results = Vec::new();
        measure(config, &Cancellation::default(), |event| {
            if let Event::Batch {
                route: Route::Rayon,
                outcomes,
                ..
            } = event
            {
                for (position, outcome) in outcomes.iter().enumerate() {
                    if let Outcome::Complete { query, output, .. } = outcome {
                        assert_eq!(*query, position);
                        results.push(output.clone());
                    } else {
                        panic!("fixture is admitted");
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        results
    };
    assert_eq!(collect(1), collect(4));
}

#[test]
fn preparation_refusal_keeps_scan_evidence() {
    let mut request = config(Case::Correlated, 33);
    request.max_table_bytes = 0;
    let mut refused = 0;
    let mut scanned = 0;
    let passed = measure(request, &Cancellation::default(), |event| {
        match event {
            Event::PreparationRefused { failure } => {
                assert!(matches!(
                    failure.cause,
                    zetesis_cpu::table::Cause::Limit { .. }
                ));
                refused += 1;
            }
            Event::Batch { route, .. } => {
                assert_eq!(*route, Route::Scan);
                scanned += 1;
            }
            Event::Complete { passed, .. } => assert!(!passed),
            _ => {}
        }
        Ok(())
    })
    .unwrap();
    assert!(!passed);
    assert_eq!((refused, scanned), (1, 2));
}

#[test]
fn query_refusals_preserve_the_remaining_schedule() {
    let mut request = config(Case::Correlated, 1);
    let mut preparation_work = None;
    measure(request, &Cancellation::default(), |event| {
        if let Event::Subject { preparation, .. } = event {
            preparation_work = Some(preparation.table.unwrap().work);
        }
        Ok(())
    })
    .unwrap();
    request.max_table_work = preparation_work.unwrap();
    let mut refused = 0;
    let mut complete = 0;
    assert!(
        !measure(request, &Cancellation::default(), |event| {
            if let Event::Batch {
                outcomes, route, ..
            } = event
                && *route != Route::Scan
            {
                for outcome in *outcomes {
                    match outcome {
                        Outcome::Refused { failure, .. } => {
                            assert!(failure.work <= request.max_table_work);
                            refused += 1;
                        }
                        Outcome::Complete { .. } => complete += 1,
                    }
                }
            }
            Ok(())
        })
        .unwrap()
    );
    assert!(refused > 0);
    assert!(complete > 0);
    assert_eq!(refused + complete, 28);
}

#[test]
fn cancelled_measurement_does_not_claim_completion() {
    let cancellation = Cancellation::default();
    let mut published = 0;
    let result = measure(config(Case::Correlated, 33), &cancellation, |event| {
        published += 1;
        if matches!(event, Event::Subject { .. }) {
            cancellation.cancel();
        }
        assert!(!matches!(event, Event::Complete { .. }));
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Error::Stopped(zetesis_cpu::Stop::Cancelled))
    ));
    assert_eq!(published, 2);
}

#[test]
fn output_failure_preserves_its_original_cause() {
    let result = measure(
        config(Case::Correlated, 1),
        &Cancellation::default(),
        |_| {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "closed measurement sink",
            ))
        },
    );
    assert!(
        matches!(result, Err(Error::Output(error)) if error.kind() == io::ErrorKind::BrokenPipe)
    );
}

#[test]
fn invalid_scope_is_refused_before_publication() {
    let mut request = config(Case::Independent, config::MAX_ROWS + 1);
    let mut events = 0;
    assert!(matches!(
        measure(request, &Cancellation::default(), |_| {
            events += 1;
            Ok(())
        }),
        Err(Error::Configuration(_))
    ));
    request.rows = 1;
    request.queries = config::MAX_QUERIES + 1;
    assert!(matches!(
        measure(request, &Cancellation::default(), |_| {
            events += 1;
            Ok(())
        }),
        Err(Error::Configuration(_))
    ));
    assert_eq!(events, 0);
}

#[test]
fn subject_identifies_typed_values_and_row_occurrences() {
    let fixture =
        fixture::Fixture::new(config(Case::Independent, 33), &Cancellation::default()).unwrap();
    let subject = serde_json::to_value(fixture.subject(Case::Independent).unwrap()).unwrap();
    let values = subject["values"].as_array().unwrap();
    assert!(
        values
            .iter()
            .any(|value| value["kind"] == "number" && value["value"] == 1)
    );
    assert!(
        values
            .iter()
            .any(|value| value["kind"] == "string" && value["value"] == "1")
    );
    assert!(
        values
            .iter()
            .any(|value| value["kind"] == "unary-tuple" && value["value"] == 1)
    );
    assert_eq!(subject["original_indices"][0], 32);
    assert_eq!(subject["original_indices"][1], 32);
}

#[test]
fn report_capacity_refuses_an_unpublished_suffix() {
    use io::Write as _;
    let mut output = Vec::new();
    let mut writer = view::BoundedWriter::new(&mut output, 3);
    writer.write_all(b"abc").unwrap();
    assert!(writer.write_all(b"d").is_err());
    assert_eq!(output, b"abc");
}
