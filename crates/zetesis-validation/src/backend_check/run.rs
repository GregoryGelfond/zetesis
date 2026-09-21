use super::{
    Attempt, Backend, CaseResult, Decision, Error, Family, Report, Request, record::Cleanup,
    validate,
};
use crate::{
    process,
    selected::{self, identity},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ffi::OsString, path::Path, time::Duration};

pub(super) struct Fixture {
    pub name: &'static str,
    pub source: &'static str,
    pub oracle: selected::Oracle,
    pub models: &'static [&'static [&'static str]],
    pub costs: Option<&'static [(i32, i64)]>,
}
const FIXTURES: [Fixture; 3] = [
    Fixture {
        name: "tight-choice-hidden-atoms",
        source: "1 {a;b} 1. #show a/0.\n",
        oracle: selected::Oracle::Auto,
        models: &[&["a"], &["b"]],
        costs: None,
    },
    Fixture {
        name: "seeded-positive-cycle",
        source: "{seed}. a :- seed. a :- b. b :- a.\n",
        oracle: selected::Oracle::Countermodel,
        models: &[&[], &["a", "b", "seed"]],
        costs: None,
    },
    Fixture {
        name: "optimal-ties",
        source: "1 {a;b} 1. #minimize {1@2,a:a;1@2,b:b}.\n",
        oracle: selected::Oracle::Auto,
        models: &[&["a"], &["b"]],
        costs: Some(&[(2, 1)]),
    },
];
impl Fixture {
    pub(super) fn expected(&self) -> Family {
        Family {
            models: self
                .models
                .iter()
                .map(|model| model.iter().map(|atom| (*atom).to_owned()).collect())
                .collect(),
            costs: self.costs.map(<[_]>::to_vec),
        }
    }
}

pub(super) fn run(request: &Request, cancelled: &AtomicBool) -> Result<Report, Error> {
    if request.limits.timeout.is_zero()
        || request.limits.cleanup_timeout.is_zero()
        || request.limits.max_output_bytes == 0
    {
        return Err(Error::InvalidLimits);
    }
    // Primary binaries are read incrementally by the maintained identity owner.
    let executable = identity::seal(&request.executable, 256 * 1024 * 1024)
        .map_err(|error| Error::Identity(error.to_string()))?;
    let directory = tempfile::tempdir().map_err(Error::Io)?;
    for fixture in &FIXTURES {
        std::fs::write(directory.path().join(fixture.name), fixture.source).map_err(Error::Io)?;
    }
    let mut cases = Vec::new();
    for fixture in &FIXTURES {
        let reference = attempt(
            request,
            executable.canonical(),
            directory.path(),
            fixture,
            Backend::Cpu,
            cancelled,
        );
        let selected = if reference.decision == Decision::Passed {
            Some(attempt(
                request,
                executable.canonical(),
                directory.path(),
                fixture,
                request.backend,
                cancelled,
            ))
        } else {
            None
        };
        let unresolved = abandoned(&reference) || selected.as_ref().is_some_and(abandoned);
        let decision = selected.as_ref().map_or_else(
            || {
                if reference.decision == Decision::Cancelled {
                    Decision::Cancelled
                } else {
                    Decision::ReferenceFailed
                }
            },
            |attempt| attempt.decision,
        );
        cases.push(CaseResult {
            name: fixture.name,
            source: fixture.source,
            reference,
            selected,
            decision,
        });
        if unresolved || cancelled.load(Ordering::Relaxed) {
            break;
        }
    }
    let input_cleanup_failure = directory.close().err().map(|error| error.to_string());
    Ok(Report {
        schema: 1,
        format: "zetesis-backend-check",
        scope: "Three fixed eager formula checks against known full selected families and CPU reference; mandatory route telemetry; not the complete physical regression or coverage suite",
        backend: request.backend,
        expected_cases: FIXTURES.len(),
        executable_after: identity::recheck(&executable),
        executable,
        cases,
        input_cleanup_failure,
    })
}

fn abandoned(attempt: &Attempt) -> bool {
    attempt
        .cleanup
        .as_ref()
        .is_some_and(|cleanup| cleanup.abandoned_child.is_some())
}

fn arguments(fixture: &Fixture, backend: Backend) -> Vec<String> {
    [
        "solve",
        fixture.name,
        "--all",
        "--json",
        "--stats",
        "--color",
        "never",
        "--device",
        match backend {
            Backend::Cpu => "cpu",
            Backend::Metal => "metal",
        },
        "--grounder",
        "eager",
        "--oracle",
        match fixture.oracle {
            selected::Oracle::Auto => "auto",
            selected::Oracle::Countermodel => "countermodel",
            selected::Oracle::Closure => "closure",
        },
        "--threads",
        "1",
        "--completion-workers",
        "1",
        "--batch-size",
        "64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn attempt(
    request: &Request,
    executable: &Path,
    directory: &Path,
    fixture: &Fixture,
    backend: Backend,
    cancelled: &AtomicBool,
) -> Attempt {
    let arguments = arguments(fixture, backend);
    let argv: Vec<OsString> = arguments.iter().map(OsString::from).collect();
    let mut result = Attempt {
        arguments,
        capture: None,
        decision: Decision::ProcessFailure,
        detail: None,
        family: None,
        observation: None,
        cleanup: None,
    };
    let output = match process::invoke_with_cancellation(
        process::Invocation {
            executable,
            arguments: &argv,
            directory,
        },
        request.limits,
        cancelled,
    ) {
        Ok(output) => output,
        Err(error) => {
            if matches!(error, process::StartError::Cancelled) {
                result.decision = Decision::Cancelled;
            }
            result.detail = Some(error.to_string());
            return result;
        }
    };
    let (capture, pending) = output.into_parts();
    // Settle ownership before decoding or any later fallible publication.
    if let Some(pending) = pending {
        let cleanup = pending.retry(Duration::from_secs(1));
        result.cleanup = Some(Cleanup {
            exit: cleanup.exit,
            failure: cleanup.failure.map(|error| error.to_string()),
            abandoned_child: cleanup.pending.map(process::PendingChild::abandon),
        });
    }
    let checked = validate::capture(&capture, backend, fixture);
    result.capture = Some(capture);
    match checked {
        Ok((family, observation)) if result.cleanup.is_none() => {
            result.family = Some(family);
            result.observation = Some(observation);
            result.decision = Decision::Passed;
        }
        Ok(_) => {
            result.detail = Some("initial child cleanup did not complete normally".into());
        }
        Err((decision, detail)) => {
            result.decision = decision;
            result.detail = Some(detail);
        }
    }
    result
}
