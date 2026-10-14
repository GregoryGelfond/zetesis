//! The fact arena against the rule form it replaces.
//!
//! Each program is compiled twice on this thread: with every fact as a rule
//! with an empty body (the reference path), and with facts in the arena. Both
//! must publish the same support rounds, evaluate the same exact activity,
//! ground the same theory (atoms, nodes, roots, origins, producers, objectives
//! and projection), defer the same terminal definitions and reconstruct the same
//! answer from them. The receipts may differ only where the arena skips work:
//! `Origins` falls by each fact row's evidence, which the fact rule charged a
//! second time for its own vector.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use zetesis_core::{Model, ValueLimits};
use zetesis_cpu::Cancellation;

use super::{FactBlock, Item};
use crate::formula_ir::testing::with_facts_as_rules;
use crate::formula_support::facts::rounds;
use crate::{
    BundleAdmissionOptions, BundleLimits, ExpansionLimits, FormulaLimits, FormulaMaterialization,
    SourceAdmission, SourceBundle,
};

/// Everything both paths must agree on, rendered for comparison.
#[derive(Debug, Default, PartialEq, Eq)]
struct Observed {
    /// Exact source evaluation: support rounds, atoms and selection.
    source: String,
    /// Full grounding: support rounds and the admitted theory.
    ground: String,
    /// Adaptive grounding: the terminal partition and a reconstruction.
    adaptive: String,
}

/// The expansion receipt of full grounding, and the evidence of the fact rows
/// the arena holds.
#[derive(Debug, Default)]
struct Receipts {
    origins: Option<u128>,
    evidence: u128,
}

fn bundle(path: &Path) -> SourceBundle {
    SourceBundle::load(path, BundleLimits::default())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn prepare(path: &Path) -> Result<crate::PreparedFormulaBundle, String> {
    crate::prepare_bundle_formula(
        bundle(path),
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .map_err(|failure| failure.error().to_string())
}

fn atoms(catalog: &zetesis_core::AtomCatalog) -> String {
    let mut rendered = String::new();
    for atom in catalog.atoms() {
        write!(rendered, "{:?};", atom.to_atom(ValueLimits::default())).unwrap();
    }
    rendered
}

fn grounded(admitted: &crate::AdmittedFormulaBundle) -> String {
    format!(
        "theory {:?}\natoms {}\norigins {:?}\nobjectives {:?}\nobjective origins {:?}\n\
         declarations {:?}\nprojection {:?}\nwarnings {:?}",
        admitted.theory(),
        atoms(admitted.atom_catalog()),
        admitted.formula_origins(),
        admitted.objectives(),
        admitted.objective_origins(),
        admitted.objective_declarations(),
        admitted.projection(),
        admitted.warnings(),
    )
}

fn reconstructed(owner: &crate::TerminalFormula) -> String {
    let mut rendered = format!(
        "deferred {}\nbase atoms {}\nbase theory {:?}\n",
        owner.deferred_templates(),
        atoms(owner.base_atom_catalog()),
        owner.base_theory(),
    );
    let all = Model::from_positions(
        owner.base_atom_catalog(),
        0..owner.base_atom_catalog().atoms().len(),
    )
    .unwrap();
    match owner
        .reconstruction()
        .map_err(|error| error.to_string())
        .and_then(|mut cursor| {
            cursor
                .reconstruct(&all, &Cancellation::default())
                .map_err(|error| error.to_string())
        }) {
        Ok(model) => {
            for atom in model.atoms() {
                write!(rendered, "{:?};", atom.to_atom(ValueLimits::default())).unwrap();
            }
        }
        Err(error) => write!(rendered, "refused {error}").unwrap(),
    }
    rendered
}

/// A refusal's text. A work ceiling's crossing amount is the work charged up
/// to the operation that crossed it, which fact products and fact rules charge
/// differently by design; of a work refusal only the resource and its limit
/// are compared.
fn refused(error: &crate::FormulaFailure) -> String {
    match error.cause() {
        crate::FormulaFailure::Limit {
            resource: crate::FormulaResource::Work,
            limit,
            ..
        } => format!("refused formula work limit {limit} exceeded"),
        _ => format!("refused {error}"),
    }
}

fn observe(path: &Path) -> (Observed, Receipts) {
    let mut observed = Observed::default();
    let mut receipts = Receipts::default();
    let (source, published) = rounds::capture(|| match prepare(path) {
        Err(error) => format!("refused {error}"),
        Ok(prepared) => match prepared.try_source() {
            Err(failure) => refused(failure.error()),
            Ok(SourceAdmission::NotApplicable(_)) => "not applicable".to_owned(),
            Ok(SourceAdmission::Admitted(source)) => format!(
                "atoms {}\nselected {:?}",
                atoms(source.atom_catalog()),
                source.selected_positions()
            ),
        },
    });
    observed.source = format!("{source}\nrounds {published:?}");
    let (ground, published) = rounds::capture(|| match prepare(path) {
        Err(error) => format!("refused {error}"),
        Ok(prepared) => {
            receipts.evidence = evidence(&prepared);
            match prepared.ground() {
                Err(failure) => refused(failure.error()),
                Ok(admitted) => {
                    receipts.origins = Some(admitted.expansion_usage().origin_locations as u128);
                    grounded(&admitted)
                }
            }
        }
    });
    observed.ground = format!("{ground}\nrounds {published:?}");
    observed.adaptive = match prepare(path) {
        Err(error) => format!("refused {error}"),
        Ok(prepared) => match prepared.ground_adaptive() {
            Err(failure) => refused(failure.error()),
            Ok(FormulaMaterialization::Complete(admitted)) => grounded(&admitted),
            Ok(FormulaMaterialization::Terminal(owner)) => reconstructed(&owner),
        },
    };
    (observed, receipts)
}

/// The evidence the arena's fact rows carry: what their fact rules' own
/// origins vectors charged.
fn evidence(prepared: &crate::PreparedFormulaBundle) -> u128 {
    prepared
        .preparation()
        .program
        .items()
        .iter()
        .map(|item| match item {
            Item::Rule(..) => 0,
            Item::Facts(product) => product.rows().map(|row| row.evidence().len() as u128).sum(),
        })
        .sum()
}

/// Compare the two paths on `path`, and return its fact rows' evidence.
fn agree(path: &Path) -> u128 {
    let (reference, rule_receipts) = with_facts_as_rules(|| observe(path));
    let (actual, receipts) = observe(path);
    assert_eq!(rule_receipts.evidence, 0, "{}", path.display());
    assert_eq!(actual.source, reference.source, "{}", path.display());
    assert_eq!(actual.ground, reference.ground, "{}", path.display());
    assert_eq!(actual.adaptive, reference.adaptive, "{}", path.display());
    if let (Some(rule), Some(arena)) = (rule_receipts.origins, receipts.origins) {
        assert_eq!(rule - arena, receipts.evidence, "{}", path.display());
    }
    receipts.evidence
}

fn written(directory: &Path, name: &str, text: &str) -> PathBuf {
    let path = directory.join(format!("{name}.lp"));
    std::fs::write(&path, text).unwrap();
    path
}

/// The lines `line` writes for each of `count` positions.
fn lines(count: usize, line: impl Fn(&mut String, usize) -> std::fmt::Result) -> String {
    let mut text = String::new();
    for position in 0..count {
        line(&mut text, position).unwrap();
    }
    text
}

/// The fact shapes of the per-fact measurements, at `k` facts.
fn shapes(k: usize) -> Vec<(&'static str, String)> {
    let facts = |name: &str| lines(k, |text, i| writeln!(text, "{name}({i})."));
    let fchoice = format!("{}{{ q(X) }} :- p(X).\n", facts("p"));
    vec![
        ("bare", facts("p")),
        (
            "preds4",
            lines(k, |text, i| writeln!(text, "p{}({i}).", i % 4)),
        ),
        ("copy", format!("{}q(X) :- p(X).\n", facts("p"))),
        (
            "join",
            lines(k / 2, |text, i| writeln!(text, "a({i},{}).", i % 7))
                + &lines(7, |text, i| writeln!(text, "b({i},{}).", i + 1))
                + "c(X,Z) :- a(X,Y), b(Y,Z).\n",
        ),
        (
            "wide",
            lines(k, |text, i| {
                writeln!(text, "e({i}, c{i}, \"s{i}\", f({i}, g({i}))).")
            }),
        ),
        (
            "fcmp",
            format!("{}r(X) :- p(X), X > {}.\n", facts("p"), k - 3),
        ),
        (
            "frange",
            format!("p(0..{}).\nr(X) :- p(X), X > {}.\n", k - 1, k - 3),
        ),
        (
            "orig",
            format!(
                "{}q(X) :- p(X), not r(X).\nr(X) :- p(X), X > {}.\n",
                facts("p"),
                k - 3
            ),
        ),
        ("fchoice", fchoice.clone()),
        (
            "fcycle",
            format!(
                "{}a(X) :- p(X), b(X).\nb(X) :- a(X).\nb(X) :- p(X), X < 3.\n",
                facts("p")
            ),
        ),
        ("fmin", format!("{fchoice}#minimize {{ X : q(X) }}.\n")),
        ("fproj", format!("{fchoice}#project q/1.\n")),
    ]
}

/// Facts the ordering rule must keep in place: duplicates, alternatives of
/// several arities, multi-column products, empty and nullary families,
/// classical negation, interleaved rules and directives.
const EDGES: &[(&str, &str)] = &[
    (
        "duplicates",
        "p(1). p(1;1). p(1). q(1) :- p(1). p(1) :- q(1). r(X) :- p(X).",
    ),
    ("arities", "p(1;a,b). p(2). q(X) :- p(X). s(X,Y) :- p(X,Y)."),
    (
        "columns",
        "p(1..3, a;b). p(4, c). q(X,Y) :- p(X,Y), X > 1. r(Y) :- p(_,Y).",
    ),
    ("empty", "p(1..0). p(1). q(X) :- p(X)."),
    ("nullary", "a. b. c :- a, b. d :- not c."),
    (
        "strings",
        "name(\"alice\"). name(bob). name(\"alice\"). r(X) :- name(X).",
    ),
    ("signs", "-p(1). p(2). q(X) :- p(X). r(X) :- -p(X)."),
    (
        "interleaved",
        "p(1). q(X) :- p(X). p(2). p(3). #show q/1. r(X) :- q(X). p(4). p(5;6).",
    ),
    ("constants", "#const n = 3. p(1..n). q(X) :- p(X), X != n."),
    ("sentinels", "p(-1). p(#inf). p(#sup). q(X) :- p(X)."),
    (
        "keys",
        "p(1,a). p(2,b). p(1,a). :- p(X,Y), p(X,Z), Y != Z. q(X) :- p(X,_).",
    ),
    (
        "terminal",
        "p(1). q(2). d(5). d(X) :- p(X). d(X) :- q(X). e(X) :- d(X), X > 1.",
    ),
    (
        "objective",
        "w(1,10). w(2,20). w(3,5). { s(X) } :- w(X,_). \
         #minimize { C,X : s(X), w(X,C) }. :- not s(1).",
    ),
    (
        "projection",
        "d(1..3). e(a;b). { s(X,Y) : d(X) } :- e(Y). #project s/2.",
    ),
];

#[test]
fn generated_fact_shapes_agree_with_fact_rules() {
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in shapes(40) {
        let path = written(directory.path(), name, &text);
        assert!(agree(&path) > 0, "{name} holds facts");
    }
}

#[test]
fn edge_case_facts_agree_with_fact_rules() {
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in EDGES {
        agree(&written(directory.path(), name, text));
    }
}

fn programs(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            programs(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "lp") {
            found.push(path);
        }
    }
}

/// Every program of the examples tree: the correctness and scalability corpus
/// and the book's examples, each loaded as a bundle with its includes.
#[test]
fn corpus_programs_agree_with_fact_rules() {
    let mut found = Vec::new();
    programs(&zetesis_test_support::repository::examples(), &mut found);
    found.sort();
    assert!(found.len() > 100, "{}", found.len());
    let mut evidence = 0;
    for path in &found {
        evidence += agree(path);
    }
    assert!(evidence > 0);
}

#[test]
fn the_fact_arena_is_leased_for_everything_it_retains() {
    // Every vector the arena grows is reserved through its lease, so the
    // lease holds exactly the arena's retained capacity.
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in shapes(200) {
        let path = written(directory.path(), name, &text);
        let prepared = prepare(&path).unwrap();
        let (leased, retained) = prepared.preparation().program.items().arena_receipt();
        assert!(retained > 0, "{name}");
        assert_eq!(leased, Some(retained), "{name}");
    }
}

#[test]
fn a_run_holds_consecutive_one_row_statements_of_one_predicate() {
    let directory = tempfile::tempdir().unwrap();
    // Statements compile in the program's canonical order: the p facts first.
    let path = written(
        directory.path(),
        "runs",
        "q(1). p(1). p(2). p(3). p(4;5). r(X) :- p(X).",
    );
    let prepared = prepare(&path).unwrap();
    let products: Vec<_> = prepared
        .preparation()
        .program
        .items()
        .blocks()
        .map(FactBlock::len)
        .collect();
    // p(1), p(2) and p(3) share a run; p(4;5) is two argument alternatives,
    // a product each; q(1) starts a run of its own.
    assert_eq!(products, [3, 1, 1, 1]);
}

#[test]
fn a_product_admits_each_column_value_once() {
    let directory = tempfile::tempdir().unwrap();
    let path = written(directory.path(), "product", "p(1..4, 1..3).");
    let prepared = prepare(&path).unwrap();
    let product = prepared
        .preparation()
        .program
        .items()
        .blocks()
        .next()
        .unwrap();
    assert_eq!(product.len(), 12);
    assert_eq!(product.cells().len(), 7);
}

/// The positive-source certificate's verdict, from the compiled program.
fn positive(source: &str) -> bool {
    let preparation = crate::formula_support::testing::prepare(source);
    let mut counters = crate::formula_support::Counters::default();
    let components = preparation
        .catalog
        .component_view(&preparation.limits, &mut counters, preparation.location)
        .unwrap();
    crate::formula_domains::PositiveSource::check(
        &preparation.program,
        components,
        &preparation.limits,
        &mut counters,
        preparation.location,
    )
    .unwrap()
    .is_some()
}

#[test]
fn the_positive_certificate_reads_fact_cells_as_fact_heads() {
    let long = "x".repeat(5_000);
    for (source, verdict) in [
        ("p(1). p(a). p(\"s\"). q(X) :- p(X).".to_owned(), true),
        ("p(1..3, b). q(X) :- p(X, b).".to_owned(), true),
        ("p(f(1)). q(X) :- p(X).".to_owned(), false),
        ("p((1, 2)). q(X) :- p(X).".to_owned(), false),
        (format!("p(\"{long}\"). q(X) :- p(X)."), false),
        (format!("p({long}). q(X) :- p(X)."), false),
    ] {
        assert_eq!(positive(&source), verdict, "{source}");
        assert_eq!(
            with_facts_as_rules(|| positive(&source)),
            verdict,
            "{source}"
        );
    }
}

/// The typed failure, and its site, of preparing `source` under `options`
/// and `limits`.
fn refusal(
    source: &str,
    options: crate::AdmissionOptions,
    limits: &FormulaLimits,
) -> (String, Option<crate::ProgramSite>) {
    let error = crate::prepare_formula(source.into(), options, ExpansionLimits::default(), *limits)
        .expect_err("refused");
    (format!("{error:?}"), error.site())
}

#[test]
fn a_refused_fact_keeps_its_typed_failure_and_site() {
    let mut narrow = crate::AdmissionOptions::default();
    narrow.core_limits.max_predicate_arity = 1;
    let few = FormulaLimits {
        max_domain_values: 3,
        ..FormulaLimits::default()
    };
    for (source, options, limits) in [
        // The arity ceiling, once per product, at the product's statement.
        (
            "p(1). q(X) :- p(X). r(1,2;3,4).",
            narrow,
            FormulaLimits::default(),
        ),
        // The domain ceiling, at the statement whose value crosses it.
        (
            "p(1). p(2). q(X) :- p(X). p(3..5).",
            crate::AdmissionOptions::default(),
            few,
        ),
    ] {
        let reference = with_facts_as_rules(|| refusal(source, options, &limits));
        assert_eq!(refusal(source, options, &limits), reference, "{source}");
        assert!(reference.1.is_some(), "{source}");
    }
}
