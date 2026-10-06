//! Located admission refusals name the file, line and column through the
//! canonical source view, for a single file and an included one alike.

use std::fs;
use std::process::{Command, Stdio};

/// A choice and a count aggregate: formula admission, whose work ceiling a
/// tiny allowance trips at the aggregate rule on line 3, column 1.
const COUNT: &str = "{ a(1..3) }.\n\nb :- #count{ X : a(X) } >= 2.\n";

fn refusal(files: &[(&str, &str)]) -> String {
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(directory.path().join(name), text).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args([
            "--backend",
            "cpu",
            "--color",
            "never",
            "--max-expansion-work",
            "20",
        ])
        .arg(directory.path().join("entry.lp"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn a_formula_limit_refusal_names_the_line_and_column() {
    let diagnostic = refusal(&[("entry.lp", COUNT)]);
    assert!(diagnostic.contains("entry.lp:3:1"), "{diagnostic}");
    assert!(!diagnostic.contains(": bytes "), "{diagnostic}");
}

#[test]
fn an_included_file_refusal_names_its_own_line_and_column() {
    let diagnostic = refusal(&[
        ("entry.lp", "#include \"child.lp\".\n"),
        ("child.lp", COUNT),
    ]);
    assert!(diagnostic.contains("child.lp:3:1"), "{diagnostic}");
    assert!(!diagnostic.contains(": bytes "), "{diagnostic}");
}

#[test]
fn a_program_wide_refusal_claims_no_line_or_column() {
    // A tiny cap on support rounds stops completion of the whole program,
    // a failure no single statement owns.
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("entry.lp"),
        "{ a(1..3) }.\nb(X) :- a(X).\nc(X) :- b(X).\nd :- #count{ X : c(X) } >= 2.\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zetesis"))
        .args([
            "--backend",
            "cpu",
            "--color",
            "never",
            "--max-support-rounds",
            "1",
        ])
        .arg(directory.path().join("entry.lp"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(2), "{diagnostic}");
    assert!(
        diagnostic.contains("entry.lp: while admitting the program"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("entry.lp:1:1"), "{diagnostic}");
}
