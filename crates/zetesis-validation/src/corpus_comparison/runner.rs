//! Sequential effects and typed per-case decisions.
use super::capture::{self, Capture};
use super::corpus::{Case, Loaded};
use super::record::{CaseEvidence, Cleanup, Observation};
use super::{
    CaseResult, Decision, NativeInvocation, Producer, Report, Request, execution, normalize,
};
use crate::{phase, stage};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[cfg(test)]
pub(super) fn run(request: &Request, corpus: Loaded, on_case: impl FnMut(&CaseResult)) -> Report {
    run_with_invocation(request, corpus, NativeInvocation::Legacy, on_case)
}

#[cfg(test)]
pub(super) fn run_with_invocation(
    request: &Request,
    corpus: Loaded,
    invocation: NativeInvocation,
    on_case: impl FnMut(&CaseResult),
) -> Report {
    run_with_cancellation(
        request,
        corpus,
        invocation,
        &AtomicBool::new(false),
        on_case,
    )
}

pub(super) fn run_with_cancellation(
    request: &Request,
    corpus: Loaded,
    invocation: NativeInvocation,
    cancelled: &AtomicBool,
    mut on_case: impl FnMut(&CaseResult),
) -> Report {
    let mut cases = Vec::new();
    let mut pending = Vec::new();
    let mut cancellation_observed = false;
    for case in &corpus.manifest.cases {
        if cancelled.load(Ordering::Relaxed) {
            cancellation_observed = true;
            break;
        }
        let result = check_case(request, &corpus, case, invocation, &mut pending, cancelled);
        cancellation_observed = matches!(
            result.decision,
            Decision::Cancelled(_) | Decision::CaptureFailed(_, super::CaptureFailure::Cancelled)
        );
        let unresolved = result
            .evidence
            .reference_process
            .as_ref()
            .is_some_and(Capture::cleanup_unresolved)
            || result
                .evidence
                .native_process
                .as_ref()
                .is_some_and(Capture::cleanup_unresolved);
        on_case(&result);
        cases.push(result);
        if unresolved || cancellation_observed {
            break;
        }
    }
    // Stop launching cases after unresolved cleanup. Retain one bounded retry
    // and explicitly record any abandoned ownership at the failure boundary.
    let cleanup = pending
        .into_iter()
        .map(|child: crate::process::PendingChild| {
            let child_id = child.id();
            let result = child.retry(Duration::from_secs(1));
            Cleanup {
                child_id,
                exit: result.exit,
                failure: result.failure.map(|error| error.to_string()),
                abandoned_unreaped_child: result.pending.map(crate::process::PendingChild::abandon),
            }
        })
        .collect();
    Report {
        request: request.clone(),
        invocation,
        required_cases: 94,
        corpus,
        cases,
        cleanup,
        cancelled: cancellation_observed,
    }
}

fn check_case(
    request: &Request,
    corpus: &Loaded,
    case: &Case,
    invocation: NativeInvocation,
    pending: &mut Vec<crate::process::PendingChild>,
    cancelled: &AtomicBool,
) -> CaseResult {
    let input = corpus.root.join(&case.path);
    let arguments = vec![
        "--models=0".into(),
        "--outf=2".into(),
        "--opt-mode=optN".into(),
        input.as_os_str().to_owned(),
    ];
    let reference = match invoke(
        request,
        &request.clingo,
        &arguments,
        &corpus.root,
        cancelled,
    ) {
        Ok(capture) => capture,
        Err(error) => {
            return decide(
                CaseEvidence::new(case),
                invocation_failure(Producer::Reference, error),
            );
        }
    };
    let mut result = CaseEvidence::new(case);
    let reference = retain_pending(reference, pending);
    let failure = reference.status.failure();
    let exit = reference.exit.0.and_then(|exit| exit.code);
    result.reference_process = Some(reference);
    if let Some(failure) = failure {
        return decide(
            result,
            Decision::CaptureFailed(Producer::Reference, failure),
        );
    }
    if !matches!(exit, Some(10 | 20 | 30)) {
        return decide(result, Decision::ReferenceError);
    }
    let answer = match normalize::reference(&result.reference_process.as_ref().unwrap().stdout) {
        Ok(answer) => answer,
        Err(error) => return decide(result, Decision::ReferenceOutputError(error)),
    };
    let contract = normalize::contracts(case, &answer);
    result.reference_answer = Some(answer);
    if let Err(error) = contract {
        return decide(result, Decision::ReferenceContractMismatch(error));
    }
    if request.reference_only {
        return decide(result, Decision::ReferencePassed);
    }
    check_native(request, corpus, result, invocation, pending, cancelled)
}

fn retain_pending(capture: Capture, pending: &mut Vec<crate::process::PendingChild>) -> Capture {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut capture = capture;
        if let Some(child) = capture.pending.take() {
            pending.push(child);
        }
        capture
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pending;
        capture
    }
}

fn check_native(
    request: &Request,
    corpus: &Loaded,
    mut result: CaseEvidence,
    invocation: NativeInvocation,
    pending: &mut Vec<crate::process::PendingChild>,
    cancelled: &AtomicBool,
) -> CaseResult {
    let input = corpus.root.join(&result.source.path);
    let arguments = native_arguments(request, invocation, &input);
    let native = invoke(
        request,
        &request.zetesis,
        &arguments,
        &corpus.root,
        cancelled,
    );
    result.native_arguments = Some(arguments);
    let native = match native {
        Ok(capture) => retain_pending(capture, pending),
        Err(error) => return decide(result, invocation_failure(Producer::Native, error)),
    };
    result.phase = observe(phase::parse(&native.stderr));
    result.stage = observe(stage::parse(&native.stderr));
    let decision = native_failure(&native);
    result.native_process = Some(native);
    if let Some(decision) = decision {
        return decide(result, decision);
    }
    check_answers(request, invocation, result)
}

fn native_arguments(
    request: &Request,
    invocation: NativeInvocation,
    input: &Path,
) -> Vec<OsString> {
    // Every native executable accepts `--backend`: older ones as their flag,
    // those between fe635ff3 and the flag's return as an alias.
    let mut arguments: Vec<OsString> = match invocation {
        NativeInvocation::Legacy => Vec::new(),
        NativeInvocation::Solve => vec!["solve".into(), "--all".into(), "--json".into()],
    };
    arguments.extend([
        "--backend".into(),
        request.native_backend.label().into(),
        "--oracle".into(),
        request.native_oracle.label().into(),
    ]);
    if invocation == NativeInvocation::Legacy {
        arguments.extend(["--models".into(), "0".into()]);
    }
    arguments.extend([
        "--batch-size".into(),
        request.native_batch_size.to_string().into(),
        "--completion-workers".into(),
        request.native_completion_workers.to_string().into(),
        "--max-completion-scratch-bytes".into(),
        request
            .native_max_completion_scratch_bytes
            .to_string()
            .into(),
    ]);
    if request.effective_native_stats() {
        arguments.push("--stats".into());
    }
    arguments.push(input.as_os_str().to_owned());
    arguments
}

fn check_answers(
    request: &Request,
    invocation: NativeInvocation,
    mut result: CaseEvidence,
) -> CaseResult {
    let answer = result.reference_answer.as_ref().unwrap();
    let output = &result.native_process.as_ref().unwrap().stdout;
    let parsed = match invocation {
        NativeInvocation::Legacy => normalize::native(output, answer.cost.is_some()),
        NativeInvocation::Solve => normalize::native_json(output, request.max_output_bytes),
    };
    let native_answer = match parsed {
        Ok(answer) => answer,
        Err(crate::answers::Error::Invalid {
            issue: crate::answers::Issue::Incomplete,
            ..
        }) => return decide(result, Decision::NativeIncomplete),
        Err(error) => {
            return decide(result, Decision::NativeOutputUnsupported(error.to_string()));
        }
    };
    let same = normalize::same(answer, &native_answer);
    result.native_answer = Some(native_answer);
    if !same {
        return decide(result, Decision::Mismatch);
    }
    if let Err(error) = normalize::contracts(&result.source, result.native_answer.as_ref().unwrap())
    {
        return decide(result, Decision::NativeContractMismatch(error));
    }
    result.answer_parity = true;
    if request.physical_formula() {
        match execution::formula_for_request(
            &result.native_process.as_ref().unwrap().stderr,
            request.native_backend,
            request.native_batch_size.get(),
            execution::CompletionRequest {
                workers: request.native_completion_workers,
                max_scratch_bytes: request.native_max_completion_scratch_bytes,
            },
            result.native_answer.as_ref().unwrap(),
        ) {
            Ok(evidence) => result.execution = Some(evidence),
            Err(error) => return decide(result, Decision::NativeExecutionUnqualified(error)),
        }
    }
    decide(result, Decision::Passed)
}

fn native_failure(native: &Capture) -> Option<Decision> {
    if let Some(failure) = native.status.failure() {
        return Some(Decision::CaptureFailed(Producer::Native, failure));
    }
    let exit_code = native.exit.0.and_then(|exit| exit.code);
    if exit_code == Some(3) {
        return Some(Decision::NativeIncomplete);
    }
    if exit_code == Some(0) {
        return None;
    }
    Some(
        if exit_code == Some(2)
            && (native.stderr.contains("source admission:")
                || native.stderr.contains("S0")
                || native.stderr.contains("source expansion"))
        {
            Decision::NativeSourceRefused
        } else {
            Decision::NativeError
        },
    )
}
fn observe<T>(parsed: Result<Option<T>, String>) -> Observation<T> {
    match parsed {
        Ok(Some(value)) => Observation::Available(value),
        Ok(None) => Observation::Absent,
        Err(error) => Observation::Malformed(error),
    }
}
fn invoke(
    request: &Request,
    executable: &Path,
    arguments: &[OsString],
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<Capture, super::decision::InvocationFailure> {
    capture::invoke_with_cancellation(
        executable,
        arguments,
        directory,
        Duration::from_millis(request.timeout_ms),
        request.max_output_bytes,
        cancelled,
    )
}
fn invocation_failure(producer: Producer, error: super::decision::InvocationFailure) -> Decision {
    match error {
        super::decision::InvocationFailure::Cancelled => Decision::Cancelled(producer),
        super::decision::InvocationFailure::Other(error) => {
            Decision::InvocationFailed(producer, error)
        }
    }
}
fn decide(evidence: CaseEvidence, decision: Decision) -> CaseResult {
    CaseResult { evidence, decision }
}

#[cfg(all(test, unix))]
#[path = "../../tests/support/runner_contracts.rs"]
mod tests;
