//! Process admission and JSON publication preserve extremal logical values.

use std::io::Write;
use std::process::{Command, Stdio};
use zetesis_validation::answers::native_json;

#[test]
fn extremal_terms_reach_complete_process_output() {
    for (source, expected) in [
        ("t(#inf).", vec![vec!["t(#inf)"]]),
        ("u(#sup).", vec![vec!["u(#sup)"]]),
        ("p(X):-X=#inf.", vec![vec!["p(#inf)"]]),
        ("{p(#sup)}.", vec![vec![], vec!["p(#sup)"]]),
        ("p:-not q(#inf).", vec![vec!["p"]]),
        ("p(1).q(#inf,X):-p(X).", vec![vec!["p(1)", "q(#inf,1)"]]),
    ] {
        for oracle in ["auto", "countermodel"] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_zetesis"))
                .args([
                    "-",
                    "--backend",
                    "cpu",
                    "--workers",
                    "1",
                    "--models",
                    "0",
                    "--json",
                    "--oracle",
                    oracle,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(source.as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{source} ({oracle}): {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let answers =
                native_json::parse(&output.stdout, native_json::Limits::default()).unwrap();
            assert_eq!(
                answers.full_model_symbols(8 * 1024).unwrap(),
                expected,
                "{source} ({oracle})"
            );
        }
    }
}
