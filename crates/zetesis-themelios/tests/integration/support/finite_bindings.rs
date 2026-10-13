//! Independent complete-interpretation and frozen-formula test evaluators.

use serde_json::Value as Json;
use std::collections::BTreeSet;
use std::time::Duration;
use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{NodeView as Node, Theory};
use zetesis_reference_support::canonical;
use zetesis_themelios::AdmittedFormula;

pub(crate) use zetesis_test_support::records::Models;

pub(crate) fn values(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut result = Vec::new();
    for index in 0..theory.view().len() {
        let node = theory.view().node(index).unwrap();
        let value = match node {
            Node::False => false,
            Node::ExtremumCut(cut) => {
                use zetesis_ferraris::ExtremumCutRule::{Prefix, Suffix};
                match *cut.rule {
                    Prefix(0) => false,
                    Prefix(boundary) if boundary == cut.row.rank_count => true,
                    Suffix(0) => true,
                    Suffix(boundary) if boundary == cut.row.rank_count => false,
                    rule => cut
                        .row
                        .conditions
                        .iter()
                        .zip(cut.row.ranks)
                        .any(|(&child, &rank)| {
                            let qualifies = match rule {
                                Prefix(boundary) => rank < boundary,
                                Suffix(boundary) => rank >= boundary,
                            };
                            qualifies && result[child]
                        }),
                }
            }
            Node::Threshold(row) => {
                row.conditions
                    .iter()
                    .zip(row.weights)
                    .filter(|(child, _)| result[**child])
                    .map(|(_, weight)| i128::from(*weight))
                    .sum::<i128>()
                    >= row.bound
            }
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(row) => row.iter().all(|&child| result[child]),
            Node::Or(row) => row.iter().any(|&child| result[child]),
            Node::Implies(left, right) => !result[left] || result[right],
        };
        result.push(value && frozen.is_none_or(|outer| outer[index]));
    }
    result
}

pub(crate) fn holds(theory: &Theory, values: &[bool]) -> bool {
    theory.roots().iter().all(|&root| values[root])
}

pub(crate) fn exhaustive(input: &AdmittedFormula) -> Models {
    assert!(input.atoms().len() <= 12, "bounded independent carrier");
    let mut result = BTreeSet::new();
    for mask in 0..1_usize << input.atoms().len() {
        let outer = values(input.theory(), mask, None);
        if !holds(input.theory(), &outer) {
            continue;
        }
        let mut subset = mask;
        let mut countermodel = false;
        while subset != 0 {
            subset = (subset - 1) & mask;
            if holds(
                input.theory(),
                &values(input.theory(), subset, Some(&outer)),
            ) {
                countermodel = true;
                break;
            }
        }
        if !countermodel {
            assert!(
                result.insert(
                    input
                        .atoms()
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| mask & (1 << index) != 0)
                        .map(|(_, atom)| canonical(atom))
                        .collect()
                )
            );
        }
    }
    result
}

pub(crate) fn native(input: &AdmittedFormula) -> Models {
    let mut search = zetesis_sat::StableModels::new(
        input.theory(),
        zetesis_sat::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let mut result = BTreeSet::new();
    for model in search.by_ref() {
        assert!(
            result.insert(
                model
                    .unwrap()
                    .atoms()
                    .map(|index| canonical(input.atoms().at(index).unwrap()))
                    .collect()
            )
        );
    }
    assert!(search.exhausted(), "complete original-theory enumeration");
    result
}

pub(crate) fn external(source: &str, valid: bool) -> Json {
    let exits: &[i32] = if valid { &oracle::DECIDED } else { &[65] };
    let run = oracle::run_accepting(
        source,
        &[
            "--models=0",
            "--outf=2",
            "--parallel-mode=1",
            "--opt-mode=enum",
        ],
        exits,
        oracle::Limits {
            timeout: Duration::from_secs(3),
            max_output_bytes: 2 * 65_536,
        },
    );
    let diagnostics = String::from_utf8_lossy(run.stderr());
    // A refused source must be refused for its unsafe variables.
    assert!(
        valid || diagnostics.contains("unsafe variables"),
        "{diagnostics}"
    );
    println!(
        "{}",
        serde_json::json!({"source": source, "valid": valid, "exit": run.code(), "stdout": String::from_utf8_lossy(run.stdout()), "stderr": diagnostics})
    );
    oracle::json(&run)
}

/// Each of `sources`, admitted by `input`, has by native enumeration exactly
/// the full models clingo enumerates for it.
pub(crate) fn original_sources_match_clingo_full_models<'a>(
    sources: impl IntoIterator<Item = &'a str>,
    input: impl Fn(&str) -> AdmittedFormula,
) {
    let (mut compared, mut models) = (0, 0);
    for source in sources {
        let expected = oracle::full_models(&external(source, true));
        assert_eq!(native(&input(source)), expected, "{source}");
        compared += 1;
        models += expected.len();
    }
    println!("complete_sources={compared} full_models={models}");
}

/// Each source of `pairs` and its expansion, both admitted by `input`, have
/// the same atoms and agree on classical truth and on frozen truth under every
/// pair of interpretations. Arbitrary M/J includes both proper subsets and
/// unrelated worlds.
pub(crate) fn frozen_truth_matches<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    input: impl Fn(&str) -> AdmittedFormula,
) {
    let (mut sources, mut compared) = (0, 0);
    for (source, expanded) in pairs {
        let left = input(source);
        let right = input(expanded);
        let names: Vec<_> = left.atoms().iter().map(canonical).collect();
        let other: Vec<_> = right.atoms().iter().map(canonical).collect();
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>(),
            other.iter().collect(),
            "{source}"
        );
        assert!(names.len() <= 7, "bounded exhaustive interpretation pairs");
        let remap = |mask: usize| {
            names.iter().enumerate().fold(0, |result, (index, name)| {
                result
                    | (usize::from(mask & (1 << index) != 0)
                        << other.iter().position(|other| other == name).unwrap())
            })
        };
        for outer in 0..1 << names.len() {
            let a = values(left.theory(), outer, None);
            let b = values(right.theory(), remap(outer), None);
            assert_eq!(
                holds(left.theory(), &a),
                holds(right.theory(), &b),
                "{source}"
            );
            for inner in 0..1 << names.len() {
                assert_eq!(
                    holds(left.theory(), &values(left.theory(), inner, Some(&a))),
                    holds(
                        right.theory(),
                        &values(right.theory(), remap(inner), Some(&b))
                    ),
                    "{source}: M={outer} J={inner}"
                );
                compared += 1;
            }
        }
        sources += 1;
    }
    println!("source_pairs={sources} frozen_pairs={compared}");
}

/// The mask over `to`'s atoms that holds the atoms `mask` holds over `from`'s.
pub(crate) fn remap(
    mask: usize,
    from: zetesis_core::catalog::Atoms<'_>,
    to: zetesis_core::catalog::Atoms<'_>,
) -> usize {
    from.iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .fold(0, |bits, (_, atom)| {
            bits | (1 << to.iter().position(|other| atom == other).unwrap())
        })
}

/// The models written as lists of atom spellings.
pub(crate) fn expected(records: &[&[&str]]) -> Models {
    records
        .iter()
        .map(|record| record.iter().map(|name| (*name).to_owned()).collect())
        .collect()
}
