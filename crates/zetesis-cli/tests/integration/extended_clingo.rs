//! Independent exhausted-model comparison for the opt-in source extension.
//! clingo is only an optional reference solver; original strings remain whole atoms
//! through JSON and quote-aware native output parsing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::process::{Command, Stdio};

use clap::Parser;
use serde_json::Value as Json;
use zetesis_cli::{Completion, Options, run};
use zetesis_cpu::Cancellation;
use zetesis_themelios::{
    AdmissionFailure, AdmissionOptions, ExpansionFailure, ExpansionLimits, ProfileFeature, admit,
    admit_extended,
};

// Display selection can map distinct stable models to the same answer line.
// Preserve multiplicity rather than treating #show as model projection.
type Models = BTreeMap<BTreeSet<String>, usize>;

fn model_atoms(line: &str) -> BTreeSet<String> {
    // Model separators are whitespace only outside a string literal. Escaped
    // quotes/backslashes and literal tabs inside strings cannot split an atom.
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut atoms = BTreeSet::new();
    for (index, character) in line.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if character.is_whitespace() {
            if start < index {
                assert!(
                    atoms.insert(line[start..index].to_owned()),
                    "duplicate atom"
                );
            }
            start = index + character.len_utf8();
        }
    }
    assert!(!quoted && !escaped, "unterminated model string: {line}");
    if start < line.len() {
        assert!(atoms.insert(line[start..].to_owned()), "duplicate atom");
    }
    atoms
}

fn native(source: &str) -> (Models, String) {
    native_search(source, "closure")
}

fn native_search(source: &str, search: &str) -> (Models, String) {
    let options = Options::try_parse_from([
        "zetesis",
        "--oracle",
        search,
        "--backend",
        "cpu",
        "--models",
        "0",
        "--workers",
        "2",
    ])
    .expect("test options");
    let mut bytes = Vec::new();
    let report = run(
        source.to_owned(),
        &options,
        &mut bytes,
        &Cancellation::default(),
    )
    .unwrap_or_else(|error| panic!("native source {source}: {error}"));
    assert_eq!(report.completion, Completion::Exhausted, "source: {source}");
    let text = String::from_utf8(bytes).expect("native UTF-8 output");
    let mut models = Models::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Answer:") {
            *models
                .entry(model_atoms(lines.next().expect("answer atoms")))
                .or_default() += 1;
        }
    }
    assert_eq!(models.values().sum::<usize>(), report.models);
    (models, text)
}

fn external(source: &str) -> Models {
    let mut child = Command::new("clingo")
        .args(["-", "0", "--outf=2", "--warn=none"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("independently installed clingo on PATH");
    child
        .stdin
        .take()
        .expect("oracle stdin")
        .write_all(source.as_bytes())
        .expect("oracle source");
    let output = child.wait_with_output().expect("oracle completion");
    assert!(
        matches!(output.status.code(), Some(10 | 20 | 30)),
        "clingo source {source}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Json = serde_json::from_slice(&output.stdout).expect("clingo JSON output");
    assert!(
        matches!(
            json["Result"].as_str(),
            Some("SATISFIABLE" | "UNSATISFIABLE")
        ),
        "incomplete oracle: {json}"
    );
    assert_eq!(
        json["Models"]["More"].as_str(),
        Some("no"),
        "oracle exhaustion: {source}"
    );
    let mut models = Models::new();
    for call in json["Call"].as_array().expect("oracle calls") {
        if let Some(witnesses) = call["Witnesses"].as_array() {
            for witness in witnesses {
                let atoms = witness["Value"]
                    .as_array()
                    .expect("oracle atom array")
                    .iter()
                    .map(|atom| {
                        atom.as_str()
                            .expect("one whole atom per JSON string")
                            .to_owned()
                    })
                    .collect();
                *models.entry(atoms).or_default() += 1;
            }
        }
    }
    assert_eq!(
        u64::try_from(models.values().sum::<usize>()).expect("model count"),
        json["Models"]["Number"]
            .as_u64()
            .expect("oracle model count")
    );
    models
}

fn compare(source: &str) {
    let (models, _) = native(source);
    let expected = external(source);
    assert_eq!(models, expected, "source: {source}");
    let (sat, _) = native_search(source, "countermodel");
    assert_eq!(sat, expected, "SAT source: {source}");
}

#[test]
fn native_strings_keep_clingo_spelling_and_do_not_split_model_atoms() {
    let source = "p(\"space here\",\"say \\\"hi\\\"\",\"a\\\\b\",\"a\\nb\",\"a\tb\",\"λ雪\").";
    let (models, output) = native(source);
    assert_eq!(models.len(), 1);
    let (model, multiplicity) = models.first_key_value().expect("one model");
    assert_eq!(*multiplicity, 1);
    assert_eq!(model.len(), 1);
    assert_eq!(
        model.first().expect("one atom"),
        source.trim_end_matches('.')
    );
    assert!(output.contains("a\tb"), "literal tab is preserved");
    assert!(
        !output.contains("a\\tb"),
        "clingo has no backslash-t escape"
    );
}

#[test]
fn shown_answers_preserve_hidden_model_multiplicity() {
    let (models, output) = native("#defined absent/2. {hidden}. visible. #show visible/0.");
    assert_eq!(models.len(), 1);
    assert_eq!(models.values().sum::<usize>(), 2);
    assert!(output.contains("Answer: 1\nvisible\nAnswer: 2\nvisible\n"));
    assert!(output.contains("Models: 2;"));
    let (empty, output) = native("{hidden}. #show.");
    assert_eq!(empty.get(&BTreeSet::new()), Some(&2));
    assert!(output.contains("Answer: 1\n\nAnswer: 2\n\n"));
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn display_signatures_and_declarations_preserve_complete_model_multisets() {
    for source in [
        "#defined absent/2. {hidden}. visible. #show visible/0.",
        "#defined absent/2. {hidden}.",
        "{hidden}. #show.",
        "{hidden}. #show absent/0.",
        "p. p(1). q(2). #show p/0.",
        "p. p(1). q(2). #show p/1. #show q/1.",
        "p. q. #show p/0. #show. #show p/0.",
        "p. q. #show. #show p/0.",
        "#show. #defined p/1.",
        "{a}. {b}. #show a/0.",
        "{hidden}. visible :- hidden. #show visible/0.",
        "#show p/0. :-.",
    ] {
        compare(source);
    }
}

#[test]
fn deliberately_unsupported_arithmetic_remains_an_admission_refusal() {
    for source in [
        "p(1/0).",
        "p(1\\0).",
        "p(2**(-1)).",
        "p(2147483647+1).",
        "p((-2147483647-1)/(-1)).",
        "p(|(-2147483647-1)|).",
        "p(a+1).",
        "p((1;2)+3).",
        "p(X+1) :- d(X).",
    ] {
        assert!(
            admit_extended(
                source.to_owned(),
                AdmissionOptions::default(),
                ExpansionLimits::default()
            )
            .is_err(),
            "source: {source}"
        );
    }
}

#[test]
fn nul_strings_refuse_instead_of_inheriting_clingo_truncation() {
    let source = "p(\"a\0b\").";
    assert!(matches!(
        admit(source.to_owned(), AdmissionOptions::default()),
        Err(AdmissionFailure::Profile {
            feature: ProfileFeature::NulString,
            ..
        })
    ));
    assert!(matches!(
        admit_extended(
            source.to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        ),
        Err(ExpansionFailure::Admission(AdmissionFailure::Profile {
            feature: ProfileFeature::NulString,
            ..
        }))
    ));
    assert!(
        admit_extended(
            "#const a=\"a\0b\". p(a).".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
    assert!(
        admit_extended(
            "p(\"raw\nnewline\").".to_owned(),
            AdmissionOptions::default(),
            ExpansionLimits::default()
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn generated_checked_numeric_expressions_match_clingo() {
    let mut expressions = 0;
    for left in [-7, -3, -1, 0, 1, 3, 7] {
        for right in [-5, -2, -1, 0, 1, 2, 5] {
            let mut source = String::new();
            for (index, operator) in ["+", "-", "*", "&", "?", "^", "/", "\\"].iter().enumerate() {
                if right == 0 && matches!(*operator, "/" | "\\") {
                    continue;
                }
                write!(source, "v({index},({left}){operator}({right})). ").expect("string append");
                expressions += 1;
            }
            write!(
                source,
                "v(8,({left})+({right})*2). v(9,({left})-({right})-2). v(10,2+({left})**2)."
            )
            .expect("precedence cases");
            expressions += 3;
            compare(&source);
        }
        compare(&format!("v(~({left}), |({left})|, -(-({left})))."));
        expressions += 3;
    }
    for base in [-3, 0, 2] {
        for exponent in 0..6 {
            compare(&format!("v(({base})**{exponent})."));
            expressions += 1;
        }
    }
    assert_eq!(expressions, 564);
    compare("v(-2147483647-1,2147483647,(-2147483647-1)+1,2**30,(-2147483647-1)\\3).");
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn original_strings_constants_and_fact_set_forms_match_clingo() {
    for source in [
        "p(\"space here\",\"say \\\"hi\\\"\",\"a\\\\b\",\"a\\nb\",\"a\tb\",\"λ雪\").",
        "p(\"a\rb\",\"a\u{000c}b\",\"a\u{007f}b\").",
        "#const top=low+2. #const low=1. #const p=7. p(low..top,p,\"low\",unbound).",
        "#const name=\"a b\". #const alias=name. p(alias;\"other\\\\value\").",
        "p((1;3),2..4). p(a;b,c). q((1..2;2..3)).",
        "p(2..1). q(3..1,a). r.",
        "#const a=b. #const b=c. p(a,b,c).",
        "#const unused=5. p.",
        "",
    ] {
        compare(source);
    }
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn expanded_relational_rules_compare_all_stable_models() {
    for lower in -2..=2 {
        for upper in -2..=2 {
            compare(&format!(
                "#const lo={lower}. #const hi={upper}. d(lo..hi). {{selected(X)}} :- d(X). :- selected(X), selected(Y), X != Y. seen :- selected(X)."
            ));
        }
    }
    for source in [
        "edge(1,2;2,3;3,4). reach(X,Y) :- edge(X,Y). reach(X,Z) :- reach(X,Y), edge(Y,Z).",
        "#const n=2. d(1..n). {p(X)} :- d(X), X != n+1. q(X) :- d(X), not p(X). :- not not p(1), p(2).",
        "#const n=2. d(1..n). {p(X)} :- d(X). q(X) :- d(X), not not p(X). :- q(1), q(2).",
        "p(\"a b\";\"a\\\\b\";\"a\\nb\"). {q(X)} :- p(X). :- q(\"a b\"), q(\"a\\\\b\").",
        "p(1..2). :- p(1).",
        "p(2..1). :- not p(1).",
    ] {
        compare(source);
    }
}

fn scalar_order_source() -> String {
    let values = [
        "(-2147483647-1)",
        "-1",
        "0",
        "1",
        "2147483647",
        "a",
        "aa",
        "z",
        "\"\"",
        "\"a\"",
        "\"aa\"",
        "\"z\"",
        "\"é\"",
        "\"😀\"",
    ];
    let mut source = String::new();
    for (index, value) in values.iter().enumerate() {
        writeln!(source, "d({index},{value}).").unwrap();
    }
    for (name, operator) in [
        ("lt", "<"),
        ("le", "<="),
        ("eq", "="),
        ("ne", "!="),
        ("ge", ">="),
        ("gt", ">"),
    ] {
        writeln!(
            source,
            "{name}(I,J) :- d(I,X), d(J,Y), X {operator} Y. #show {name}/2."
        )
        .unwrap();
    }
    source
}

#[test]
#[ignore = "requires an independently installed clingo executable"]
fn mixed_scalar_order_comparisons_match_clingo() {
    let source = scalar_order_source();
    let expected = external(&source);
    let (actual, _) = native_search(&source, "countermodel");
    assert_eq!(actual, expected);
}

#[test]
fn scalar_ordering_selects_symbolic_and_string_pairs_without_numeric_coercion() {
    let (models, _) = native_search(&scalar_order_source(), "countermodel");
    assert_eq!(models.len(), 1);
    let atoms = models.keys().next().unwrap();
    assert_eq!(models.values().copied().sum::<usize>(), 1);
    assert_eq!(atoms.len(), 14 * 14 * 3);
    for atom in [
        "lt(0,1)",
        "lt(4,5)",
        "lt(5,6)",
        "lt(7,8)",
        "lt(8,9)",
        "lt(12,13)",
        "eq(0,0)",
        "ge(13,5)",
    ] {
        assert!(atoms.contains(atom), "missing {atom}");
    }
    for atom in ["eq(5,9)", "lt(8,7)", "lt(13,12)"] {
        assert!(!atoms.contains(atom), "unexpected {atom}");
    }
}
