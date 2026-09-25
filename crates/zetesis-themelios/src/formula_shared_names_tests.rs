//! Rule and objective scopes refer to one canonical predicate vocabulary.
use super::*;
use crate::ExpansionLimits;
use themelios_base::source::{Source, SourceId};

#[test]
fn objective_scopes_share_canonical_predicate_text() {
    // The rule's body pattern and the objective's condition both name `p`.
    // The element's ranged weight puts it in a scope of its own, which
    // compiles under the same canonical authority, so both borrowed views
    // resolve the same text without retaining their own name allocation.
    let source = Source::new(
        SourceId::new(113),
        "q(N):-p(N).#minimize{1..2@1:p(N)}.".into(),
    )
    .unwrap();
    let parsed =
        themelios_syntax::parse::parse(&source, themelios_syntax::dialect::Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let raised = themelios_program::raise::raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let mut metadata = crate::metadata::Builder::default();
    crate::metadata::collect_profile(raised.program(), &mut metadata, true).unwrap();
    let metadata = metadata.finish(location).unwrap();
    let prepared = PreparationContext {
        options: AdmissionOptions::default(),
        budget: &mut budget,
        catalog: &mut catalog,
        work: GroundingWork::new(&FormulaLimits::default(), &mut counters, location),
    }
    .prepare(
        raised.program(),
        metadata.project_selection().clone(),
        &crate::formula_choice_source::Catalog::default(),
    )
    .unwrap();
    let limits = FormulaLimits::default();
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let view = catalog
        .component_view(&limits, &mut counters, location)
        .unwrap()
        .unwrap();
    let mut predicate = |pattern: crate::formula_support::components::Pattern| {
        pattern
            .get(view, &limits, &mut counters, location)
            .unwrap()
            .predicate()
    };
    let rule_pattern = prepared
        .rules
        .iter()
        .flat_map(|rule| rule.body.iter())
        .filter_map(|literal| match literal {
            LiteralIr::Atom(_, pattern) => Some(predicate(*pattern)),
            _ => None,
        })
        .find(|predicate| predicate.name() == "p")
        .unwrap();
    let objective_pattern = prepared.objectives[0]
        .positive
        .iter()
        .map(|pattern| predicate(*pattern))
        .find(|predicate| predicate.name() == "p")
        .unwrap();
    assert_eq!(rule_pattern, objective_pattern);
    assert!(std::ptr::eq(rule_pattern.name(), objective_pattern.name()));
}
