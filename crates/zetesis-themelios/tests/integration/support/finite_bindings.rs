//! Independent complete-interpretation and frozen-formula test evaluators.

use serde_json::Value as Json;
use std::collections::BTreeSet;
use std::time::Duration;
use zetesis_clingo_support as oracle;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Node, Theory};
use zetesis_reference_support::canonical;
use zetesis_themelios::AdmittedFormula;

pub(crate) type Models = BTreeSet<BTreeSet<String>>;

pub(crate) fn values(theory: &Theory, mask: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut result = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let value = match *node {
            Node::False => false,
            Node::Atom(atom) => mask & (1 << atom) != 0,
            Node::And(left, right) => result[left] && result[right],
            Node::Or(left, right) => result[left] || result[right],
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
