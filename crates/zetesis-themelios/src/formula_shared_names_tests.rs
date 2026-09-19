//! One compilation mints one name per predicate, inside objective scopes too.
use super::*;
use crate::ExpansionLimits;
use themelios_base::source::{Source, SourceId};

#[test]
fn an_objective_scope_names_predicates_by_the_compilation_allocation() {
    // The rule's body pattern and the objective's condition both name `p`.
    // The element's ranged weight puts it in a scope of its own, which
    // compiles under the compilation's shared names, so the two patterns
    // hold one allocation and identity compares by pointer.
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
    let prepared = prepare(
        raised.program(),
        &crate::formula_choice_source::Catalog::default(),
        AdmissionOptions::default(),
        &FormulaLimits::default(),
        &mut budget,
        Location {
            source: source.id(),
            span: source.span(),
        },
    )
    .unwrap();
    let rule_pattern = prepared
        .rules
        .iter()
        .flat_map(|rule| rule.body.iter())
        .find_map(|literal| match literal {
            LiteralIr::Atom(_, pattern) if pattern.predicate().name() == "p" => {
                Some(pattern.predicate())
            }
            _ => None,
        })
        .unwrap();
    let objective_pattern = prepared.objectives[0]
        .positive
        .iter()
        .find(|pattern| pattern.predicate().name() == "p")
        .unwrap()
        .predicate();
    assert!(rule_pattern.shares_name(objective_pattern));
}
