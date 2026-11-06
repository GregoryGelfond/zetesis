//! The quantified series: the inner preparation's census and door checks on
//! the saturation cores, each size's family checked against clingo. Each
//! size is the committed program with its one size constant amended; the
//! solver runs in-process on the countermodel oracle, one thread, the eager
//! grounder, and reports its statistics in the JSON document.
use clap::Parser;
use serde_json::Value;
use std::collections::BTreeSet;
use zetesis_algebra::Cancellation;
use zetesis_cli::{Completion, Options, run};
use zetesis_clingo_support as oracle;
use zetesis_test_support::repository::examples as root;

/// One core family: its committed program, its size constant with the
/// committed value, and the sizes of the series.
#[derive(Clone, Copy, Debug)]
struct Family {
    name: &'static str,
    path: &'static str,
    constant: &'static str,
    default: u64,
    sizes: [u64; 4],
    /// The core's universal atoms at a size.
    universals: fn(u64) -> u64,
}

const PARITY: Family = Family {
    name: "parity",
    path: "scalability/two-qbf-parity/two_qbf_parity.lp",
    constant: "n",
    default: 6,
    sizes: [16, 20, 24, 28],
    // The inputs and the gates' outputs.
    universals: |n| 3 * n - 2,
};
const TSEITIN: Family = Family {
    name: "tseitin",
    path: "scalability/two-qbf-tseitin/two_qbf_tseitin.lp",
    constant: "vertices",
    default: 12,
    sizes: [12, 18, 24, 30],
    // The edges of a 3-regular graph.
    universals: |vertices| 3 * vertices / 2,
};
const PIGEONHOLE: Family = Family {
    name: "pigeonhole",
    path: "scalability/two-qbf-pigeonhole/two_qbf_pigeonhole.lp",
    constant: "holes",
    default: 4,
    sizes: [5, 6, 7, 8],
    // The places: every pigeon in every hole.
    universals: |holes| holes * (holes + 1),
};

impl Family {
    /// The committed program with its size constant amended to `size`: the
    /// one `#const` line, found once at the committed value.
    fn program(self, size: u64) -> String {
        let text = std::fs::read_to_string(root().join(self.path)).unwrap();
        let line = format!("#const {} = {}.", self.constant, self.default);
        assert_eq!(text.matches(&line).count(), 1, "{self:?}");
        text.replace(&line, &format!("#const {} = {}.", self.constant, size))
    }

    /// The core's universal atoms at `size`.
    fn universals(self, size: u64) -> u64 {
        (self.universals)(size)
    }
}

/// What one solve reports: the family, the candidates, the inner walk's
/// regions and the preparation's receipts.
struct Receipt {
    family: BTreeSet<BTreeSet<String>>,
    models: u64,
    candidates: u64,
    visited: u64,
    refuted: u64,
    preparation: Value,
}

/// A term as clingo displays it: a number or a symbol.
fn term(parts: &Value) -> String {
    let parts = parts.as_array().unwrap();
    assert_eq!(parts.len(), 1, "{parts:?}");
    let part = &parts[0];
    match part["kind"].as_str().unwrap() {
        "number" => part["value"].to_string(),
        "symbol" => part["value"].as_str().unwrap().to_owned(),
        other => panic!("term kind {other}"),
    }
}

/// A model's shown atoms as clingo displays them.
fn shown(model: &Value) -> BTreeSet<String> {
    let atoms = model["model"]["atoms"].as_array().unwrap();
    model["model"]["shown"]["atom_indices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|index| {
            let atom = &atoms[usize::try_from(index.as_u64().unwrap()).unwrap()];
            assert_eq!(atom["sign"], "positive");
            let arguments: Vec<String> = atom["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .map(term)
                .collect();
            format!(
                "{}({})",
                atom["predicate"].as_str().unwrap(),
                arguments.join(",")
            )
        })
        .collect()
}

fn solve(program: &str) -> Receipt {
    let options = Options::try_parse_from([
        "zetesis",
        "--backend",
        "cpu",
        "--oracle",
        "countermodel",
        "--grounder",
        "eager",
        "--threads",
        "1",
        "--models",
        "0",
        "--color",
        "never",
        "--json",
        "--stats",
    ])
    .unwrap();
    let mut bytes = Vec::new();
    let report = run(
        program.into(),
        &options,
        &mut bytes,
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Exhausted);
    let document: Value = serde_json::Deserializer::from_slice(&bytes)
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(document["outcome"]["completion"], "exhausted");
    let search = &document["statistics"]["search"];
    let regions = &search["reduct_query_regions"];
    Receipt {
        family: document["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(shown)
            .collect(),
        models: document["outcome"]["published_models"].as_u64().unwrap(),
        candidates: search["candidates"].as_u64().unwrap(),
        visited: regions["visited"].as_u64().unwrap(),
        refuted: regions["refuted"].as_u64().unwrap(),
        preparation: regions["preparation"].clone(),
    }
}

/// clingo's family of the program: each model's shown atoms.
fn external(program: &str) -> BTreeSet<BTreeSet<String>> {
    let run = oracle::run(program, &["0", "--verbose=0"], oracle::Limits::default());
    let text = std::str::from_utf8(run.stdout()).unwrap();
    let mut lines: Vec<_> = text.lines().collect();
    let status = lines.pop().expect("clingo status line");
    assert!(
        matches!(status, "SATISFIABLE" | "UNSATISFIABLE"),
        "{status}"
    );
    lines
        .into_iter()
        .map(|line| line.split_whitespace().map(str::to_owned).collect())
        .collect()
}

/// Solve one size, its family checked against clingo's.
fn checked(family: Family, size: u64) -> Receipt {
    let program = family.program(size);
    let receipt = solve(&program);
    let expected = external(&program);
    assert_eq!(receipt.family, expected, "{} {size}", family.name);
    receipt
}

fn count(preparation: &Value, field: &str) -> u64 {
    preparation[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{field} in {preparation}"))
}

#[test]
#[ignore = "requires clingo: the quantified series' families agree and the inner census holds"]
fn g_census_inner_finds_the_cores_groups_and_rows() {
    // Parity over n inputs: two chains of n - 1 gates, four clauses each, and
    // the pair of clauses that makes the results differ: 2(n - 1) + 1 groups
    // per candidate. The stable candidate's rows refute its root in the
    // first round; each dropped-clause candidate's inexact gate is probed to
    // exactness in a second round under the other rows, so every hull ends
    // exact and the rows number at least the gates' (8n - 7 in all).
    for n in PARITY.sizes {
        let receipt = checked(PARITY, n);
        let p = &receipt.preparation;
        let candidates = receipt.candidates;
        assert_eq!((receipt.models, candidates), (1, 4), "n={n}");
        assert_eq!(count(p, "attempts"), candidates, "n={n}");
        assert_eq!(count(p, "prepared"), candidates, "n={n}");
        assert_eq!(count(p, "saturation_units"), candidates, "n={n}");
        assert_eq!(count(p, "groups"), candidates * (2 * (n - 1) + 1), "n={n}");
        assert_eq!(count(p, "inexact_hulls"), 0, "n={n}");
        assert!(
            count(p, "rows") >= 8 * n - 7,
            "n={n}: rows {}",
            count(p, "rows")
        );
        assert_eq!(count(p, "rounds"), candidates + 3, "n={n}");
        println!(
            "parity n={n}: probes={} rounds={} rows={} work={} regions={} refuted={}",
            count(p, "probes"),
            count(p, "rounds"),
            count(p, "rows"),
            count(p, "work"),
            receipt.visited,
            receipt.refuted
        );
    }
    // Tseitin on a cubic graph: one group of four clauses per vertex; the
    // stable candidate loads a row per vertex, and each dropped-clause
    // candidate's inexact vertex is probed to exactness in a second round.
    for vertices in TSEITIN.sizes {
        let receipt = checked(TSEITIN, vertices);
        let p = &receipt.preparation;
        let candidates = receipt.candidates;
        assert_eq!((receipt.models, candidates), (1, 4), "vertices={vertices}");
        assert_eq!(
            count(p, "saturation_units"),
            candidates,
            "vertices={vertices}"
        );
        assert_eq!(
            count(p, "groups"),
            candidates * vertices,
            "vertices={vertices}"
        );
        assert_eq!(count(p, "inexact_hulls"), 0, "vertices={vertices}");
        assert!(
            count(p, "rows") >= 4 * vertices - 3,
            "vertices={vertices}: rows {}",
            count(p, "rows")
        );
        println!(
            "tseitin vertices={vertices}: probes={} rows={} work={} regions={} refuted={}",
            count(p, "probes"),
            count(p, "rows"),
            count(p, "work"),
            receipt.visited,
            receipt.refuted
        );
    }
    // Pigeonhole with h holes: h + 1 pigeon clauses, each a group over its
    // h holes, and h · (h + 1) · h / 2 exclusion pairs, each a group of two.
    // Every table bounds its support: a pigeon's clause its lower bound of
    // one, and the pairs grow into the holes' cliques and the pigeons' own,
    // since a pigeon in two holes leaves the others too few. The stable
    // candidate's root falls to the double count; a dropped-pigeon
    // candidate's stands, with one clause and one clique the fewer and a
    // bound of nothing over the dropped pigeon's places completing the
    // lower side.
    for h in PIGEONHOLE.sizes {
        let receipt = checked(PIGEONHOLE, h);
        let p = &receipt.preparation;
        let candidates = receipt.candidates;
        assert_eq!((receipt.models, candidates), (1, 4), "holes={h}");
        assert_eq!(count(p, "saturation_units"), candidates, "holes={h}");
        // Every candidate's pigeon clauses and hole pairs, less the one
        // clause each non-minimal candidate drops.
        assert_eq!(
            count(p, "groups"),
            candidates * (h + 1 + h * h * (h + 1) / 2) - (candidates - receipt.models),
            "holes={h}"
        );
        assert_eq!(count(p, "bounds"), 12 * h + 5, "holes={h}");
        assert!(
            count(p, "families") >= candidates,
            "holes={h}: families {}",
            count(p, "families")
        );
        println!(
            "pigeonhole holes={h}: probes={} rounds={} rows={} bounds={} families={} work={} regions={} refuted={}",
            count(p, "probes"),
            count(p, "rounds"),
            count(p, "rows"),
            count(p, "bounds"),
            count(p, "families"),
            count(p, "work"),
            receipt.visited,
            receipt.refuted
        );
    }
}

#[test]
#[ignore = "requires clingo: every stable candidate of the series is refuted at its query root"]
fn g_door_inner_a_stable_candidates_are_root_refuted() {
    for family in [PARITY, TSEITIN, PIGEONHOLE] {
        for size in family.sizes {
            let receipt = checked(family, size);
            assert_eq!(
                count(&receipt.preparation, "root_refuted"),
                receipt.models,
                "{} {size}",
                family.name
            );
        }
    }
}

#[test]
#[ignore = "requires clingo: the inner regions per non-minimal candidate grow polynomially along the series"]
fn g_door_inner_b_inner_regions_grow_polynomially() {
    for family in [PARITY, TSEITIN, PIGEONHOLE] {
        // The smallest size fixes the constant: regions per non-minimal
        // candidate there, over its universal atoms squared. Every larger
        // size must stay within that constant times its own atoms squared,
        // compared exactly by cross-multiplication.
        let mut smallest: Option<(u128, u128, u128)> = None;
        for size in family.sizes {
            let receipt = checked(family, size);
            let root_refuted = count(&receipt.preparation, "root_refuted");
            let nonminimal = u128::from(receipt.candidates - receipt.models);
            assert!(nonminimal > 0, "{} {size}", family.name);
            // Each root-refuted candidate's walk is its one region.
            let regions = u128::from(receipt.visited - root_refuted);
            let value = u128::from(family.universals(size));
            let (regions_0, nonminimal_0, value_0) =
                *smallest.get_or_insert((regions, nonminimal, value));
            println!(
                "{} universals={value}: regions per non-minimal candidate={}",
                family.name,
                regions / nonminimal
            );
            assert!(
                regions * nonminimal_0 * value_0 * value_0
                    <= regions_0 * nonminimal * value * value,
                "{} {size}: {regions} regions over {nonminimal} candidates exceed the quadratic bound set at {value_0} universals",
                family.name
            );
        }
    }
}
