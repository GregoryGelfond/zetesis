//! Each facade macro builds the value of its canonical typed constructor.

use z::program::construct;
use z::program::program::{
    Aggregate, AggregateFunction, BodyAggregateElement, Choice, ChoiceElement, Condition,
    DefaultNegation, External, FunctionAggregate, Guard, HeadAggregate, HeadAggregateElement,
    Identity, LiteralInner, OptimizeElement, Show, weight,
};
use z::program::symbol::VarName;
use z::program::{
    Atom, Body, BodyElement, Head, IntoHead, Literal, Name, Origin, Relation, Rule, Sign,
    Signature, Statement, Term,
};

fn name(text: &str) -> Name {
    Name::new(text).expect("a valid name")
}

fn variable() -> Term {
    Term::variable(VarName::new("X").expect("a valid variable"))
}

fn p_one() -> Atom {
    Atom::new(name("p"), [Term::from(1i32)])
}

fn p_variable() -> Atom {
    Atom::new(name("p"), [variable()])
}

fn q_if_p() -> Rule {
    Atom::new(name("q"), [variable()])
        .into_head()
        .when(p_variable())
}

fn verum() -> Literal {
    Literal {
        negation: DefaultNegation::None,
        inner: LiteralInner::True,
    }
}

// A program block is ASP, so rustfmt must preserve its statement terminators.
#[rustfmt::skip]
#[test]
fn the_program_macro_builds_canonical_statements() {
    let actual: z::Program = z::program! { p(1). q(X) :- p(X). };
    let expected = z::program::Program::of([
        Statement::from(Rule::fact(p_one())),
        Statement::from(q_if_p()),
    ]);
    assert_eq!(actual, expected);
}

#[rustfmt::skip]
#[test]
fn the_program_macro_can_be_imported() {
    use z::program;
    let actual: z::Program = program! { p(1). };
    assert_eq!(actual, z::Program::of([Rule::fact(p_one())]));
}

#[test]
fn an_empty_macro_builds_the_empty_program() {
    assert_eq!(z::program! {}, z::Program::empty());
}

#[rustfmt::skip]
#[test]
fn macro_statements_have_constructed_provenance() {
    let program = z::program! { p(1). };
    let statement = program.statements().next().expect("one statement");
    let origins: Vec<_> = statement.provenance().origins().collect();
    assert_eq!(origins, [&Origin::Constructed]);
}

#[test]
fn the_atom_macro_builds_strong_negation() {
    assert_eq!(z::atom!(-p(1)), -p_one());
}

#[test]
fn the_fact_macro_splices_a_value() {
    let value = 1i32;
    assert_eq!(z::fact!(p($value)), Rule::fact(p_one()));
}

#[test]
fn the_rule_macro_builds_a_rule() {
    assert_eq!(z::rule!(q(X) :- p(X)), q_if_p());
}

#[test]
fn the_constraint_macro_builds_falsum() {
    assert_eq!(z::constraint!(:- p(X)), Rule::constraint(p_variable()));
}

#[test]
fn the_minimize_macro_builds_an_objective() {
    let expected = construct::minimize([OptimizeElement::new(
        weight(Term::from(3i32)).at_priority(Term::from(1i32)),
        [],
        Condition::empty(),
    )]);
    assert_eq!(z::minimize!({ 3@1 }), expected);
}

#[test]
fn the_maximize_macro_builds_an_objective() {
    let expected = construct::maximize([OptimizeElement::new(
        weight(Term::from(5i32)),
        [],
        Condition::empty(),
    )]);
    assert_eq!(z::maximize!({ 5 }), expected);
}

#[test]
fn the_show_macro_builds_a_signature() {
    let expected = Show::Signature(Signature::new(Sign::Positive, name("p"), 1));
    assert_eq!(z::show!(p / 1), expected);
}

#[test]
fn the_external_macro_builds_a_declaration() {
    let expected = External::new(p_variable(), Body::empty(), None);
    assert_eq!(z::external!(p(X)), expected);
}

#[test]
fn the_fact_macro_builds_a_choice() {
    let expected = Rule::fact(Choice::new(
        Some(Guard {
            relation: None,
            term: Term::from(1i32),
        }),
        [ChoiceElement::new(
            Atom::new(name("a"), []).into(),
            Condition::new([Literal::from(p_variable())]),
        )],
        None,
    ));
    assert_eq!(z::fact!(1 { a : p(X) }), expected);
}

#[test]
fn the_fact_macro_builds_a_head_aggregate() {
    let expected = Rule::fact(HeadAggregate::new(
        None,
        AggregateFunction::Count,
        [HeadAggregateElement::new(
            [variable()],
            p_variable().into(),
            Condition::empty(),
        )],
        None,
    ));
    assert_eq!(z::fact!(#count { X : p(X) }), expected);
}

#[test]
fn the_rule_macro_builds_a_body_aggregate() {
    let expected = Atom::new(name("a"), [])
        .into_head()
        .when(Body::new([BodyElement::from(Aggregate::Function(
            FunctionAggregate::new(
                Some(Guard {
                    relation: Some(Relation::Le),
                    term: Term::from(1i32),
                }),
                AggregateFunction::Sum,
                [BodyAggregateElement::new(
                    [variable()],
                    Condition::new([Literal::from(p_variable())]),
                )],
                None,
            ),
        ))]));
    assert_eq!(z::rule!(a :- 1 <= #sum { X : p(X) }), expected);
}

#[test]
fn repeated_booleans_match_typed_construction() {
    let element = || ChoiceElement::new(verum(), Condition::empty());
    let expected = Rule::fact(Choice::new(None, [element(), element()], None));
    assert_eq!(z::fact!({ #true; #true }), expected);
}

#[test]
fn repeated_booleans_remain_two_occurrences() {
    let rule = z::fact!({ #true; #true });
    let Head::Choice(choice) = rule.head().get() else {
        panic!("a choice head");
    };
    let identities: Vec<_> = choice
        .elements()
        .map(|element| element.get().identity())
        .collect();
    assert_eq!(identities, [Identity::ByOccurrence; 2]);
}

#[test]
fn repeated_atoms_merge_by_content() {
    let rule = z::fact!({
        p(1);
        p(1)
    });
    let Head::Choice(choice) = rule.head().get() else {
        panic!("a choice head");
    };
    let identities: Vec<_> = choice
        .elements()
        .map(|element| element.get().identity())
        .collect();
    assert_eq!(identities, [Identity::ByContent]);
}
