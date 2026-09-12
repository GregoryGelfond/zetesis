//! Source identities and refusal controls for the private occurrence catalog.

use themelios_base::source::{Source, SourceId};
use themelios_program::raise::raise as raise_program;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::parse;

use super::*;
use crate::{ExpansionFailure, ExpansionLimits};

fn source(id: u32, text: &str) -> Source {
    Source::new(SourceId::new(id), text.into()).expect("bounded source")
}

fn budget(limits: ExpansionLimits) -> Budget {
    Budget::new(limits, limits.max_templates)
}

fn include(
    catalog: &mut Catalog,
    parsed: &Parse<ast::Program>,
    budget: &mut Budget,
) -> Result<Program, FormulaFailure> {
    raise(parsed, &mut metadata::Builder::default(), budget, catalog)
}

fn catalog(source: &Source) -> (Catalog, Program) {
    let parsed = parse(source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let mut catalog = Catalog::default();
    let program = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits::default()),
    )
    .unwrap();
    (catalog, program)
}

fn fallback(source: &Source) -> Location {
    Location {
        source: source.id(),
        span: source.span(),
    }
}

#[test]
fn occurrences_keep_original_boolean_element_ranges() {
    let source = source(
        9,
        "p(\"λ\").\r\n%! documented\r\n1{#true;#true:p(\"λ\");#false}1.\r\nq.",
    );
    let (catalog, program) = catalog(&source);
    let variants: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    let boolean = variants
        .into_iter()
        .find(|carrier| has_boolean_choice(carrier.get()))
        .unwrap();
    let locations = boolean_origins(boolean.get()).unwrap();
    let original: Vec<_> = locations
        .iter()
        .map(|location| {
            assert_eq!(location.source, source.id());
            source.slice(location.span).unwrap()
        })
        .collect();
    assert_eq!(original, ["#true", "#true:p(\"λ\")", "#false"]);
}

#[test]
fn occurrences_keep_original_documented_rule_ranges() {
    let source = source(9, "p.\r\n%! documented\r\n1{#true}1.\r\nq.");
    let parsed = parse(&source, Dialect::Clingo);
    let original = parsed.tree().statements().nth(1).unwrap();
    let (catalog, _) = catalog(&source);
    let origins: Vec<_> = catalog.variants[0].provenance().origins().collect();
    assert_eq!(
        origins,
        [&Origin::Parsed(
            parsed.location(original.syntax().text_range())
        )]
    );
}

#[test]
fn occurrences_retain_the_upstream_documentation() {
    let source = source(9, "%! first line\r\n%! second line\r\n1{#true}1.");
    let (catalog, _) = catalog(&source);
    assert_eq!(
        catalog.variants[0]
            .provenance()
            .annotations()
            .doc()
            .collect::<Vec<_>>(),
        // The upstream doc view retains the CR bytes; zetesis does not rewrite it.
        ["first line\r\nsecond line\r"]
    );
}

#[test]
fn repeated_elements_keep_separate_occurrence_ranges() {
    let source = source(9, "2{#true;#true}2.");
    let (catalog, program) = catalog(&source);
    let variants: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(boolean_origins(variants[0].get()).unwrap().len(), 2);
}

#[test]
fn repeated_whole_rules_keep_independent_scopes() {
    let source = source(9, "1{#true}1. 1{#true}1.");
    let (catalog, program) = catalog(&source);
    assert_eq!(
        program.statements().count(),
        1,
        "negative control: the dependency merged whole rules"
    );
    let variants: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(variants.len(), 2);
    let first = boolean_origins(variants[0].get()).unwrap();
    let second = boolean_origins(variants[1].get()).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert!(first.is_disjoint(&second));
}

#[test]
fn equal_raised_rules_keep_unequal_occurrence_counts() {
    for text in ["1{#true}1. 1{#true;#true}1.", "1{#true;#true}1. 1{#true}1."] {
        let source = source(9, text);
        let (catalog, program) = catalog(&source);
        assert_eq!(
            program.statements().count(),
            1,
            "negative control: merged content cannot recover multiplicity"
        );
        let mut counts: Vec<_> = catalog
            .statements(&program, fallback(&source))
            .map(|carrier| boolean_origins(carrier.unwrap().get()).unwrap().len())
            .collect();
        counts.sort_unstable();
        assert_eq!(counts, [1, 2]);
    }
}

#[test]
fn equal_rules_in_separate_sources_keep_source_identity() {
    let first = source(9, "1{#true}1.");
    let second = source(10, "1{#true}1.");
    let mut catalog = Catalog::default();
    let mut budget = budget(ExpansionLimits::default());
    let mut statements = Vec::new();
    for source in [&first, &second] {
        let parsed = parse(source, Dialect::Clingo);
        let program = include(&mut catalog, &parsed, &mut budget).unwrap();
        statements.extend(program.statements().cloned());
    }
    let program = Program::of_nodes(statements);
    assert_eq!(program.statements().count(), 1);
    let identities: BTreeSet<_> = catalog
        .statements(&program, fallback(&first))
        .map(|carrier| {
            boolean_origins(carrier.unwrap().get())
                .unwrap()
                .first()
                .unwrap()
                .source
        })
        .collect();
    assert_eq!(identities, [first.id(), second.id()].into());
}

#[test]
fn other_head_identities_do_not_enter_the_catalog() {
    for text in ["2{a;a}2.", "1#count{1:#true;1:#true}1.", "a | #true."] {
        let source = source(9, text);
        let parsed = parse(&source, Dialect::Clingo);
        let mut catalog = Catalog::default();
        include(
            &mut catalog,
            &parsed,
            &mut budget(ExpansionLimits {
                max_term_work: 0,
                ..ExpansionLimits::default()
            }),
        )
        .unwrap();
        assert!(catalog.variants.is_empty());
        let program = raise_program(&parsed).into_program();
        assert_eq!(
            catalog
                .statements(&program, fallback(&source))
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn a_missing_catalog_refuses_boolean_source_rules() {
    let source = source(9, "1{#true}1.");
    let (_, program) = catalog(&source);
    assert!(matches!(
        Catalog::default()
            .statements(&program, fallback(&source))
            .next()
            .unwrap(),
        Err(FormulaFailure::ChoiceSource { .. })
    ));
}

#[test]
fn a_partial_catalog_refuses_merged_rule_origins() {
    let first = source(9, "1{#true}1.");
    let second = source(10, "1{#true;#true}1.");
    let (catalog, first_program) = catalog(&first);
    let second_program = raise_program(&parse(&second, Dialect::Clingo)).into_program();
    let merged = Program::of_nodes(
        first_program
            .statements()
            .chain(second_program.statements())
            .cloned(),
    );
    assert!(matches!(
        catalog
            .statements(&merged, fallback(&first))
            .next()
            .unwrap(),
        Err(FormulaFailure::ChoiceSource { .. })
    ));
}

#[test]
fn constructed_rule_origins_cannot_replace_a_catalog() {
    let source = source(9, "1{#true}1.");
    let (catalog, program) = catalog(&source);
    let constructed = Program::of_nodes(
        program
            .statements()
            .map(|carrier| WithProvenance::constructed(carrier.get().clone())),
    );
    assert!(matches!(
        catalog
            .statements(&constructed, fallback(&source))
            .next()
            .unwrap(),
        Err(FormulaFailure::ChoiceSource { .. })
    ));
}

#[test]
fn source_work_refuses_before_retaining_a_variant() {
    let source = source(9, "p. 1{#true}1. q.");
    let parsed = parse(&source, Dialect::Clingo);
    let rule = parsed.tree().statements().nth(1).unwrap();
    let expected = parsed.location(rule.syntax().text_range()).span.len() as usize
        + rule.syntax().descendants().count();
    let mut catalog = Catalog::default();
    let result = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits {
            max_term_work: expected - 1,
            ..ExpansionLimits::default()
        }),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::TermWork, observed, location, ..
    })) if observed == expected as u128 && location == parsed.location(rule.syntax().text_range()))
    );
    assert!(catalog.variants.is_empty());
}

#[test]
fn catalog_work_is_cumulative_across_sources() {
    let first = source(9, "1{#true}1.");
    let second = source(10, first.text());
    let parsed = parse(&first, Dialect::Clingo);
    let expected = first.text().len()
        + parsed
            .tree()
            .statements()
            .next()
            .unwrap()
            .syntax()
            .descendants()
            .count();
    let mut catalog = Catalog::default();
    let mut budget = budget(ExpansionLimits {
        max_term_work: expected,
        ..ExpansionLimits::default()
    });
    include(&mut catalog, &parsed, &mut budget).unwrap();
    let result = include(&mut catalog, &parse(&second, Dialect::Clingo), &mut budget);
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::TermWork, observed, location, ..
    })) if observed == (expected * 2) as u128 && location.source == second.id())
    );
    assert_eq!(catalog.variants.len(), 1);
}

#[test]
fn retained_nodes_are_reserved_before_copying() {
    let source = source(9, "1{#true;#true}1.");
    let parsed = parse(&source, Dialect::Clingo);
    let nodes = parsed
        .tree()
        .statements()
        .next()
        .unwrap()
        .syntax()
        .descendants()
        .count();
    let mut catalog = Catalog::default();
    let result = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits {
            max_values: nodes - 1,
            ..ExpansionLimits::default()
        }),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::Values, observed, ..
    })) if observed == nodes as u128)
    );
    assert!(catalog.variants.is_empty());
}

#[test]
fn occurrence_locations_are_reserved_before_copying() {
    let source = source(9, "1{#true;#true}1.");
    let parsed = parse(&source, Dialect::Clingo);
    let locations = parsed
        .tree()
        .statements()
        .next()
        .unwrap()
        .syntax()
        .descendants()
        .count()
        * 4;
    let mut catalog = Catalog::default();
    let result = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits {
            max_origin_locations: locations - 1,
            ..ExpansionLimits::default()
        }),
    );
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::Origins, observed, ..
    })) if observed == locations as u128)
    );
    assert!(catalog.variants.is_empty());
}

#[test]
fn signed_boolean_elements_keep_their_original_scope() {
    let source = source(9, "1{not #true;not not #false}1.");
    let (catalog, program) = catalog(&source);
    let variants: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    let actual: Vec<_> = boolean_origins(variants[0].get())
        .unwrap()
        .into_iter()
        .map(|location| source.slice(location.span).unwrap())
        .collect();
    assert_eq!(actual, ["not #true", "not not #false"]);
}

#[test]
fn duplicate_scope_refusal_preserves_the_catalog() {
    let source = source(9, "1{#true}1.");
    let parsed = parse(&source, Dialect::Clingo);
    let (mut catalog, program) = catalog(&source);
    let result = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits::default()),
    );
    assert!(matches!(result, Err(FormulaFailure::ChoiceSource { .. })));
    let retained: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(boolean_origins(retained[0].get()).unwrap().len(), 1);
}

#[test]
fn raising_preserves_the_original_program_parts() {
    let source = source(
        9,
        "1{#true}1. #program step(t). 1{#false}1. #program base. 1{#true;#true}1.",
    );
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let (catalog, actual) = catalog(&source);
    let expected = raise_program(&parsed).into_program();
    assert_eq!(actual, expected);
    assert_eq!(actual.parts().count(), 2);
    let locations: Vec<_> = catalog
        .variants
        .iter()
        .map(|entry| {
            entry
                .provenance()
                .origins()
                .find_map(|origin| match origin {
                    Origin::Parsed(location) => Some(*location),
                    _ => None,
                })
                .unwrap()
        })
        .collect();
    assert!(locations.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn metadata_matches_collection_from_the_program() {
    let source = source(
        9,
        "#show z/0. #defined a/0. 1{#true}1. #show. #show z/0. #show 7.",
    );
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let mut actual = metadata::Builder::default();
    let program = raise(
        &parsed,
        &mut actual,
        &mut budget(ExpansionLimits::default()),
        &mut Catalog::default(),
    )
    .unwrap();
    let mut expected = metadata::Builder::default();
    metadata::collect_profile(&program, &mut expected, true).unwrap();
    assert_eq!(actual.finish(), expected.finish());
}

#[test]
fn raise_diagnostics_precede_occurrence_copy_limits() {
    let source = source(9, "1{#true}1. p(2147483648).");
    let parsed = parse(&source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let expected = raise_program(&parsed).diagnostics().to_vec();
    assert!(!expected.is_empty());
    let mut catalog = Catalog::default();
    let error = include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits {
            max_term_work: 0,
            max_values: 0,
            max_origin_locations: 0,
            ..ExpansionLimits::default()
        }),
    )
    .unwrap_err();
    let FormulaFailure::Expansion(ExpansionFailure::Admission(AdmissionFailure::Raise(actual))) =
        error
    else {
        panic!("expected original raise diagnostics: {error}");
    };
    assert_eq!(actual, expected);
    assert!(catalog.variants.is_empty());
}

#[test]
fn unrelated_source_bytes_do_not_charge_choice_copying() {
    let source = source(9, &format!("1{{#true}}1. % {}", "unrelated".repeat(128)));
    let parsed = parse(&source, Dialect::Clingo);
    let statement = parsed.tree().statements().next().unwrap();
    let work = parsed.location(statement.syntax().text_range()).span.len() as usize
        + statement.syntax().descendants().count();
    let mut catalog = Catalog::default();
    include(
        &mut catalog,
        &parsed,
        &mut budget(ExpansionLimits {
            max_term_work: work,
            ..ExpansionLimits::default()
        }),
    )
    .unwrap();
    assert_eq!(catalog.variants.len(), 1);
}
