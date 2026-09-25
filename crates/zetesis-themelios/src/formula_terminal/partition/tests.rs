//! Same-owner source/IR correspondence controls over actual preparation.

use themelios_base::{
    source::{Source, SourceId},
    span::Location,
};
use themelios_program::program::DefaultNegation;

use super::{Definitions, Partition, partition};
use crate::expansion::Budget;
use crate::formula_ir::{HeadIr, HeadLiteral, HeadOperand, LiteralIr, PreparationContext, RuleIr};
use crate::formula_support::{Counters, GroundingWork, SupportCatalog, components};
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits, FormulaResource};

fn prepare(text: &str) -> crate::formula::Preparation {
    let source = Source::new(SourceId::new(241), text.into()).unwrap();
    let parsed =
        themelios_syntax::parse::parse(&source, themelios_syntax::dialect::Dialect::Clingo);
    assert!(parsed.diagnostics().is_empty());
    let raised = themelios_program::raise::raise(&parsed);
    assert!(raised.diagnostics().is_empty());
    let location = Location {
        source: source.id(),
        span: source.span(),
    };
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 10_000);
    let mut catalog = SupportCatalog::default();
    let mut counters = Counters::default();
    let mut metadata = crate::metadata::Builder::default();
    crate::metadata::collect_profile(raised.program(), &mut metadata, true).unwrap();
    let metadata = metadata.finish(location).unwrap();
    let program = PreparationContext {
        options: AdmissionOptions::default(),
        budget: &mut budget,
        catalog: &mut catalog,
        work: GroundingWork::new(&limits, &mut counters, location),
    }
    .prepare(
        raised.program(),
        metadata.project_selection().clone(),
        &crate::formula_choice_source::Catalog::default(),
    )
    .unwrap();
    crate::formula::Preparation {
        catalog,
        accounting: counters.into_accounting(),
        program,
        budget,
        limits,
        options: crate::grounding_options::Execution::default(),
        location,
    }
}

fn selected_rule(prepared: &mut crate::formula::Preparation) -> &mut RuleIr {
    prepared
        .program
        .rules
        .iter_mut()
        .find(|rule| !rule.body.is_empty())
        .unwrap()
}

fn pattern(
    prepared: &mut crate::formula::Preparation,
    name: &str,
    slots: &[usize],
) -> components::Pattern {
    let predicate = zetesis_core::Predicate::new(name, slots.len()).unwrap();
    let mut counters = Counters::resume(
        std::mem::take(&mut prepared.accounting),
        crate::grounding_observer::Work::default(),
    );
    let mut admission = prepared
        .catalog
        .component_admission(&prepared.limits, &mut counters, prepared.location)
        .unwrap();
    let predicate = admission
        .predicate(
            (&predicate).into(),
            &prepared.limits,
            &mut counters,
            prepared.location,
        )
        .unwrap();
    let terms: Vec<_> = slots
        .iter()
        .map(|slot| components::Term::Variable(*slot))
        .collect();
    let changed = admission
        .pattern(
            predicate,
            &terms,
            &prepared.limits,
            &mut counters,
            prepared.location,
        )
        .unwrap();
    admission
        .finish(&prepared.limits, &mut counters, prepared.location)
        .unwrap();
    prepared.accounting = counters.into_accounting();
    changed
}

#[test]
fn all_terminal_producers_move_into_the_deferred_program() {
    let prepared = prepare("p(1). q(2). d(X):-p(X). d(X):-q(X).");
    let original_count = prepared.program.analyzed.statements().count();
    let Partition {
        base,
        terminal:
            Some(Definitions {
                original,
                deferred,
                deferred_storage,
            }),
    } = partition(prepared).unwrap()
    else {
        panic!("eligible terminal definitions")
    };
    assert_eq!(deferred.len(), 2);
    assert_eq!(base.program.rules.len(), 2);
    assert_eq!(original.program.statements().count(), original_count);
    assert_eq!(base.program.analyzed.statements().count(), 2);
    assert_eq!(
        deferred_storage.bytes() as u128,
        super::deferred_bytes(&deferred)
    );
}

#[test]
fn coalesced_source_carriers_cover_every_ir_occurrence() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    let source_count = prepared.program.analyzed.statements().count();
    let rule = selected_rule(&mut prepared);
    let HeadIr::Normal(head) = rule.head else {
        panic!("normal fixture")
    };
    let duplicate = RuleIr {
        head: HeadIr::Normal(head),
        body: rule
            .body
            .iter()
            .map(|literal| match literal {
                LiteralIr::Atom(negation, pattern) => LiteralIr::Atom(*negation, *pattern),
                _ => panic!("flat fixture"),
            })
            .collect(),
        variables: rule.variables,
        body_variables: rule.body_variables,
        bindings: None,
        origins: rule.origins.clone(),
        location: rule.location,
    };
    prepared.program.rules.push(duplicate);
    let Partition {
        terminal: Some(Definitions {
            original, deferred, ..
        }),
        ..
    } = partition(prepared).unwrap()
    else {
        panic!("all equivalent occurrences are retained")
    };
    assert_eq!(original.program.statements().count(), source_count);
    assert_eq!(deferred.len(), 2);
}

#[test]
fn missing_source_producers_prevent_partition() {
    let mut prepared = prepare("p(1). q(2). d(X):-p(X). d(X):-q(X).");
    let position = prepared
        .program
        .rules
        .iter()
        .position(|rule| !rule.body.is_empty())
        .unwrap();
    prepared.program.rules.remove(position);
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn selected_signatures_in_disjunctions_prevent_partition() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    let rule = selected_rule(&mut prepared);
    let HeadIr::Normal(Some(head)) = rule.head else {
        panic!("normal fixture")
    };
    rule.head = HeadIr::Disjunction(vec![HeadLiteral {
        negation: DefaultNegation::Not,
        operand: HeadOperand::Atom(head),
    }]);
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn omitted_body_occurrences_prevent_partition() {
    let mut prepared = prepare("p(1). q(1). d(X):-p(X),q(X).");
    selected_rule(&mut prepared).body.pop();
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn same_locations_do_not_establish_correspondence() {
    let mut prepared = prepare("p(1). q(2). d(X):-p(X).");
    let q = prepared
        .program
        .rules
        .iter()
        .filter_map(|rule| match &rule.head {
            HeadIr::Normal(Some(atom)) if rule.body.is_empty() => Some(*atom),
            _ => None,
        })
        .next_back()
        .unwrap();
    selected_rule(&mut prepared).body = vec![LiteralIr::Atom(DefaultNegation::None, q)];
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn anonymous_body_variables_remain_independent() {
    let prepared = prepare("p(1,2). q(3). d(X):-p(_,_),q(X).");
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition {
            terminal: Some(_),
            ..
        }
    ));
}

#[test]
fn dependency_projection_is_inapplicable() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    prepared.program.analysis_basis = crate::AnalysisBasis::DependencyProjection;
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn objective_declarations_exclude_partition() {
    let prepared = prepare("p(1). d(X):-p(X). #minimize{}.");
    assert!(!prepared.program.objective_declarations.is_empty());
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn signature_projection_excludes_partition() {
    let prepared = prepare("p(1). d(X):-p(X). #project d/1.");
    assert!(prepared.program.project_selection.is_explicit());
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn fallback_retains_accepted_work() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    let before = prepared.accounting.work;
    selected_rule(&mut prepared).body.clear();
    let Partition {
        base: prepared,
        terminal: None,
    } = partition(prepared).unwrap()
    else {
        panic!("mismatched lowered body")
    };
    assert!(prepared.accounting.work > before);
}

#[test]
fn exhausted_work_is_not_optional_fallback() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    prepared.limits.max_work = prepared.accounting.work;
    assert!(matches!(
        partition(prepared),
        Err(crate::FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
}

#[test]
fn distinct_source_variables_cannot_collapse() {
    let mut prepared = prepare("p(1,2). d(X,Y):-p(X,Y).");
    let changed = pattern(&mut prepared, "d", &[0, 0]);
    selected_rule(&mut prepared).head = HeadIr::Normal(Some(changed));
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn repeated_source_variables_cannot_split() {
    let mut prepared = prepare("p(1,1). d(X):-p(X,X).");
    let changed = pattern(&mut prepared, "p", &[0, 1]);
    let rule = selected_rule(&mut prepared);
    rule.body = vec![LiteralIr::Atom(DefaultNegation::None, changed)];
    rule.variables = 2;
    rule.body_variables = 2;
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn compiled_reads_of_a_selected_head_prevent_partition() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    let changed = pattern(&mut prepared, "d", &[0]);
    selected_rule(&mut prepared)
        .body
        .push(LiteralIr::Atom(DefaultNegation::Not, changed));
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}

#[test]
fn support_storage_refusal_is_not_optional_fallback() {
    let mut prepared = prepare("p(1). d(X):-p(X).");
    prepared.limits.max_support_bytes = 0;
    assert!(matches!(
        partition(prepared),
        Err(crate::FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
}

#[test]
fn typed_closed_compounds_are_terminal_definitions() {
    let prepared = prepare("d(f(7,(a,\"a\",-g))). d(f(#inf,(#sup,g(1,2)))).");
    let Partition {
        terminal: Some(Definitions { deferred, .. }),
        ..
    } = partition(prepared).unwrap()
    else {
        panic!("already closed typed constants are eligible")
    };
    assert_eq!(deferred.len(), 2);
}

#[test]
fn nested_constant_type_mismatches_prevent_partition() {
    // Each pair agrees through the outer constructor and its first child. A
    // root-only comparison would incorrectly cover both
    // source carriers after replacing one lowered head by the other.
    for text in [
        "d(f(7,(a,9))). d(f(7,(\"a\",9))).",
        "d(f(7,(g,9))). d(f(7,(-g,9))).",
        "d(f(7,(g(1,2),9))). d(f(7,((1,2),9))).",
    ] {
        let mut prepared = prepare(text);
        assert_eq!(prepared.program.rules.len(), 2);
        assert_eq!(prepared.program.analyzed.statements().count(), 2);
        let HeadIr::Normal(Some(replacement)) = prepared.program.rules[1].head else {
            panic!("closed fact fixture")
        };
        prepared.program.rules[0].head = HeadIr::Normal(Some(replacement));
        assert!(
            matches!(
                partition(prepared).unwrap(),
                Partition { terminal: None, .. }
            ),
            "{text}"
        );
    }
}

#[test]
fn reads_in_nested_conditions_prevent_partition() {
    const SOURCE: &str = "p(1). d(X):-p(X). :-q(X):p(X).";
    // Establish that the real source and its unchanged nested lowering qualify;
    // merely having a conditional must not make the negative control vacuous.
    let Partition {
        terminal: Some(Definitions { deferred, .. }),
        ..
    } = partition(prepare(SOURCE)).unwrap()
    else {
        panic!("unrelated conditional is a permitted base consumer")
    };
    assert_eq!(deferred.len(), 1);

    let mut prepared = prepare(SOURCE);
    let head = prepared
        .program
        .rules
        .iter()
        .find_map(|rule| match rule.head {
            HeadIr::Normal(Some(head)) if !rule.body.is_empty() => Some(head),
            _ => None,
        })
        .unwrap();
    let condition = prepared
        .program
        .rules
        .iter_mut()
        .flat_map(|rule| rule.body.iter_mut())
        .find_map(|literal| match literal {
            LiteralIr::Conditional(conditional) => Some(&mut conditional.condition),
            _ => None,
        })
        .expect("actual lowered conditional scope");
    let atom = condition
        .iter_mut()
        .find_map(|literal| match literal {
            LiteralIr::Atom(_, atom) => Some(atom),
            _ => None,
        })
        .expect("actual positive binder inside the conditional");
    // Only the unselected constraint's inner read changes. Every selected head
    // and positive producer still matches its source carrier exactly, so the
    // defensive all-scope read check is the reason to reject this partition.
    *atom = head;
    assert!(matches!(
        partition(prepared).unwrap(),
        Partition { terminal: None, .. }
    ));
}
