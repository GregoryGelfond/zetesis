//! Nonblocking Linux/macOS capture; the waitable leader reserves the group ID.

use std::io::{self, Read};
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};

use super::{
    Capture, Cleanup, Failure, Invocation, Limits, Operation, Outcome, PendingChild, StartError,
    Stop,
};

const POLL_INTERVAL: Duration = Duration::from_millis(1);
const READ_CHUNK: usize = 8192;

pub(super) fn invoke(
    invocation: Invocation<'_>,
    limits: Limits,
    started: Instant,
    deadline: Instant,
    supervised: bool,
) -> Result<Outcome, StartError> {
    let mut child = Command::new(invocation.executable)
        .args(invocation.arguments)
        .current_dir(invocation.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(StartError::Spawn)?;
    // Command requests both pipes. A missing handle is still a post-spawn fault.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut owned = PendingChild {
        child,
        group_owned: true,
    };
    let mut capture = Capture {
        child_id: owned.child.id(),
        stop: Stop::Failure,
        exit: None,
        elapsed: Duration::ZERO,
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure: None,
        cleanup_failure: None,
    };
    let setup = configure(stdout.as_ref()).and_then(|()| configure(stderr.as_ref()));
    if let Err(error) = setup {
        capture.failure = Some(Failure::new(Operation::ConfigurePipe, error));
    } else {
        poll(
            &mut owned,
            &mut stdout,
            &mut stderr,
            &mut capture,
            limits.max_output_bytes,
            deadline,
        );
    }
    // No drain thread can outlive this call. Once stopped, keep the exact prefix
    // already admitted and close both pipes before bounded group cleanup.
    drop(stdout);
    drop(stderr);
    if supervised && capture.stop == Stop::Completed && owned.group_owned {
        match helper_succeeded(&owned) {
            Ok(true) => {}
            Ok(false) => {
                // A failed helper may have left its solver after closing pipes.
                // The waitable helper still reserves this group ID.
                if let Err(error) =
                    pid(&owned).and_then(|pid| match kill_process_group(pid, Signal::KILL) {
                        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
                        Err(error) => Err(error.into()),
                    })
                {
                    capture.failure = Some(Failure::new(Operation::TerminateGroup, error));
                    capture.stop = Stop::Failure;
                }
            }
            Err(error) => {
                owned.group_owned = false;
                capture.failure = Some(Failure::new(Operation::ObserveExit, error));
                capture.stop = Stop::Failure;
            }
        }
    }
    let cleanup = finish(
        owned,
        limits.cleanup_timeout,
        capture.stop != Stop::Completed,
    );
    capture.exit = cleanup.exit;
    capture.cleanup_failure = cleanup.failure;
    if capture.stop == Stop::Completed
        && (cleanup.pending.is_some() || capture.cleanup_failure.is_some())
    {
        capture.stop = Stop::Failure;
    }
    capture.elapsed = started.elapsed();
    Ok(Outcome {
        capture,
        pending: cleanup.pending,
    })
}

fn poll(
    owned: &mut PendingChild,
    stdout: &mut Option<ChildStdout>,
    stderr: &mut Option<ChildStderr>,
    capture: &mut Capture,
    output_limit: usize,
    deadline: Instant,
) {
    let mut remaining = output_limit;
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut stderr_first = false;
    loop {
        if Instant::now() >= deadline {
            capture.stop = Stop::Deadline;
            break;
        }
        match observe(owned) {
            Ok(true) if !stdout_open && !stderr_open => {
                capture.stop = Stop::Completed;
                break;
            }
            Ok(_) => {}
            Err(error) => {
                // ECHILD can mean an outside reaper consumed our reservation.
                // Never signal a group whose leader ownership became uncertain.
                owned.group_owned = false;
                capture.failure = Some(Failure::new(Operation::ObserveExit, error));
                break;
            }
        }
        let mut progress = false;
        let mut streams = [
            (
                stdout.as_mut().map(|pipe| pipe as &mut dyn Read),
                &mut capture.stdout,
                &mut stdout_open,
                Operation::ReadStdout,
            ),
            (
                stderr.as_mut().map(|pipe| pipe as &mut dyn Read),
                &mut capture.stderr,
                &mut stderr_open,
                Operation::ReadStderr,
            ),
        ];
        if stderr_first {
            streams.swap(0, 1);
        }
        stderr_first = !stderr_first;
        for (pipe, bytes, open, operation) in streams {
            if !*open {
                continue;
            }
            let Some(pipe) = pipe else {
                continue;
            };
            match drain(pipe, bytes, &mut remaining, operation) {
                Ok(ReadState::Blocked) => {}
                Ok(ReadState::Progress) => progress = true,
                Ok(ReadState::Closed) => {
                    *open = false;
                    progress = true;
                }
                Ok(ReadState::Exceeded) => {
                    capture.stop = Stop::OutputLimit;
                    break;
                }
                Err(error) => {
                    capture.failure = Some(error);
                    break;
                }
            }
        }
        if capture.stop == Stop::OutputLimit || capture.failure.is_some() {
            break;
        }
        pause_if_idle(progress, deadline, thread::sleep);
    }
}

fn pause_if_idle(progress: bool, deadline: Instant, pause: impl FnOnce(Duration)) {
    if !progress {
        pause(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

fn configure(pipe: Option<&impl AsFd>) -> io::Result<()> {
    let pipe = pipe.ok_or_else(|| io::Error::other("missing requested child pipe"))?;
    let flags = fcntl_getfl(pipe)?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK)?;
    Ok(())
}

fn pid(child: &PendingChild) -> io::Result<Pid> {
    // Pid excludes zero; exclude one too because kill(-1, ...) has special scope.
    i32::try_from(child.child.id())
        .ok()
        .filter(|raw| *raw > 1)
        .and_then(Pid::from_raw)
        .ok_or_else(|| io::Error::other("child ID cannot identify an owned process group"))
}

fn observe(child: &PendingChild) -> io::Result<bool> {
    // NOWAIT is essential: the leader stays waitable until after group signalling,
    // preventing its numeric group ID from being reused between wait and kill.
    waitid(
        WaitId::Pid(pid(child)?),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    )
    .map(|status| status.is_some())
    .map_err(Into::into)
}

fn helper_succeeded(child: &PendingChild) -> io::Result<bool> {
    waitid(
        WaitId::Pid(pid(child)?),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    )
    .map(|status| status.is_some_and(|status| status.exit_status() == Some(0)))
    .map_err(Into::into)
}

enum ReadState {
    Blocked,
    Progress,
    Closed,
    Exceeded,
}

fn drain(
    pipe: &mut dyn Read,
    output: &mut Vec<u8>,
    remaining: &mut usize,
    operation: Operation,
) -> Result<ReadState, Failure> {
    let mut buffer = [0_u8; READ_CHUNK];
    let attempted = remaining.saturating_add(1).min(READ_CHUNK);
    let count = match pipe.read(&mut buffer[..attempted]) {
        Ok(0) => return Ok(ReadState::Closed),
        Ok(count) => count,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            return Ok(ReadState::Blocked);
        }
        Err(error) => return Err(Failure::new(operation, error)),
    };
    let retained = count.min(*remaining);
    output
        .try_reserve(retained)
        .map_err(|error| Failure::new(Operation::RetainBytes, io::Error::other(error)))?;
    output.extend_from_slice(&buffer[..retained]);
    *remaining -= retained;
    Ok(if retained == count {
        ReadState::Progress
    } else {
        ReadState::Exceeded
    })
}

pub(super) fn cleanup(child: PendingChild, timeout: Duration) -> Cleanup {
    finish(child, timeout, true)
}

fn finish(mut child: PendingChild, timeout: Duration, terminate: bool) -> Cleanup {
    let Some(deadline) = Instant::now().checked_add(timeout) else {
        return Cleanup {
            exit: None,
            pending: Some(child),
            failure: Some(Failure::new(
                Operation::ReapChild,
                io::Error::other("cleanup deadline overflow"),
            )),
        };
    };
    let mut failure = None;
    if terminate
        && child.group_owned
        && let Err(error) =
            pid(&child).and_then(|pid| kill_process_group(pid, Signal::KILL).map_err(Into::into))
    {
        failure = Some(Failure::new(Operation::TerminateGroup, error));
    }
    loop {
        match child.child.try_wait() {
            Ok(Some(exit)) => {
                return Cleanup {
                    exit: Some(exit.into()),
                    pending: None,
                    failure,
                };
            }
            Ok(None) => {}
            Err(error) => {
                child.group_owned = false;
                failure.get_or_insert_with(|| Failure::new(Operation::ReapChild, error));
                return Cleanup {
                    exit: None,
                    pending: Some(child),
                    failure,
                };
            }
        }
        if Instant::now() >= deadline {
            return Cleanup {
                exit: None,
                pending: Some(child),
                failure,
            };
        }
        thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn productive_read_does_not_schedule_a_pause() {
        let mut output = Vec::new();
        let mut remaining = READ_CHUNK * 2;
        let input = vec![b'x'; READ_CHUNK * 2];
        let mut reader = io::Cursor::new(input);
        for _ in 0..2 {
            let state = drain(
                &mut reader,
                &mut output,
                &mut remaining,
                Operation::ReadStdout,
            )
            .unwrap();
            assert!(matches!(state, ReadState::Progress));
            pause_if_idle(matches!(state, ReadState::Progress), Instant::now(), |_| {
                panic!("productive capture was throttled")
            });
        }
        assert_eq!(output.len(), READ_CHUNK * 2);
    }

    #[test]
    fn idle_poll_schedules_one_bounded_pause() {
        let mut calls = Vec::new();
        pause_if_idle(false, Instant::now() + Duration::from_secs(1), |duration| {
            calls.push(duration);
        });
        assert_eq!(calls, [POLL_INTERVAL]);
    }

    #[test]
    fn cleanup_returns_unreaped_ownership() {
        let child = Command::new("/bin/sh")
            .args(["-c", "exec sleep 5"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let id = child.id();
        // Suppress signalling for the first attempt so pending ownership is
        // deterministic, independent of SIGKILL delivery or scheduler timing.
        let first = cleanup(
            PendingChild {
                child,
                group_owned: false,
            },
            Duration::ZERO,
        );
        assert!(first.exit.is_none());
        let mut pending = first.pending.expect("live child remains owned");
        assert_eq!(pending.id(), id);
        pending.group_owned = true;
        let second = pending.retry(Duration::from_secs(1));
        if let Some(pending) = second.pending {
            panic!("fixture cleanup abandoned child {}", pending.abandon());
        }
        assert_eq!(second.exit.unwrap().signal, Some(9));
    }

    #[test]
    fn read_failure_retains_the_admitted_prefix() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected capture failure"))
            }
        }
        let mut output = b"prefix".to_vec();
        let mut remaining = 10;
        let Err(failure) = drain(
            &mut Broken,
            &mut output,
            &mut remaining,
            Operation::ReadStdout,
        ) else {
            panic!("broken reader unexpectedly succeeded");
        };
        assert_eq!(failure.operation(), Operation::ReadStdout);
        assert_eq!(output, b"prefix");
        assert_eq!(remaining, 10);
    }
}
