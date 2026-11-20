//! `--threads N` bounds every thread the solver runs at once, the main thread
//! included: grounding, search, completion and deadlines alike.

use std::io::Write;
use std::process::{Command, Stdio};

const GROUNDING: &str = "n(1..150).
p(X,Y) :- n(X), n(Y), X < Y, Y < X + 10.
q(X) :- p(X,Y), not p(Y,X).
r(X,Z) :- p(X,Y), p(Y,Z).
";

const SEARCH: &str = "#const n = 9.
{ q(R,C) : C = 1..n } = 1 :- R = 1..n.
:- q(R1,C), q(R2,C), R1 < R2.
:- q(R1,C1), q(R2,C2), R1 < R2, R2 - R1 = |C2 - C1|.
";

const DISJUNCTIVE: &str = "n(1..10).
a(X) | b(X) :- n(X).
:- a(X), a(X + 1).
c :- b(X), b(X + 1), b(X + 2).
";

const TIMED: &str = "#const n = 13.
{ q(R,C) : C = 1..n } = 1 :- R = 1..n.
:- q(R1,C), q(R2,C), R1 < R2.
:- q(R1,C1), q(R2,C2), R1 < R2, R2 - R1 = |C2 - C1|.
";

/// The audited peak, the main thread included, and the OS count at exit
/// where the platform exposes it.
fn peak(program: &str, threads: usize, extra: &[&str]) -> (usize, Option<usize>) {
    let mut source = tempfile::NamedTempFile::new().unwrap();
    source.write_all(program.as_bytes()).unwrap();
    let threads = threads.to_string();
    let result = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .arg(source.path())
        .args(["--backend", "cpu", "--color", "never", "--models", "0"])
        .args(["--threads", &threads])
        .args(extra)
        .env("ZETESIS_THREAD_AUDIT", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let diagnostics = String::from_utf8(result.stderr).unwrap();
    assert!(matches!(result.status.code(), Some(0 | 3)), "{diagnostics}");
    let audit = diagnostics
        .lines()
        .find_map(|line| line.strip_prefix("zetesis-thread-audit: "))
        .unwrap_or_else(|| panic!("no audit line: {diagnostics}"));
    let field = |name: &str| {
        audit
            .split(' ')
            .find_map(|field| field.strip_prefix(name))
            .map(|value| value.parse().unwrap())
    };
    (field("peak=").unwrap(), field("os_live="))
}

#[test]
fn no_more_threads_run_than_requested() {
    let cases: [(&str, &str, &[&str]); 6] = [
        ("grounding", GROUNDING, &["--grounder", "eager"]),
        ("grounding-lazy", GROUNDING, &["--grounder", "lazy"]),
        ("search", SEARCH, &[]),
        ("disjunctive", DISJUNCTIVE, &[]),
        ("timed", TIMED, &["--time-limit", "1"]),
        ("timed-expired", TIMED, &["--time-limit", "0"]),
    ];
    for threads in [1, 2, 3] {
        for (name, program, extra) in cases {
            let (peak, os_live) = peak(program, threads, extra);
            assert!(
                peak <= threads,
                "{name} at --threads {threads}: peak {peak}"
            );
            if let Some(os_live) = os_live {
                assert!(
                    os_live <= threads,
                    "{name} at --threads {threads}: {os_live} at exit"
                );
            }
        }
    }
}

#[test]
fn several_threads_are_used_when_requested() {
    // The audit counts real threads: a wide request starts more than one.
    let (peak, _) = peak(GROUNDING, 4, &["--grounder", "eager"]);
    assert!(peak > 1, "peak {peak}");
}
