//! Sequential effects and typed per-case decisions.
use super::capture::{self, Capture};
use super::corpus::{Case, Loaded};
use super::record::{CaseEvidence, Cleanup, Observation};
use super::{CaseResult, Decision, Producer, Report, Request, execution, normalize};
use crate::{phase, stage};
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

pub(super) fn run(
    request: &Request,
    corpus: Loaded,
    mut on_case: impl FnMut(&CaseResult),
) -> Report {
    let mut cases = Vec::new();
    let mut pending = Vec::new();
    for case in &corpus.manifest.cases {
        let result = check_case(request, &corpus, case, &mut pending);
        on_case(&result);
        cases.push(result);
        if !pending.is_empty() {
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
        corpus,
        cases,
        cleanup,
    }
}

fn check_case(
    request: &Request,
    corpus: &Loaded,
    case: &Case,
    pending: &mut Vec<crate::process::PendingChild>,
) -> CaseResult {
    let input = corpus.root.join(&case.path);
    let arguments = vec![
        "--models=0".into(),
        "--outf=2".into(),
        "--opt-mode=optN".into(),
        input.as_os_str().to_owned(),
    ];
    let reference = match invoke(request, &request.clingo, &arguments, &corpus.root) {
        Ok(capture) => capture,
        Err(error) => {
            return decide(
                CaseEvidence::new(case),
                Decision::InvocationFailed(Producer::Reference, error),
            );
        }
    };
    let mut result = CaseEvidence::new(case);
    let reference = retain_pending(reference, pending);
    let failure = reference.status.failure();
    let exit = reference.exit_code;
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
    check_native(request, corpus, result, pending)
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
    pending: &mut Vec<crate::process::PendingChild>,
) -> CaseResult {
    let input = corpus.root.join(&result.source.path);
    let mut arguments = vec![
        "--backend".into(),
        request.native_backend.label().into(),
        "--oracle".into(),
        request.native_oracle.label().into(),
        "--models".into(),
        "0".into(),
        "--batch-size".into(),
        request.native_batch_size.to_string().into(),
        "--completion-workers".into(),
        request.native_completion_workers.to_string().into(),
        "--max-completion-scratch-bytes".into(),
        request
            .native_max_completion_scratch_bytes
            .to_string()
            .into(),
    ];
    if request.effective_native_stats() {
        arguments.push("--stats".into());
    }
    arguments.push(input.as_os_str().to_owned());
    let native = invoke(request, &request.zetesis, &arguments, &corpus.root);
    result.native_arguments = Some(arguments);
    let native = match native {
        Ok(capture) => retain_pending(capture, pending),
        Err(error) => return decide(result, Decision::InvocationFailed(Producer::Native, error)),
    };
    result.phase = observe(phase::parse(&native.stderr));
    result.stage = observe(stage::parse(&native.stderr));
    let decision = native_failure(&native);
    result.native_process = Some(native);
    if let Some(decision) = decision {
        return decide(result, decision);
    }
    let answer = result.reference_answer.as_ref().unwrap();
    let native_answer = match normalize::native(
        &result.native_process.as_ref().unwrap().stdout,
        answer.cost.is_some(),
    ) {
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
    if native.exit_code == Some(3) {
        return Some(Decision::NativeIncomplete);
    }
    if native.exit_code == Some(0) {
        return None;
    }
    Some(
        if native.exit_code == Some(2)
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
) -> Result<Capture, String> {
    capture::invoke(
        executable,
        arguments,
        directory,
        Duration::from_millis(request.timeout_ms),
        request.max_output_bytes,
    )
}
fn decide(evidence: CaseEvidence, decision: Decision) -> CaseResult {
    CaseResult { evidence, decision }
}

#[cfg(all(test, unix))]
#[path = "../../tests/support/runner_contracts.rs"]
mod tests;
