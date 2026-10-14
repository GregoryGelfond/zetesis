//! Exact full inspection, canonical preflight, and bounded lazy fact readers.

use super::summary::{PreparationFacts, Summary};
use crate::expansion::Budget;
use crate::formula_ir::testing::with_facts_as_rules;
use crate::formula_support::testing::prepare;
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};

const SHAPES: &[&str] = &[
    "p(1..4). p(3..6). q(X) :- p(X).",
    "p(-2..2, a;b). -p(-2..2,a).",
    "p(a;b,c;0). q. {r} :- p(a).",
    "p(f((1;2))). p(f(1)). p(f(2)).",
    "p((a,); f(1); -f(1); #inf; #sup; \"text\").",
    "#const bound=4. p(1..bound). p(2+1). #show p/1.",
    "p(4..2). q(a..z). r. s(1..2).",
    "p(1..3). {q(X)} :- p(X). r(X) :- q(X), not s(X). s(X) :- p(X), not r(X).",
    "p(1..3). q(X) | r(X) :- p(X).",
    "p(1..3). {q(X)} :- p(X). #minimize {X:q(X)}.",
    "edge(1,2). edge(1,3). edge(2,3). col(a;b). node(1..3). 1{color(X,C):col(C)}1 :- node(X). :- edge(X,Y),color(X,C),color(Y,C).",
];

fn exact(left: &themelios_program::program::Program, right: &themelios_program::program::Program) {
    assert_eq!(left, right);
    // Program's deterministic tree Debug includes nested provenance, unlike its
    // intentional provenance-blind Eq. No hash-index implementation is rendered.
    assert_eq!(format!("{left:?}"), format!("{right:?}"));
}

#[test]
fn full_inspection_preserves_normalized_content_and_nested_evidence() {
    for source in SHAPES {
        let actual = prepare(source);
        let reference = with_facts_as_rules(|| prepare(source));
        assert!(!actual.program.analyzed.is_materialized(), "{source}");
        exact(
            actual.program.analyzed.full_program(),
            reference.program.analyzed.full_program(),
        );
        assert_eq!(
            actual
                .program
                .analyzed
                .full_analysis(&actual.program.analysis),
            reference
                .program
                .analyzed
                .full_analysis(&reference.program.analysis),
            "{source}"
        );
    }
}

#[test]
fn summary_preserves_full_graph_and_classes() {
    for source in SHAPES {
        let prepared = prepare(source);
        let full = prepared
            .program
            .analyzed
            .full_analysis(&prepared.program.analysis);
        assert_eq!(
            prepared.program.analysis.dependencies(),
            full.dependencies(),
            "{source}"
        );
        assert_eq!(
            prepared.program.analysis.classes(),
            full.classes(),
            "{source}"
        );
    }
}

#[test]
fn internal_class_reads_and_debug_leave_full_view_unbuilt() {
    let parsed = crate::ParsedSource::new(
        "p(1..100). {q(X)} :- p(X).".into(),
        crate::AdmissionOptions::default(),
    )
    .unwrap();
    let prepared = parsed
        .prepare_formula(ExpansionLimits::default(), FormulaLimits::default())
        .unwrap();
    let summary = &prepared.preparation().program.analyzed;
    assert_eq!(summary.program().statements().count(), 2);
    assert!(!summary.is_materialized());
    let classes = prepared.source_classes().clone();
    let before = format!("{prepared:?}");
    assert!(!summary.is_materialized());
    assert_eq!(prepared.analyzed_program().statements().count(), 101);
    assert_eq!(prepared.source_analysis().classes(), &classes);
    assert!(summary.is_materialized());
    assert_eq!(
        format!("{prepared:?}"),
        before,
        "Debug is independent of inspection history"
    );
    assert!(std::ptr::eq(
        prepared.analyzed_program(),
        prepared.analyzed_program()
    ));
    assert!(std::ptr::eq(
        prepared.source_analysis(),
        prepared.source_analysis()
    ));
}

#[test]
fn analysis_ceilings_use_canonical_rows_in_full_statement_order() {
    let prepared = prepare("p(1..3). p(2..4). p(X) :- q(X). q(1;2). :- p(3),q(2).");
    let summary = &prepared.program.analyzed;
    let full = summary.full_program();
    for node_limit in 0..45 {
        for edge_limit in [0, 1, 2, 3, 20] {
            let limits = FormulaLimits {
                max_analysis_nodes: node_limit,
                max_analysis_edges: edge_limit,
                ..FormulaLimits::default()
            };
            let actual = summary.analyze(
                &limits,
                &mut Budget::new(ExpansionLimits::default(), usize::MAX),
                ProgramSite::program(),
            );
            let expected = super::analyze(
                full,
                &limits,
                &mut Budget::new(ExpansionLimits::default(), usize::MAX),
                ProgramSite::program(),
            );
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual.dependencies(), expected.dependencies());
                    assert_eq!(actual.classes(), expected.classes());
                }
                (Err(actual), Err(expected)) => {
                    assert!(matches!(
                        actual,
                        FormulaFailure::Limit {
                            resource: FormulaResource::AnalysisNodes
                                | FormulaResource::AnalysisEdges,
                            ..
                        }
                    ));
                    assert_eq!(
                        format!("{actual:?}"),
                        format!("{expected:?}"),
                        "nodes={node_limit}, edges={edge_limit}"
                    );
                }
                _ => {
                    panic!("different preflight outcome at nodes={node_limit}, edges={edge_limit}")
                }
            }
        }
    }
}

#[test]
fn fact_reader_retains_all_rows_and_competing_producers() {
    use themelios_program::symbol::{Name, Sign, Signature, Symbol};
    let prepared = prepare("p(1..3). p(2..4). p(3) :- q. q. fixed(1..3). {fixed(4)}.");
    let summary = &prepared.program.analyzed;
    let reader = PreparationFacts::new(summary);
    let mut facts = crate::formula_rewrite::FixedFacts::summary(&reader);
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut work = zetesis_domain::KeyWork::new(10_000);
    let Ok(facts) = facts.read(&mut work, &mut budget, ProgramSite::program()) else {
        panic!("complete fact reading");
    };
    let p = Signature {
        name: Name::new("p").unwrap(),
        arity: 1,
        sign: Sign::Positive,
    };
    let fixed = Signature {
        name: Name::new("fixed").unwrap(),
        arity: 1,
        sign: Sign::Positive,
    };
    assert!(
        facts.values(&p, 0, &mut work).unwrap().is_none(),
        "ordinary rule is a competing producer"
    );
    assert!(
        facts.values(&fixed, 0, &mut work).unwrap().is_none(),
        "choice is a competing producer"
    );
    let complete = prepare("p(1..3). p(2..4).");
    let reader = PreparationFacts::new(&complete.program.analyzed);
    let mut facts = crate::formula_rewrite::FixedFacts::summary(&reader);
    let Ok(facts) = facts.read(&mut work, &mut budget, ProgramSite::program()) else {
        panic!("complete fact reading");
    };
    assert_eq!(
        facts.values(&p, 0, &mut work).unwrap().unwrap(),
        [
            Symbol::Number(1),
            Symbol::Number(2),
            Symbol::Number(3),
            Symbol::Number(4)
        ]
        .iter()
        .collect::<Vec<_>>()
    );
}

#[test]
fn fact_reader_distinguishes_optional_stop_from_expansion_refusal() {
    let prepared = prepare("p(1..4).");
    let summary = &prepared.program.analyzed;
    let reader = PreparationFacts::new(summary);
    let mut facts = crate::formula_rewrite::FixedFacts::summary(&reader);
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    assert!(matches!(
        facts.read(
            &mut zetesis_domain::KeyWork::new(0),
            &mut budget,
            ProgramSite::program()
        ),
        Err(crate::formula_rewrite::FactFailure::Stopped(_))
    ));
    let reader = PreparationFacts::new(summary);
    let mut facts = crate::formula_rewrite::FixedFacts::summary(&reader);
    let mut budget = Budget::new(
        ExpansionLimits {
            max_term_work: 0,
            ..ExpansionLimits::default()
        },
        usize::MAX,
    );
    assert!(matches!(
        facts.read(
            &mut zetesis_domain::KeyWork::new(10_000),
            &mut budget,
            ProgramSite::program()
        ),
        Err(crate::formula_rewrite::FactFailure::Source(
            FormulaFailure::Expansion(_)
        ))
    ));
    assert!(!summary.is_materialized());
}

#[test]
fn completed_view_has_no_recipe_or_reconstruction() {
    let prepared = prepare("p(1..3).");
    let program = prepared.program.analyzed.full_program().clone();
    let analysis = themelios_analysis::Analysis::of(&program);
    let summary = Summary::complete(program.clone());
    assert!(summary.is_materialized());
    exact(summary.full_program(), &program);
    assert!(std::ptr::eq(
        summary.full_analysis(&analysis),
        std::ptr::from_ref(&analysis)
    ));
}

#[test]
fn public_inspection_cannot_waive_full_preparation_charges() {
    let mut fresh = prepare("p(1..8). d(X) :- p(X).");
    let mut inspected = prepare("p(1..8). d(X) :- p(X).");
    inspected.program.analyzed.full_program();
    fresh
        .program
        .analyzed
        .materialize(&mut fresh.budget, fresh.location)
        .unwrap();
    inspected
        .program
        .analyzed
        .materialize(&mut inspected.budget, inspected.location)
        .unwrap();
    assert_eq!(fresh.budget.usage(), inspected.budget.usage());
    let charged = fresh.budget.usage();
    fresh
        .program
        .analyzed
        .materialize(&mut fresh.budget, fresh.location)
        .unwrap();
    assert_eq!(fresh.budget.usage(), charged);
}

#[test]
fn fact_reader_merges_generated_and_arena_facts_in_program_order() {
    let prepared = prepare("p(a). p(f((1;2))). p(f(1)). p(z).");
    let summary = &prepared.program.analyzed;
    let reader = PreparationFacts::new(summary);
    let atoms = reader
        .atoms(
            &mut Budget::new(ExpansionLimits::default(), usize::MAX),
            ProgramSite::program(),
        )
        .unwrap();
    let full: Vec<_> = summary
        .full_program()
        .statements()
        .filter_map(|statement| crate::fact_expansion::fact_atom(statement.get()))
        .collect();
    assert_eq!(atoms.iter().collect::<Vec<_>>(), full);
    assert!(summary.producers().next().is_none());
}

#[test]
fn fact_reader_matches_full_tuples_opacity_and_stop_steps() {
    use std::collections::BTreeSet;
    use themelios_program::program::Statement;
    use zetesis_domain::{FactIndex, KeyWork};

    for source in SHAPES {
        let prepared = prepare(source);
        let summary = &prepared.program.analyzed;
        let reader = PreparationFacts::new(summary);
        let atoms = reader
            .atoms(
                &mut Budget::new(ExpansionLimits::default(), usize::MAX),
                ProgramSite::program(),
            )
            .unwrap();
        let full = summary.full_program();
        let signatures: BTreeSet<_> = full
            .statements()
            .flat_map(|statement| match statement.get() {
                Statement::Rule(rule) => rule.head_signatures().collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        let count = u64::try_from(full.statements().count()).unwrap();
        for limit in 0..=count {
            let mut actual_work = KeyWork::new(limit);
            let mut expected_work = KeyWork::new(limit);
            let actual = FactIndex::read_parts(summary.producers(), atoms, &mut actual_work);
            let expected = FactIndex::read(full, &mut expected_work);
            assert_eq!(
                actual_work.steps(),
                expected_work.steps(),
                "{source}, {limit}"
            );
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    let mut actual_queries = KeyWork::new(u64::MAX);
                    let mut expected_queries = KeyWork::new(u64::MAX);
                    for signature in &signatures {
                        assert_eq!(
                            actual.tuples(signature, &mut actual_queries).unwrap(),
                            expected.tuples(signature, &mut expected_queries).unwrap(),
                            "{source}, {signature:?}"
                        );
                    }
                    assert_eq!(actual_queries.steps(), expected_queries.steps());
                }
                (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                _ => panic!("different fact-reader stop at {limit}: {source}"),
            }
        }
    }
}

#[test]
fn inspection_cannot_waive_full_preparation_refusal() {
    for bytes in [0, 1, 64, 1_024] {
        let fresh = prepare("p(1..8). d(X) :- p(X).");
        let inspected = prepare("p(1..8). d(X) :- p(X).");
        inspected.program.analyzed.full_program();
        let limits = ExpansionLimits {
            max_scalar_bytes: bytes,
            ..ExpansionLimits::default()
        };
        let mut fresh_budget = Budget::new(limits, usize::MAX);
        let mut inspected_budget = Budget::new(limits, usize::MAX);
        let fresh_result = fresh
            .program
            .analyzed
            .materialize(&mut fresh_budget, fresh.location);
        let inspected_result = inspected
            .program
            .analyzed
            .materialize(&mut inspected_budget, inspected.location);
        assert_eq!(fresh_budget.usage(), inspected_budget.usage());
        match (fresh_result, inspected_result) {
            (Ok(fresh), Ok(inspected)) => exact(fresh, inspected),
            (Err(fresh), Err(inspected)) => {
                assert_eq!(format!("{fresh:?}"), format!("{inspected:?}"));
            }
            _ => panic!("inspection changed full-view refusal at {bytes}"),
        }
    }
}
