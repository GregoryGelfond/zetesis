//! Source comparisons agree with the pinned estate term-order implementation.

use std::cmp::Ordering;

use themelios_program::symbol::Symbol;
use zetesis_core::Value;

#[test]
fn scalar_language_order_agrees_with_pinned_symbol_order_for_every_pair() {
    let texts = [
        "(-2147483647-1)",
        "-1",
        "0",
        "1",
        "2147483647",
        "a",
        "aa",
        "z",
        "\"\"",
        "\"a\"",
        "\"aa\"",
        "\"z\"",
        "\"é\"",
        "\"😀\"",
    ];
    let values: Vec<(Value, Symbol)> = texts
        .iter()
        .map(|text| {
            let admitted = zetesis_themelios::admit_extended(
                format!("p({text})."),
                zetesis_themelios::AdmissionOptions::default(),
                zetesis_themelios::ExpansionLimits::default(),
            )
            .unwrap();
            let value = match &admitted.program().templates()[0].head().unwrap().terms()[0] {
                zetesis_core::Term::Constant(value) => value.clone(),
                zetesis_core::Term::Variable(_) => panic!("scalar fact must remain ground"),
            };
            let parsed = themelios_syntax::parse::parse(
                &themelios_base::source::Source::new(
                    themelios_base::source::SourceId::new(0),
                    format!("p({text})."),
                )
                .unwrap(),
                themelios_syntax::dialect::Dialect::Clingo,
            );
            let raised = themelios_program::raise::raise(&parsed);
            let statement = raised.program().statements().next().unwrap();
            let themelios_program::program::Statement::Rule(rule) = statement.get() else {
                panic!("fact");
            };
            let themelios_program::program::Head::Literal(literal) = rule.head().get() else {
                panic!("head");
            };
            let themelios_program::program::LiteralInner::Atom(atom) = &literal.inner else {
                panic!("atom");
            };
            let themelios_program::program::Arguments::Single(terms) = &atom.get().arguments else {
                panic!("single scalar");
            };
            let symbol = terms[0].evaluate().unwrap();
            (value, symbol)
        })
        .collect();
    for (left, left_symbol) in &values {
        for (right, right_symbol) in &values {
            assert_eq!(
                left.compare_terms(right),
                left_symbol.cmp(right_symbol),
                "{left:?} versus {right:?}"
            );
            assert_eq!(left.compare_terms(right) == Ordering::Equal, left == right);
        }
    }
    // Storage canonicalization deliberately retains its existing order.
    assert!(Value::String(String::new()) < Value::Symbol("z".into()));
    assert_eq!(
        Value::String(String::new()).compare_terms(&Value::Symbol("z".into())),
        Ordering::Greater
    );
}
