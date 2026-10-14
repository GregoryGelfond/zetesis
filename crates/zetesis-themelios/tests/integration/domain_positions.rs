//! On every maintained program the lazy route admits (the correctness corpus,
//! the scalability programs and the top-level examples) and on generated fact
//! shapes, the domain position table names each domain value's position and
//! each constant head term's, exactly as the domain's content search finds
//! them. CPU oracle preparation reads constants through the table, and the
//! content search is the authority it must agree with.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use zetesis_core::{DomainPositions, TemplateTerm};
use zetesis_test_support::{programs::rules, repository};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, admit_extended};

fn programs(directory: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            programs(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "lp") {
            found.push(path);
        }
    }
}

/// Check the table against the content search on `text`'s program; whether
/// the lazy route admitted it.
fn agrees(text: String, name: &str) -> bool {
    // A program the lazy route does not admit never reaches CPU oracle
    // preparation's table.
    let Ok(admitted) = admit_extended(
        text,
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    ) else {
        return false;
    };
    let program = admitted.program();
    let positions = DomainPositions::build(program, |slots| Ok::<_, ()>(Vec::with_capacity(slots)))
        .unwrap()
        .unwrap();
    let domain = program.domain();
    for index in 0..domain.len() {
        let value = domain.at(index).unwrap();
        assert_eq!(positions.position(value), Some(index), "{name}");
    }
    for template in rules(program) {
        let Some(head) = template.head() else {
            continue;
        };
        for term in head.terms() {
            if let TemplateTerm::Constant(value) = term {
                assert_eq!(
                    positions.position(value),
                    domain.binary_search(value).ok(),
                    "{name}"
                );
            }
        }
    }
    true
}

#[test]
fn table_positions_equal_the_content_search_on_maintained_programs() {
    let mut paths = Vec::new();
    programs(&repository::examples(), &mut paths);
    let admitted = paths
        .iter()
        .filter(|path| {
            agrees(
                std::fs::read_to_string(path).unwrap(),
                &path.display().to_string(),
            )
        })
        .count();
    assert!(admitted > 0, "some maintained program takes the lazy route");
}

#[test]
fn table_positions_equal_the_content_search_on_fact_shapes() {
    let (mut bare, mut wide, mut join) = (String::new(), String::new(), String::new());
    for i in 0..300 {
        write!(bare, "p({i}). ").unwrap();
        // Nested function terms put subterms that are not domain values into
        // the catalog: g(N) occurs only inside f(N, g(N)).
        write!(
            wide,
            "e({i}, c{i}, \"s{}\", f({i}, g({}))). ",
            i % 13,
            i % 7
        )
        .unwrap();
        write!(join, "e({i}, {}). ", i + 1).unwrap();
    }
    join.push_str("r(X, Y) :- e(X, Y). r(X, Z) :- r(X, Y), e(Y, Z).");
    for (name, text) in [("bare", bare), ("wide", wide), ("join", join)] {
        assert!(agrees(text, name), "{name} takes the lazy route");
    }
}
