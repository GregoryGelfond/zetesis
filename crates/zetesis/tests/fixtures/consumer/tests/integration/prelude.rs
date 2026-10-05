//! The facade prelude keeps each shared spelling's canonical meaning.

use z::prelude::*;

#[test]
fn the_prelude_parses_and_raises_a_program() {
    let source = Source::new(SourceId::new(3), "p(1).".to_owned()).expect("short source");
    let parsed: Parse<ast::Program> = parse(&source, Dialect::Clingo);
    let raised = z::program::raise::raise(&parsed);
    let expected: Program = Program::of([Rule::fact(Atom::new(
        Name::new("p").expect("a name"),
        [Term::from(1i32)],
    ))]);
    assert_eq!(raised.program(), &expected);
}

#[test]
fn symbols_keep_the_canonical_exchange_type() {
    let symbol: z::Symbol = Symbol::from(1i32);
    let mut answer: z::program::AnswerSet = AnswerSet::new();
    answer.insert(symbol.clone());
    assert!(answer.contains(&symbol));
}

#[test]
fn the_prelude_preserves_tier_type_identity() {
    // These assignments deliberately test type identity and ambiguous-name
    // resolution at a use site; importing globs alone would not detect a clash.
    let _: Option<z::program::program::Query> = None::<Query>;
    let _: Option<z::query::Atom> = None::<Atom>;
    let _: Option<z::query::Consequences> = None::<Consequences>;
    let _: Option<z::query::WorldView<'static>> = None::<WorldView<'static>>;
    let _: Option<z::solve::outcome::Conclusion> = None::<Conclusion>;
    let _: Option<z::analysis::analysis::Analysis> = None::<Analysis>;
}
