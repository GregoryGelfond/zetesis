//! Inherited-key extraction covers lowered local generators and structure.
use super::*;

#[test]
fn parsed_family_keys_include_structured_and_generated_reads() {
    for text in [
        "q(N) :- d(X,U), N=#count{f(X+1,Y):p(Y)}.",
        "q(N) :- d(X,U), N=#count{Y:p(f(X),Y)}.",
        "q(N) :- d(X,U), N=#count{Y:p(f(X+1),Y)}.",
        "q(N) :- d(X,U), N=#count{Y:Y=0..X}.",
    ] {
        let source = Source::new(SourceId::new(113), text.into()).unwrap();
        let parsed =
            themelios_syntax::parse::parse(&source, themelios_syntax::dialect::Dialect::Clingo);
        assert!(parsed.diagnostics().is_empty(), "{text}");
        let raised = themelios_program::raise::raise(&parsed);
        assert!(raised.diagnostics().is_empty(), "{text}");
        let Statement::Rule(rule) = raised.program().statements().next().unwrap().get() else {
            unreachable!()
        };
        with_compiler(ExpansionLimits::default(), |compiler| {
            let compiled = compiler.rule(rule, Vec::new(), None).unwrap();
            let outer = compiled
                .body
                .iter()
                .find_map(|literal| match literal {
                    LiteralIr::Atom(DefaultNegation::None, atom) => Some(*atom),
                    _ => None,
                })
                .expect("the flat outer d(X,U) witness");
            let outer = compiler
                .source
                .pattern_ref(outer, compiler.limits, compiler.counters, compiler.location)
                .unwrap();
            let mut terms = outer.terms().iter();
            let Some(zetesis_core::TemplateTerm::Variable(read)) = terms.next() else {
                panic!("the first inherited d(X,U) slot: {text}")
            };
            let Some(zetesis_core::TemplateTerm::Variable(unread)) = terms.next() else {
                panic!("the second inherited d(X,U) slot: {text}")
            };
            assert!(terms.next().is_none());
            let family = compiled
                .body
                .iter()
                .find_map(|literal| match literal {
                    LiteralIr::Aggregate(aggregate) => Some(aggregate),
                    _ => None,
                })
                .unwrap();
            assert_eq!(family.family_inputs, [read], "{text}");
            assert!(!family.family_inputs.contains(&unread), "{text}");
            assert!(
                !family.family_inputs.contains(&family.binding.unwrap()),
                "{text}"
            );
        });
    }
}

fn recursive_family(compiler: &mut Compiler<'_>, inherited: usize) -> AggregateIr {
    let LiteralIr::Aggregate(mut source) = aggregate(0) else {
        unreachable!()
    };
    source.elements.push(AggregateElementIr {
        family: crate::formula_ir::LocalFamily(0),
        key: AggregateKey::Tuple(vec![CoreTerm::Variable(5)]),
        condition: vec![
            LiteralIr::Range {
                target: 4,
                lower: variable(inherited),
                upper: variable(2),
                binder: true,
            },
            LiteralIr::Bind {
                target: 5,
                value: variable(4),
            },
            comparison_guard(&[2]),
            conditional_read(compiler, inherited),
        ],
        variables: 6,
        required: false,
    });
    source
}

#[test]
fn recursive_family_key_excludes_local_generators_and_unread_outer_slots() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let source = recursive_family(compiler, 1);
        compiler.assignment_scope(&source).unwrap();
        // Slots 4/5 are locally generated, slot 3 is an unrelated inherited
        // value, and slot 0 is this producer's output. Only 1/2 are read.
        assert_eq!(
            compiler.assignment_family_inputs(&source, 4).unwrap(),
            [1, 2]
        );
    });
}

#[test]
fn recursive_self_read_is_rejected_before_family_key_construction() {
    with_compiler(ExpansionLimits::default(), |compiler| {
        let source = recursive_family(compiler, 0);
        let failure = compiler.assignment_scope(&source).unwrap_err();
        assert!(matches!(
            failure,
            FormulaFailure::UnsafeVariable { variable: 0, .. }
        ));
    });
}
