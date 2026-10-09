use super::*;
use crate::{AdmissionOptions, ExpansionLimits, ParsedSource, StatementId};
use themelios_program::raise::raise;

const SOURCE: &str = include_str!("../../tests/fixtures/fixed-constraints/projection.lp");

fn program(source: &str) -> Program {
    let parsed = ParsedSource::new(source.into(), AdmissionOptions::default()).unwrap();
    let raised = raise(parsed.parsed());
    assert!(raised.diagnostics().is_empty());
    raised.into_program()
}

fn owned(program: &Program, budget: &mut Budget) -> (Program, Owners) {
    let statements: Vec<_> = program.statements().cloned().collect();
    let owners = (0..statements.len())
        .map(|index| Some(StatementId::new(index)))
        .collect();
    Owners::collect(statements, owners, budget, ProgramSite::program()).unwrap()
}

fn analyze(source: &str, policy: Policy) -> Replacements {
    analyze_program(&program(source), policy)
}

fn analyze_program(program: &Program, policy: Policy) -> Replacements {
    let mut budget = Budget::new(ExpansionLimits::default(), 100_000);
    let (program, owners) = owned(program, &mut budget);
    let mut facts = FixedFacts::new(&program);
    with_policy(
        &program,
        &owners,
        &mut facts,
        &mut budget,
        ProgramSite::program(),
        policy,
    )
    .unwrap()
}

fn constraints(source: &str) -> Program {
    Program::of_nodes(program(source).statements().filter(|carrier| {
        matches!(carrier.get(), Statement::Rule(rule) if matches!(rule.head().get(), Head::Falsum))
    }).cloned())
}

fn rewritten(source: &str) -> Program {
    Program::of(
        analyze(source, POLICY)
            .into_values()
            .flat_map(|replacement| replacement.rules),
    )
}

#[test]
fn distinct_interfaces_produce_one_constraint_each() {
    let expected = include_str!("../../tests/fixtures/fixed-constraints/expected.lp");
    assert_eq!(rewritten(SOURCE), constraints(expected));
}

#[test]
fn fixed_guard_membership_preserves_tuple_correlation() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/correlation.lp");
    assert_eq!(rewritten(source), constraints(":- p(a)."));
}

#[test]
fn repeated_anchor_variables_keep_equality() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/repeated.lp");
    assert_eq!(rewritten(source), constraints(":- p(a)."));
}

#[test]
fn fixed_signatures_keep_strong_sign() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/signed.lp");
    assert_eq!(rewritten(source), constraints(":- p(a)."));
}

#[test]
fn positive_constructor_patterns_specialize_typed_values() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/nested.lp");
    let expected = include_str!("../../tests/fixtures/fixed-constraints/expected-nested.lp");
    assert_eq!(rewritten(source), constraints(expected));
}

#[test]
fn a_nonfact_anchor_producer_prevents_specialization() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/derived.lp");
    assert!(analyze(source, POLICY).is_empty());
}

#[test]
fn a_nonfact_guard_keeps_its_variable_live() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/guard-produced.lp");
    assert!(analyze(source, POLICY).is_empty());
}

#[test]
fn scalar_instructions_keep_the_original_constraint() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/scalar.lp");
    assert!(analyze(source, POLICY).is_empty());
}

#[test]
fn output_refusal_never_publishes_a_partial_family() {
    assert!(analyze(SOURCE, Policy { rules: 1, ..POLICY }).is_empty());
}

#[test]
fn work_refusal_never_publishes_a_partial_family() {
    let expected = rewritten(SOURCE);
    assert!(
        !analyze(
            SOURCE,
            Policy {
                work: 400,
                ..POLICY
            }
        )
        .is_empty()
    );
    for work in 0..400 {
        let rules = Program::of(
            analyze(SOURCE, Policy { work, ..POLICY })
                .into_values()
                .flat_map(|replacement| replacement.rules),
        );
        assert!(
            rules.statements().next().is_none() || rules == expected,
            "work {work}"
        );
    }
}

#[test]
fn optional_refusal_keeps_accepted_work_charged() {
    let mut budget = Budget::new(ExpansionLimits::default(), 100_000);
    let (program, owners) = owned(&program(SOURCE), &mut budget);
    let before = budget.usage().term_work;
    let mut facts = FixedFacts::new(&program);
    with_policy(
        &program,
        &owners,
        &mut facts,
        &mut budget,
        ProgramSite::program(),
        Policy { work: 7, ..POLICY },
    )
    .unwrap();
    assert_eq!(budget.usage().term_work - before, 7);
}

#[test]
fn cancellation_is_a_typed_refusal() {
    let cancellation = zetesis_cpu::Cancellation::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 100_000)
        .with_cancellation(Some(cancellation.clone()));
    let (program, owners) = owned(&program(SOURCE), &mut budget);
    cancellation.cancel();
    let mut facts = FixedFacts::new(&program);
    assert!(matches!(
        specialize(
            &program,
            &owners,
            &mut facts,
            &mut budget,
            ProgramSite::program()
        ),
        Err(FormulaFailure::Expansion(
            ExpansionFailure::Interrupted { .. }
        ))
    ));
}

#[test]
fn family_byte_refusal_retains_the_original() {
    let mut budget = Budget::new(
        ExpansionLimits {
            max_family_bytes: 0,
            ..ExpansionLimits::default()
        },
        100_000,
    );
    let (program, owners) = owned(&program(SOURCE), &mut budget);
    let mut facts = FixedFacts::new(&program);
    assert!(
        specialize(
            &program,
            &owners,
            &mut facts,
            &mut budget,
            ProgramSite::program()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn replacements_keep_original_statement_ownership() {
    let original = program(SOURCE);
    let owner = original.statements().position(|carrier| matches!(carrier.get(), Statement::Rule(rule) if matches!(rule.head().get(), Head::Falsum))).unwrap();
    let replacements = analyze(SOURCE, POLICY);
    assert_eq!(
        replacements.keys().copied().collect::<Vec<_>>(),
        [StatementId::new(owner)]
    );
}

fn constructor_constraint(nesting: usize) -> Program {
    use themelios_program::symbol::Name;

    let mut term = Term::variable(VarName::new("X").unwrap());
    for _ in 0..nesting {
        term = Term::function(Name::new("f").unwrap(), [term]);
    }
    assert!(!term.is_ground());
    // Parse only a shallow fixture. The typed constructor builds the boundary
    // term without exercising the independent source parser nesting ceiling.
    let source =
        program("edge(a,x). edge(a,y). allowed(x). allowed(y). :- edge(X,Y), allowed(Y), p(X).");
    Program::of_nodes(source.statements().cloned().map(|carrier| {
        carrier.map(|statement| {
            let Statement::Rule(rule) = statement else {
                return statement;
            };
            if !matches!(rule.head().get(), Head::Falsum) {
                return Statement::Rule(rule);
            }
            let body = rule.body().get().elements().map(|element| {
                let mut element = element.get().clone();
                if let BodyElement::Literal(literal) = &mut element
                    && let LiteralInner::Atom(atom) = &literal.inner
                    && atom.get().name.as_str() == "p"
                {
                    literal.inner = LiteralInner::Atom(atom.clone().map(|mut atom| {
                        atom.arguments = Arguments::Single(vec![term.clone()]);
                        atom
                    }));
                }
                element
            });
            Statement::Rule(Rule::new(Head::Falsum, Body::new(body)))
        })
    }))
}

#[test]
fn newly_ground_constructors_keep_the_whole_depth_limit() {
    // The written positive pattern is non-ground; substitution introduces the
    // ground value, so checking only its atomic children would miss the limit.
    let depth = zetesis_core::ValueLimits::default().max_depth;
    let replacements = analyze_program(&constructor_constraint(depth - 1), POLICY);
    assert_eq!(replacements.len(), 1);
    let replacement = replacements.into_values().next().unwrap();
    assert_eq!(replacement.rules.len(), 1);
    let element = replacement.rules[0].body().get().elements().next().unwrap();
    let BodyElement::Literal(literal) = element.get() else {
        panic!("positive residual");
    };
    let [Term::Symbolic(value)] = terms(atom(literal)) else {
        panic!("substitution grounds the whole constructor");
    };
    assert_eq!(value.subsymbols().count(), depth);
    crate::compile::validate_scalar(value, ProgramSite::program()).unwrap();
    assert!(analyze_program(&constructor_constraint(depth), POLICY).is_empty());
}

#[test]
fn merged_original_owners_prevent_replacement() {
    let mut budget = Budget::new(ExpansionLimits::default(), 100_000);
    let original = program(SOURCE);
    let mut statements: Vec<_> = original.statements().cloned().collect();
    let constraint = statements.iter().find(|carrier| matches!(carrier.get(), Statement::Rule(rule) if matches!(rule.head().get(), Head::Falsum))).unwrap().clone();
    statements.push(constraint);
    let identities = (0..statements.len())
        .map(|index| Some(StatementId::new(index)))
        .collect();
    let (program, owners) =
        Owners::collect(statements, identities, &mut budget, ProgramSite::program()).unwrap();
    let mut facts = FixedFacts::new(&program);
    assert!(
        specialize(
            &program,
            &owners,
            &mut facts,
            &mut budget,
            ProgramSite::program()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn ineligible_anchor_shapes_leave_facts_unread() {
    let source = include_str!("../../tests/fixtures/fixed-constraints/ineligible-anchors.lp");
    let mut budget = Budget::new(ExpansionLimits::default(), 100_000);
    let (program, owners) = owned(&program(source), &mut budget);
    let mut facts = FixedFacts::new(&program);
    specialize(
        &program,
        &owners,
        &mut facts,
        &mut budget,
        ProgramSite::program(),
    )
    .unwrap();
    // Reading after the pass still spends the complete program reading. This
    // would spend zero if any impossible anchor had prepared the shared index.
    let mut reading = KeyWork::new(POLICY.work);
    facts.read(&mut reading).unwrap();
    assert_eq!(
        u128::from(reading.steps()),
        program.statements().count() as u128
    );
}
