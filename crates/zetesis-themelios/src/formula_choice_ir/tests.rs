//! Constructed counted entries use the same preparation and grounding path.

use crate::ProgramSite;
use themelios_base::source::{Source, SourceId};
use themelios_program::program::{
    Body, Choice, ChoiceElement, Condition, DefaultNegation, Guard, Head, Literal, LiteralInner,
    Program, Relation, Rule,
};
use themelios_program::symbol::Symbol;
use themelios_program::term::Term;

use crate::expansion::Budget;
use crate::formula_ir::PreparationContext;
use crate::formula_support::{Counters, GroundingWork, SupportCatalog};
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits};

fn choice(count: usize, bound: i32) -> Rule {
    let element = ChoiceElement::new(
        Literal {
            negation: DefaultNegation::None,
            inner: LiteralInner::True,
        },
        Condition::default(),
    );
    Rule::new(
        Head::Choice(Choice::new(
            None,
            std::iter::repeat_n(element, count),
            Some(Guard {
                relation: Some(Relation::Eq),
                term: Term::Symbolic(Symbol::Number(bound)),
            }),
        )),
        Body::default(),
    )
}

fn model_count(source: &Program) -> usize {
    let fallback = Source::new(SourceId::new(9), String::new()).unwrap();
    let location = ProgramSite::source(themelios_base::span::Location {
        source: fallback.id(),
        span: fallback.span(),
    });
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100);
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let prepared = PreparationContext {
        options: AdmissionOptions::default().into(),
        budget: &mut budget,
        catalog: &mut catalog,
        work: GroundingWork::new(&limits, &mut counters, location),
    }
    .prepare(source, crate::ProjectSelection::default())
    .unwrap();
    let compiled = crate::formula_ground::ground(
        crate::formula::Preparation {
            catalog,
            accounting: counters.into_accounting(),
            program: prepared,
            budget,
            limits,
            options: crate::grounding_options::Execution::default(),
            location,
        },
        None,
        None,
    )
    .unwrap();
    assert!(compiled.atoms.atoms().is_empty());
    let mut search = zetesis_sat::StableModels::new(
        &compiled.theory,
        zetesis_sat::Limits::default(),
        zetesis_cpu::Cancellation::default(),
    )
    .unwrap();
    let count = search.by_ref().map(Result::unwrap).count();
    assert!(search.exhausted());
    count
}

#[test]
fn constructed_boolean_entries_do_not_need_parsed_origins() {
    assert_eq!(model_count(&Program::of([choice(2, 2)])), 1);
    assert_eq!(model_count(&Program::of([choice(1, 2)])), 0);
}

#[test]
fn equal_rule_merges_do_not_multiply_counted_entries() {
    assert_eq!(model_count(&Program::of([choice(2, 2), choice(2, 2)])), 1);
    assert_eq!(model_count(&Program::of([choice(1, 1), choice(2, 1)])), 0);
}
