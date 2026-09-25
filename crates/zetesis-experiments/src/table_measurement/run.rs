use std::{io, mem::size_of, time::Instant};

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use zetesis_core::{Value, relation::Relation};
use zetesis_cpu::{
    Cancellation,
    table::{self, Table},
};

use super::{Configuration, Error, Event, Outcome, Output, Phase, Preparation, Route};
use super::{
    config::{
        DICTIONARY_VALUES, DOMAIN_VALUES, MAX_RETAINED_BYTES, MAX_SUBJECT_BYTES, MAX_VARIABLES,
    },
    fixture::Fixture,
    view::BoundedWriter,
};

/// Measure three complete row/domain procedures on one borrowed immutable table.
///
/// Each raw batch is validated against an independent original-row reference
/// outside its timer. Rayon retains original query order and shares one Table.
/// The table retains its support index across every narrow/replace/widen query.
/// Per-query times include preparation of that query's table masks; construction
/// of the reusable relation, table and scan-domain memberships is separate.
///
/// # Errors
/// Invalid configuration, acquisition/operational failure, cancellation, result
/// mismatch or sink failure stops with prior events intact. Finite table limits
/// publish typed refusals and yield `Ok(false)` after the executable schedule.
/// Output capacity is capped at 128 MiB per retained expected/actual batch; the
/// bounded CLI additionally caps the complete report at 64 MiB. Caller-retained
/// events, allocator metadata and thread stacks remain outside these limits.
pub fn measure(
    configuration: Configuration,
    cancellation: &Cancellation,
    mut emit: impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<bool, Error> {
    let configuration = configuration.validate()?;
    cancellation.poll().map_err(Error::Stopped)?;
    emit(&Event::Start {
        schema: 1,
        configuration,
    })
    .map_err(Error::Output)?;
    let started = Instant::now();
    let fixture = Fixture::new(configuration, cancellation)?;
    let fixture_ns = started.elapsed().as_nanos();
    let subject = fixture.subject(configuration.case)?;
    let mut bytes = Vec::new();
    serde_json::to_writer(
        &mut BoundedWriter::new(&mut bytes, MAX_SUBJECT_BYTES),
        &subject,
    )
    .map_err(|error| Error::Output(io::Error::other(error)))?;
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
    drop(bytes);
    let started = Instant::now();
    let expected = (0..configuration.queries)
        .map(|query| fixture.reference(query, cancellation))
        .collect::<Result<Vec<_>, _>>()?;
    let reference_ns = started.elapsed().as_nanos();
    let reference_bytes = expected.capacity() * size_of::<Output>()
        + expected.iter().map(Output::retained_bytes).sum::<usize>();
    capacity(reference_bytes)?;
    let started = Instant::now();
    let domains = prepared_domains(&fixture);
    let scan_domains_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let relation = Relation::from_catalog(
        &fixture.predicate,
        &fixture.atoms,
        &fixture.indices,
        zetesis_core::relation::Limits::default(),
    )
    .map_err(Error::Relation)?;
    let relation_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let table = match Table::prepare(
        &relation,
        &fixture.scope,
        limits(configuration),
        cancellation,
    ) {
        Ok(table) => Ok(table),
        Err(failure) => Err(applicability(failure)?),
    };
    let table_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(configuration.workers)
        .build()
        .map_err(Error::Pool)?;
    let pool_ns = started.elapsed().as_nanos();
    let preparation = Preparation {
        rows: configuration.rows,
        coherent_rows: expected[0].rows.len(),
        row_words: configuration.rows.div_ceil(u32::BITS as usize),
        support_entries: table.as_ref().ok().map(Table::support_entries),
        fixture_ns,
        reference_ns,
        scan_domains_ns,
        relation_ns,
        table_ns,
        pool_ns,
        pool_workers: pool.current_num_threads(),
        fixture_bound_bytes: fixture.storage_bound,
        scan_domain_bytes: domain_bytes(&domains),
        relation_bytes: relation.storage().retained_bytes,
        relation_work: relation.storage().construction_work,
        reference_bytes,
        table: table.as_ref().ok().map(|table| table.statistics().into()),
    };
    emit(&Event::Subject {
        sha256: &fingerprint,
        subject: &subject,
        preparation: &preparation,
    })
    .map_err(Error::Output)?;
    if let Err(failure) = &table {
        emit(&Event::PreparationRefused { failure }).map_err(Error::Output)?;
    }
    let plan = Plan {
        fixture: &fixture,
        domains: &domains,
        table: table.as_ref().ok(),
        configuration,
        cancellation,
    };
    schedule(&plan, &expected, &fingerprint, &pool, &mut emit)
}

fn schedule(
    plan: &Plan<'_, '_, '_>,
    expected: &[Output],
    fingerprint: &str,
    pool: &rayon::ThreadPool,
    emit: &mut impl FnMut(&Event<'_>) -> io::Result<()>,
) -> Result<bool, Error> {
    let configuration = plan.configuration;
    let cancellation = plan.cancellation;
    let mut batches = 0;
    let mut passed = plan.table.is_some();
    for route in [Route::Scan, Route::Table, Route::Rayon] {
        if route != Route::Scan && plan.table.is_none() {
            continue;
        }
        for (phase, count) in [
            (Phase::Initial, 1),
            (Phase::Warmup, configuration.warmups),
            (Phase::Timed, configuration.repetitions),
        ] {
            for repetition in 0..count {
                cancellation.poll().map_err(Error::Stopped)?;
                let started = Instant::now();
                let outcomes = if route == Route::Rayon {
                    pool.install(|| {
                        (0..configuration.queries)
                            .into_par_iter()
                            .map(|query| plan.query(route, query))
                            .collect::<Result<Vec<_>, _>>()
                    })?
                } else {
                    (0..configuration.queries)
                        .map(|query| plan.query(route, query))
                        .collect::<Result<Vec<_>, _>>()?
                };
                let batch_ns = started.elapsed().as_nanos();
                let mut output_bytes = outcomes.capacity() * size_of::<Outcome>();
                for (query, outcome) in outcomes.iter().enumerate() {
                    cancellation.poll().map_err(Error::Stopped)?;
                    match outcome {
                        Outcome::Complete {
                            query: position,
                            output,
                            ..
                        } => {
                            if *position != query || output != &expected[query] {
                                return Err(Error::Parity);
                            }
                            output_bytes += output.retained_bytes();
                        }
                        Outcome::Refused {
                            query: position, ..
                        } => {
                            if *position != query {
                                return Err(Error::Parity);
                            }
                            passed = false;
                        }
                    }
                }
                capacity(output_bytes)?;
                emit(&Event::Batch {
                    subject_sha256: fingerprint,
                    route,
                    phase,
                    repetition,
                    batch_ns,
                    output_bytes,
                    outcomes: &outcomes,
                })
                .map_err(Error::Output)?;
                batches += 1;
            }
        }
    }
    cancellation.poll().map_err(Error::Stopped)?;
    emit(&Event::Complete {
        queries: configuration.queries,
        batches,
        skipped_routes: if plan.table.is_none() {
            &[Route::Table, Route::Rayon]
        } else {
            &[]
        },
        passed,
    })
    .map_err(Error::Output)?;
    Ok(passed)
}

fn capacity(bytes: usize) -> Result<(), Error> {
    if bytes > MAX_RETAINED_BYTES {
        return Err(Error::Configuration("retained output exceeds 128 MiB"));
    }
    Ok(())
}
fn limits(configuration: Configuration) -> table::Limits {
    table::Limits {
        max_entries: MAX_VARIABLES * DOMAIN_VALUES,
        max_bytes: configuration.max_table_bytes,
        max_work: configuration.max_table_work,
    }
}
fn applicability(failure: table::Failure) -> Result<table::Failure, Error> {
    match failure.cause {
        table::Cause::Limit { .. } => Ok(failure),
        table::Cause::Interrupted(stop) => Err(Error::Stopped(stop)),
        _ => Err(Error::Table(failure)),
    }
}

type Domains<'a> = Vec<Vec<Vec<&'a Value>>>;
fn prepared_domains(fixture: &Fixture) -> Domains<'_> {
    fixture
        .domains
        .iter()
        .map(|family| {
            family
                .iter()
                .map(|domain| {
                    let mut values: Vec<_> = domain.iter().collect();
                    values.sort_unstable();
                    values.dedup();
                    values
                })
                .collect()
        })
        .collect()
}
fn domain_bytes(domains: &Domains<'_>) -> usize {
    domains.capacity() * size_of::<Vec<Vec<&Value>>>()
        + domains
            .iter()
            .map(|family| {
                family.capacity() * size_of::<Vec<&Value>>()
                    + family
                        .iter()
                        .map(|domain| domain.capacity() * size_of::<&Value>())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

struct Plan<'a, 'owner, 'source> {
    fixture: &'a Fixture,
    domains: &'a Domains<'a>,
    table: Option<&'a Table<'owner, 'source>>,
    configuration: Configuration,
    cancellation: &'a Cancellation,
}
impl Plan<'_, '_, '_> {
    fn query(&self, route: Route, query: usize) -> Result<Outcome, Error> {
        self.cancellation.poll().map_err(Error::Stopped)?;
        let started = Instant::now();
        if route == Route::Scan {
            let raw = scan(self.fixture, &self.domains[query], self.cancellation)?;
            let projection_ns = started.elapsed().as_nanos();
            let conversion = Instant::now();
            let comparisons = raw.comparisons;
            let output = raw.convert(self.fixture)?;
            let conversion_ns = conversion.elapsed().as_nanos();
            return Ok(Outcome::Complete {
                query,
                projection_ns,
                conversion_ns,
                scan_comparisons: Some(comparisons),
                table: None,
                output_bytes: output.retained_bytes(),
                output,
            });
        }
        let table = self.table.ok_or(Error::Parity)?;
        let family = &self.fixture.domains[query];
        let mut domains: [&[Value]; MAX_VARIABLES] = [&[]; MAX_VARIABLES];
        for (index, domain) in family.iter().enumerate() {
            domains[index] = domain;
        }
        let projection = match table.project(
            &domains[..family.len()],
            limits(self.configuration),
            self.cancellation,
        ) {
            Ok(projection) => projection,
            Err(failure) => {
                return Ok(Outcome::Refused {
                    query,
                    attempt_ns: started.elapsed().as_nanos(),
                    failure: applicability(failure)?,
                });
            }
        };
        let projection_ns = started.elapsed().as_nanos();
        let conversion = Instant::now();
        let rows = (0..self.fixture.indices.len())
            .filter(|&row| projection.contains(row))
            .collect();
        let domains = (0..family.len())
            .map(|variable| {
                projection
                    .domain(variable)
                    .ok_or(Error::Parity)?
                    .map(|value| self.fixture.id(value))
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        let output = Output { rows, domains };
        let conversion_ns = conversion.elapsed().as_nanos();
        Ok(Outcome::Complete {
            query,
            projection_ns,
            conversion_ns,
            scan_comparisons: None,
            table: Some(projection.statistics().into()),
            output_bytes: output.retained_bytes(),
            output,
        })
    }
}

struct Scanned<'a> {
    rows: Vec<usize>,
    domains: Vec<Vec<&'a Value>>,
    comparisons: u64,
}
impl Scanned<'_> {
    fn convert(self, fixture: &Fixture) -> Result<Output, Error> {
        Ok(Output {
            rows: self.rows,
            domains: self
                .domains
                .iter()
                .map(|domain| domain.iter().map(|value| fixture.id(*value)).collect())
                .collect::<Result<_, _>>()?,
        })
    }
}

fn scan<'a>(
    fixture: &'a Fixture,
    domains: &[Vec<&Value>],
    cancellation: &Cancellation,
) -> Result<Scanned<'a>, Error> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(fixture.indices.len())
        .map_err(|_| Error::Allocation)?;
    let mut projected: Vec<Vec<&Value>> = (0..domains.len())
        .map(|_| Vec::with_capacity(DICTIONARY_VALUES))
        .collect();
    let mut comparisons = 0;
    for (position, &original) in fixture.indices.iter().enumerate() {
        cancellation.poll().map_err(Error::Stopped)?;
        let tuple = fixture.atoms[original].values();
        let mut present = true;
        for (column, &variable) in fixture.scope.iter().enumerate() {
            if domains[variable]
                .binary_search_by(|value| {
                    comparisons += 1;
                    value.cmp(&&tuple[column])
                })
                .is_err()
            {
                present = false;
                break;
            }
            let first = fixture
                .scope
                .iter()
                .position(|&label| label == variable)
                .unwrap();
            if first != column {
                comparisons += 1;
                if tuple[first] != tuple[column] {
                    present = false;
                    break;
                }
            }
        }
        if !present {
            continue;
        }
        rows.push(position);
        for (value, &variable) in tuple.iter().zip(&fixture.scope) {
            if let Err(index) = projected[variable].binary_search_by(|other| {
                comparisons += 1;
                other.cmp(&value)
            }) {
                projected[variable].insert(index, value);
            }
        }
    }
    Ok(Scanned {
        rows,
        domains: projected,
        comparisons,
    })
}
