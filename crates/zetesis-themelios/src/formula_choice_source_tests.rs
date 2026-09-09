//! Source identities and refusal controls for the private occurrence catalog.

use themelios_program::raise::raise;
use themelios_syntax::parse::parse;
use themelios_syntax::token::check_token_source_laws;

use super::*;
use crate::{ExpansionFailure, ExpansionLimits};

fn source(id: u32, text: &str) -> Source {
    Source::new(SourceId::new(id), text.into()).expect("bounded source")
}

fn budget(limits: ExpansionLimits) -> Budget {
    Budget::new(limits, limits.max_templates)
}

fn catalog(source: &Source) -> (Catalog, Program) {
    let parsed = parse(source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let mut catalog = Catalog::default();
    catalog
        .include(source, &parsed, &mut budget(ExpansionLimits::default()))
        .unwrap();
    let raised = raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    (catalog, raised.into_program())
}

fn fallback(source: &Source) -> Location {
    Location {
        source: source.id(),
        span: source.span(),
    }
}

fn window(source: &Source) -> StatementWindow<'_> {
    let parsed = parse(source, Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let statement = parsed
        .tree()
        .statements()
        .find(|statement| {
            let ast::Statement::Rule(rule) = statement else {
                return false;
            };
            matches!(
                rule.head(),
                Some(ast::Head::Aggregate(ast::Aggregate::Set(_)))
            )
        })
        .unwrap();
    StatementWindow {
        lexer: Lexer::new(source, Dialect::Clingo),
        span: parsed.location(statement.syntax().text_range()).span,
    }
}

#[test]
fn window_obeys_public_token_source_laws() {
    let source = source(
        9,
        "p(\"λ\").\r\n%! choice documentation\r\n1{#true;#false:p(\"λ\")}1.\r\nq(\"雪\").",
    );
    assert!(check_token_source_laws(&window(&source)).is_empty());
}

#[test]
fn window_retains_the_original_source_bytes() {
    let source = source(9, "p. 1{#true}1. q.");
    let window = window(&source);
    assert_eq!(window.id(), source.id());
    assert_eq!(window.text(), source.text());
    assert_eq!(
        parse_statement(&window, NestingLimit::DEFAULT)
            .syntax()
            .text(),
        source.text()
    );
}

#[test]
fn selected_tokens_are_unchanged_lexer_tokens() {
    let source = source(9, "p.\r\n1{#true: % comment\r\n p;#false}1.\r\nq.");
    let window = window(&source);
    let mut at = window.span.start();
    let mut last = None;
    while at < window.span.end() {
        let actual = window.token_at(at, LexMode::Normal).unwrap();
        assert_eq!(actual, window.lexer.token_at(at, LexMode::Normal).unwrap());
        assert!(!actual.text.is_empty());
        at = at
            .checked_add(u32::try_from(actual.text.len()).unwrap())
            .unwrap();
        last = Some(actual.kind);
    }
    assert_eq!(at, window.span.end());
    assert_eq!(last, Some(SyntaxKind::DOT));
}

#[test]
fn fragments_keep_original_boolean_element_ranges() {
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
fn fragments_keep_original_documented_rule_ranges() {
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
        catalog.include(source, &parsed, &mut budget).unwrap();
        statements.extend(raise(&parsed).program().statements().cloned());
    }
    let program = Program::of(statements);
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
        catalog
            .include(
                &source,
                &parsed,
                &mut budget(ExpansionLimits {
                    max_term_work: 0,
                    ..ExpansionLimits::default()
                }),
            )
            .unwrap();
        assert!(catalog.variants.is_empty());
        let program = raise(&parsed).into_program();
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
    let second_program = raise(&parse(&second, Dialect::Clingo)).into_program();
    let merged = Program::of(
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
    let constructed = Program::of(
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
fn mismatched_source_identity_refuses_fragment_raising() {
    let first = source(9, "1{#true}1.");
    let second = source(10, first.text());
    let result = Catalog::default().include(
        &second,
        &parse(&first, Dialect::Clingo),
        &mut budget(ExpansionLimits::default()),
    );
    assert!(matches!(result, Err(FormulaFailure::ChoiceSource { .. })));
}

#[test]
fn mismatched_source_bytes_refuse_fragment_raising() {
    let first = source(9, "1{#true}1.");
    let second = source(9, "2{#true}2.");
    let result = Catalog::default().include(
        &second,
        &parse(&first, Dialect::Clingo),
        &mut budget(ExpansionLimits::default()),
    );
    assert!(matches!(result, Err(FormulaFailure::ChoiceSource { .. })));
}

#[test]
fn fragment_work_refuses_before_retaining_a_variant() {
    let source = source(9, "p. 1{#true}1. q.");
    let parsed = parse(&source, Dialect::Clingo);
    let rule = parsed.tree().statements().nth(1).unwrap();
    let expected = source.text().len() * 3 + rule.syntax().descendants().count();
    let mut catalog = Catalog::default();
    let result = catalog.include(
        &source,
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
    let expected = first.text().len() * 3
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
    catalog.include(&first, &parsed, &mut budget).unwrap();
    let result = catalog.include(&second, &parse(&second, Dialect::Clingo), &mut budget);
    assert!(
        matches!(result, Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
        resource: ExpansionResource::TermWork, observed, location, ..
    })) if observed == (expected * 2) as u128 && location.source == second.id())
    );
    assert_eq!(catalog.variants.len(), 1);
}

#[test]
fn retained_nodes_are_reserved_before_fragment_parsing() {
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
    let result = catalog.include(
        &source,
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
fn occurrence_locations_are_reserved_before_parsing() {
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
    let result = catalog.include(
        &source,
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
    let result = catalog.include(&source, &parsed, &mut budget(ExpansionLimits::default()));
    assert!(matches!(result, Err(FormulaFailure::ChoiceSource { .. })));
    let retained: Vec<_> = catalog
        .statements(&program, fallback(&source))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(boolean_origins(retained[0].get()).unwrap().len(), 1);
}
