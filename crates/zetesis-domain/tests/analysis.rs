//! Direct Program analysis, upper-cover properties, and conservative boundaries.

use std::collections::BTreeSet;

use proptest::prelude::*;
use themelios_base::source::{Source, SourceId};
use themelios_program::program::{
    Arguments, Atom, Body, BodyElement, DefaultNegation, Literal, LiteralInner, Program, Rule,
    Statement,
};
use themelios_program::provenance::WithProvenance;
use themelios_program::raise::raise;
use themelios_program::symbol::{Name, Sign, Signature, Symbol, VarName};
use themelios_program::term::{Term, Variable};
use themelios_syntax::{dialect::Dialect, parse::parse};
use zetesis_domain::{
    Analysis, Context, Domain, Limits, Resource, Status, UnknownReason, Widening, analyze,
};

fn source(text: &str) -> Program {
    let source = Source::new(SourceId::new(17), text.to_owned()).unwrap();
    let parsed = parse(&source, Dialect::Clingo);
    assert!(
        parsed.diagnostics().is_empty(),
        "{text}: {:?}",
        parsed.diagnostics()
    );
    let raised = raise(&parsed);
    assert!(
        raised.diagnostics().is_empty(),
        "{text}: {:?}",
        raised.diagnostics()
    );
    raised.program().clone()
}
fn signature(name: &str, arity: u32) -> Signature {
    Signature {
        sign: Sign::Positive,
        name: Name::new(name).unwrap(),
        arity,
    }
}
fn numbers(analysis: &Analysis<'_>, name: &str, index: usize, arity: u32) -> BTreeSet<i32> {
    let Domain::Finite(values) = analysis.domain(&signature(name, arity), index) else {
        panic!("{name}/{arity} argument {index} unexpectedly Unknown")
    };
    values
        .iter()
        .map(|value| {
            let Symbol::Number(value) = value else {
                panic!("expected numeric fixture");
            };
            *value
        })
        .collect()
}

#[test]
fn finite_flow_joins_union_producers_and_share_exact_source_symbols() {
    let program = source("p(1).p(2).r(3).q(X):-p(X),r(X).out(X):-q(X).out(4).cycle(X):-cycle(X).");
    let result = analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(
        numbers(&result, "q", 0, 1),
        BTreeSet::from([1, 2, 3]),
        "union intentionally forgets join correlations"
    );
    assert_eq!(numbers(&result, "out", 0, 1), BTreeSet::from([1, 2, 3, 4]));
    assert!(numbers(&result, "cycle", 0, 1).is_empty());
    assert!(result.belongs_to(&program));
    assert!(!result.belongs_to(&program.clone()));
    for (_, _, argument) in result.arguments() {
        for producer in argument.producers() {
            assert!(
                program
                    .statements()
                    .any(|statement| std::ptr::eq(statement, *producer))
            );
            assert!(producer.provenance().origins().next().is_some());
        }
    }
    let Domain::Finite(values) = result.domain(&signature("p", 1), 0) else {
        unreachable!()
    };
    let symbol = values.first().unwrap();
    assert!(program.statements().any(|statement| {
        let Statement::Rule(rule) = statement.get() else {
            return false;
        };
        let themelios_program::program::Head::Literal(literal) = rule.head().get() else {
            return false;
        };
        let themelios_program::program::LiteralInner::Atom(atom) = &literal.inner else {
            return false;
        };
        atom.get()
            .argument_terms()
            .any(|term| matches!(term, Term::Symbolic(value) if std::ptr::eq(value, *symbol)))
    }));
}

#[test]
fn unsupported_producers_and_local_scopes_cannot_narrow_global_bounds() {
    for text in [
        "p(1).p(X):-not q(X).out(X):-p(X).",
        "p(1).p(X+1):-p(X).out(X):-p(X).",
        "p(1).{p(X):q(X)}.out(X):-p(X).",
        "p(1).p(X):q(X)|other. out(X):-p(X).",
        "p(1).p(_):-q(X).out(X):-p(X).",
        "p(1).p(X):-q(_).out(X):-p(X).",
        "p(1).p(X):-X=1..2.out(X):-p(X).",
        "p(1).p(@value()).out(X):-p(X).",
    ] {
        let program = source(text);
        let result = analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::FixedPoint, "{text}");
        assert_eq!(
            result.domain(&signature("p", 1), 0),
            &Domain::Unknown,
            "{text}"
        );
        assert_eq!(
            result.domain(&signature("out", 1), 0),
            &Domain::Unknown,
            "{text}"
        );
    }
    let program = source("p(1).q(2).a(X):-p(X).b(X):-q(X).{c(X):q(X)}.d(X):-p(X),not c(X),X>99.");
    let result = analyze(&program, Limits::default());
    assert_eq!(numbers(&result, "a", 0, 1), BTreeSet::from([1]));
    assert_eq!(numbers(&result, "b", 0, 1), BTreeSet::from([2]));
    assert_eq!(
        numbers(&result, "d", 0, 1),
        BTreeSet::from([1]),
        "filters do not narrow the upper cover"
    );
}

#[test]
fn unresolved_context_never_publishes_resolved_constant_or_partial_claims() {
    for (text, reason) in [
        ("#const n=2.p(n).", UnknownReason::Constants),
        ("p(1).#include \"missing.lp\".", UnknownReason::Include),
        ("p(1).#external q(2).", UnknownReason::External),
        ("p(1).#program step(t).q(t).", UnknownReason::ProgramPart),
        ("p(1).&a{}.", UnknownReason::Theory),
    ] {
        let program = source(text);
        let result = analyze(&program, Limits::default());
        assert_eq!(result.status(), Status::Unknown(reason), "{text}");
        assert_eq!(result.arguments().count(), 0);
        assert_eq!(result.domain(&signature("p", 1), 0), &Domain::Unknown);
        assert_eq!(result.domain(&signature("unseen", 1), 0), &Domain::Unknown);
        assert!(result.context().is_some());
        match result.context().unwrap() {
            Context::Part(part) => assert!(program.parts().any(|p| std::ptr::eq(p, part))),
            Context::Statement(statement) => {
                assert!(program.statements().any(|s| std::ptr::eq(s, statement)));
            }
        }
    }
    let program = source("p(n).#defined input/1.#show output/1.");
    let result = analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::FixedPoint);
    let Domain::Finite(values) = result.domain(&signature("p", 1), 0) else {
        unreachable!()
    };
    assert!(
        values
            .iter()
            .any(|symbol| symbol.name().is_some_and(|name| name.as_str() == "n"))
    );
    assert_eq!(
        result.domain(&signature("input", 1), 0),
        &Domain::Unknown,
        "#defined supplies no open-input domain"
    );
}

#[test]
fn signed_signatures_and_argument_positions_remain_distinct() {
    let program = source("p(1,2).-p(3,4).q(X,Y):--p(Y,X).");
    let result = analyze(&program, Limits::default());
    assert_eq!(numbers(&result, "p", 0, 2), BTreeSet::from([1]));
    assert_eq!(numbers(&result, "q", 0, 2), BTreeSet::from([4]));
    assert_eq!(numbers(&result, "q", 1, 2), BTreeSet::from([3]));
    assert_eq!(result.domain(&signature("q", 2), 2), &Domain::Unknown);
}

#[test]
fn every_global_ceiling_returns_unknown_instead_of_an_unfinished_finite_set() {
    let program = source("p(1).q(X):-p(X).");
    let default = Limits::default();
    for (limits, resource) in [
        (
            Limits {
                max_work: 0,
                ..default
            },
            Resource::Work,
        ),
        (
            Limits {
                max_predicates: 0,
                ..default
            },
            Resource::Predicates,
        ),
        (
            Limits {
                max_positions: 0,
                ..default
            },
            Resource::Positions,
        ),
        (
            Limits {
                max_links: 0,
                ..default
            },
            Resource::Links,
        ),
        (
            Limits {
                max_value_entries: 0,
                ..default
            },
            Resource::ValueEntries,
        ),
        (
            Limits {
                max_rounds: 1,
                ..default
            },
            Resource::Rounds,
        ),
        (
            Limits {
                max_inspected_bytes: 0,
                ..default
            },
            Resource::InspectedBytes,
        ),
    ] {
        let result = analyze(&program, limits);
        assert!(
            matches!(result.status(), Status::Stopped(stop) if stop.resource == resource && stop.observed > stop.limit)
        );
        assert_eq!(result.arguments().count(), 0);
        assert_eq!(result.domain(&signature("p", 1), 0), &Domain::Unknown);
    }
    let complete = analyze(&program, default);
    let work = complete.statistics().work;
    assert_eq!(
        analyze(
            &program,
            Limits {
                max_work: work,
                ..default
            }
        )
        .status(),
        Status::FixedPoint
    );
    assert!(
        matches!(analyze(&program, Limits { max_work: work - 1, ..default }).status(), Status::Stopped(stop) if stop.resource == Resource::Work)
    );
}

#[test]
fn finite_width_and_deep_symbols_widen_only_affected_arguments() {
    let program = source("p(1).p(2).q(X):-p(X).independent(7).");
    let result = analyze(
        &program,
        Limits {
            max_values_per_argument: 1,
            ..Limits::default()
        },
    );
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(
        result.argument(&signature("p", 1), 0).unwrap().widening(),
        Some(Widening::ValueWidth)
    );
    assert_eq!(result.domain(&signature("q", 1), 0), &Domain::Unknown);
    assert_eq!(numbers(&result, "independent", 0, 1), BTreeSet::from([7]));
    let mut deep = Symbol::Number(1);
    for _ in 0..1_000 {
        deep = Symbol::Tuple(vec![deep]);
    }
    let program = Program::of_nodes([fact("deep", deep)]);
    let result = analyze(&program, Limits::default());
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(
        result
            .argument(&signature("deep", 1), 0)
            .unwrap()
            .widening(),
        Some(Widening::SymbolSize)
    );
    let program = source("p(f(1,2)).");
    for limits in [
        Limits {
            max_symbol_nodes: 2,
            ..Limits::default()
        },
        Limits {
            max_symbol_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_symbol_bytes: 0,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            analyze(&program, limits).domain(&signature("p", 1), 0),
            &Domain::Unknown
        );
    }
    assert!(
        matches!(analyze(&program, Limits { max_symbol_nodes: 3, max_symbol_depth: 2, max_symbol_bytes: 1, ..Limits::default() }).domain(&signature("p", 1), 0), Domain::Finite(values) if values.len()==1)
    );
}

fn atom(name: &str, term: Term) -> Atom {
    Atom {
        sign: Sign::Positive,
        name: Name::new(name).unwrap(),
        arguments: Arguments::Single(vec![term]),
    }
}
fn fact(name: &str, value: Symbol) -> WithProvenance<Statement> {
    WithProvenance::constructed(Statement::Rule(Rule::new(
        atom(name, value.into()),
        Body::new([]),
    )))
}
fn variable() -> Term {
    Term::Variable(Variable::Named(VarName::new("X").unwrap()))
}
fn positive(name: &str) -> BodyElement {
    BodyElement::Literal(Literal {
        negation: DefaultNegation::None,
        inner: LiteralInner::Atom(WithProvenance::constructed(atom(name, variable()))),
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn abstract_arguments_cover_independent_concrete_positive_fixed_points(
        facts in any::<u16>(), rules in prop::collection::vec((0_usize..4,0_usize..4,0_usize..4),0..16),
        width in 0_usize..5,
    ) {
        let names = ["a", "b", "c", "d"];
        let mut statements = Vec::new();
        let mut concrete = [[false; 4]; 4];
        for (predicate, name) in names.iter().enumerate() {
            for (value, present) in concrete[predicate].iter_mut().enumerate() {
                if facts & (1 << (predicate * 4 + value)) != 0 {
                    *present = true;
                    statements.push(fact(name, Symbol::Number(i32::try_from(value).unwrap())));
                }
            }
        }
        for &(head, left, right) in &rules {
            statements.push(WithProvenance::constructed(Statement::Rule(Rule::new(
                atom(names[head], variable()), Body::new([positive(names[left]), positive(names[right])]),
            ))));
        }
        loop {
            let previous = concrete;
            for &(head, left, right) in &rules {
                for value in 0..4 { concrete[head][value] |= previous[left][value] && previous[right][value]; }
            }
            if concrete == previous { break; }
        }
        let program = Program::of_nodes(statements);
        let result = analyze(&program, Limits { max_values_per_argument: width, ..Limits::default() });
        prop_assert_eq!(result.status(), Status::FixedPoint);
        for (predicate, row) in concrete.iter().enumerate() {
            for (value, present) in row.iter().enumerate() {
                if *present {
                    prop_assert!(result.domain(&signature(names[predicate],1),0).permits(&Symbol::Number(i32::try_from(value).unwrap())));
                }
            }
        }
    }
}
