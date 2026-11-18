//! Solver threads start only through `zetesis_algebra::threads`, which counts
//! them against `--threads` and joins its pools. Authored library source
//! outside that module names none of the calls that start a thread or enter
//! Rayon's global pool, except the named exceptions below. Test code (files
//! under `tests/`, `tests.rs`, `testing.rs`, and a file's `#[cfg(test)] mod
//! tests`) may start its own threads. This is a textual audit: parallel
//! iterators installed on a counted pool are outside it, and the CLI's
//! thread-cap test observes the whole process at run time.

use crate::support::{authored_sources, repository, source};

/// Paths that start a thread or enter Rayon's global pool.
const CALLS: &[&str] = &[
    "thread::spawn",
    "thread::Builder",
    "thread::scope",
    "rayon::join",
    "rayon::scope",
    "rayon::scope_fifo",
    "rayon::in_place_scope",
    "rayon::in_place_scope_fifo",
    "rayon::spawn",
    "rayon::spawn_fifo",
    "rayon::broadcast",
    "rayon::spawn_broadcast",
    "rayon::current_num_threads",
    "ThreadPoolBuilder",
    "build_global",
];

/// Names that, imported from `std::thread` or `rayon`, do the same.
const IMPORTS: &[(&str, &[&str])] = &[
    ("std::thread::{", &["spawn", "Builder", "scope"]),
    (
        "rayon::{",
        &[
            "join",
            "scope",
            "scope_fifo",
            "in_place_scope",
            "in_place_scope_fifo",
            "spawn",
            "spawn_fifo",
            "broadcast",
            "spawn_broadcast",
            "current_num_threads",
            "ThreadPoolBuilder",
        ],
    ),
];

/// A library thread outside the solver, with the reason it is not counted.
struct Exception {
    file: &'static str,
    call: &'static str,
    reason: &'static str,
}

/// The module that counts and joins every solver thread, and the named
/// exceptions.
const COUNTING: &str = "crates/zetesis-algebra/src/threads.rs";
const EXCEPTIONS: &[Exception] = &[Exception {
    file: "crates/zetesis-validation/src/corpus_comparison/capture/portable.rs",
    call: "thread::spawn",
    reason: "drains a measured child process's output pipe in the validation harness; it is not a solver thread",
}];

/// Whether `file` is test code, which may start its own threads.
fn is_test_file(file: &str) -> bool {
    file.contains("/tests/")
        || file.contains("/benches/")
        || file.contains("/examples/")
        || file.ends_with("tests.rs")
        || file.ends_with("testing.rs")
}

/// Whether `line` makes the item after it test-only.
fn is_test_attribute(line: &str) -> bool {
    let line = line.trim();
    line == "#[cfg(test)]" || line.starts_with("#[cfg(all(test,")
}

/// The authored text before the file's inline test module, if any.
fn library_part(text: &str) -> &str {
    let mut at = 0;
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if is_test_attribute(line)
            && lines
                .peek()
                .is_some_and(|next| next.starts_with("mod ") && next.trim_end().ends_with('{'))
        {
            return &text[..at];
        }
        at += line.len();
    }
    text
}

/// The files that test-only `mod name;` declarations in `file` bring in.
fn test_modules(file: &str, text: &str) -> Vec<String> {
    let directory = match file.rsplit_once('/') {
        Some((directory, "lib.rs" | "main.rs" | "mod.rs")) => directory.to_owned(),
        _ => file.trim_end_matches(".rs").to_owned(),
    };
    let mut modules = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if is_test_attribute(line)
            && let Some(name) = lines.peek().and_then(|next| {
                next.trim()
                    .strip_prefix("mod ")
                    .and_then(|rest| rest.strip_suffix(';'))
            })
        {
            modules.push(format!("{directory}/{name}.rs"));
            modules.push(format!("{directory}/{name}/mod.rs"));
        }
    }
    modules
}

/// The calls `text` makes, each with its line, before its inline test module.
fn calls(text: &str) -> Vec<(usize, &'static str)> {
    let text = library_part(text);
    let line = |at: usize| text[..at].matches('\n').count() + 1;
    let mut found = Vec::new();
    for call in CALLS {
        for (at, _) in text.match_indices(call) {
            let after = text[at + call.len()..].chars().next();
            if !after.is_some_and(|next| next.is_alphanumeric() || next == '_') {
                found.push((line(at), *call));
            }
        }
    }
    for (prefix, names) in IMPORTS {
        for (at, _) in text.match_indices(prefix) {
            let list = &text[at + prefix.len()..];
            let list = &list[..list.find('}').unwrap_or(list.len())];
            for token in list.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if let Some(name) = names.iter().find(|name| **name == token) {
                    found.push((line(at), *name));
                }
            }
        }
    }
    found.sort_unstable();
    found
}

#[test]
fn library_threads_start_only_through_the_counting_module() {
    let root = repository();
    let sources: Vec<(String, String)> = authored_sources(&root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            let file = path.strip_prefix(&root).unwrap().display().to_string();
            (file, source(&path).unwrap())
        })
        .collect();
    let test_only: Vec<String> = sources
        .iter()
        .flat_map(|(file, text)| test_modules(file, text))
        .collect();
    let mut violations = Vec::new();
    let mut excepted = Vec::new();
    for (file, text) in &sources {
        if file == COUNTING || is_test_file(file) || test_only.contains(file) {
            continue;
        }
        for (line, call) in calls(text) {
            match EXCEPTIONS
                .iter()
                .find(|exception| exception.file == *file && exception.call == call)
            {
                Some(exception) => excepted.push(exception.file),
                None => violations.push(format!(
                    "{file}:{line}: `{call}` starts a thread outside {COUNTING}"
                )),
            }
        }
    }
    for exception in EXCEPTIONS {
        if !excepted.contains(&exception.file) {
            violations.push(format!(
                "{}: the exception for `{}` ({}) names no present call",
                exception.file, exception.call, exception.reason
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn the_audit_finds_calls_and_imports_but_not_counted_pools_or_tests() {
    let found = calls(
        "use std::thread::{self, Builder};\nfn f() { std::thread::spawn(|| ()); rayon::join(|| (), || ()); }\nuse rayon::{prelude::*, scope};\n",
    );
    assert_eq!(
        found,
        [
            (1, "Builder"),
            (2, "rayon::join"),
            (2, "thread::spawn"),
            (3, "scope")
        ]
    );
    assert!(
        calls("use rayon::{Scope, ThreadPool, ThreadPoolBuildError};\nfn f(p: &ThreadPool) { p.install(|| ()); p.in_place_scope(|_| ()); }\n")
            .is_empty()
    );
    assert!(
        calls(
            "fn f() {}\n#[cfg(test)]\nmod tests {\n    fn g() { std::thread::spawn(|| ()); }\n}\n"
        )
        .is_empty()
    );
    assert!(
        calls(
            "#[cfg(all(test, unix))]\nmod tests {\n    fn g() { std::thread::scope(|_| ()); }\n}\n"
        )
        .is_empty()
    );
    assert_eq!(
        test_modules("crates/a/src/b.rs", "#[cfg(test)]\nmod c;\nmod d;\n"),
        ["crates/a/src/b/c.rs", "crates/a/src/b/c/mod.rs"]
    );
    assert_eq!(
        test_modules("crates/a/src/lib.rs", "#[cfg(test)]\nmod c;\n")[0],
        "crates/a/src/c.rs"
    );
    assert!(is_test_file("crates/a/src/b/tests.rs"));
    assert!(!is_test_file("crates/a/src/b.rs"));
}
