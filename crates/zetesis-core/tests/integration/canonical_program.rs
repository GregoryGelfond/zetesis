//! Program admission consumes syntax into borrowed canonical execution views.

use zetesis_core::catalog::{Error as CatalogError, TermRef};
use zetesis_core::{
    AdmissionError, AdmissionLimits, Atom, AtomPattern, Filter, PatternRef, Predicate, Program,
    Template, TemplateTerm, Term, Value, ValueLimits, ValueNode,
};

fn fact(value: Value) -> Template {
    let pattern =
        AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Constant(value)]).unwrap();
    Template::new(Some(pattern), vec![], vec![], vec![], vec![])
}

#[test]
fn interned_subterms_do_not_expand_the_domain() {
    let value = Value::from_nodes(
        vec![
            ValueNode::Function {
                name: "f".into(),
                sign: zetesis_core::Sign::Positive,
                arity: 1,
            },
            ValueNode::Symbol("child".into()),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let program = Program::new(vec![fact(value.clone())], AdmissionLimits::default()).unwrap();
    assert_eq!(program.domain().len(), 1);
    assert_eq!(program.domain().at(0).unwrap(), value);
    assert!(
        program
            .domain()
            .binary_search(&Value::Symbol("child".into()))
            .is_err()
    );
    let constant = program
        .templates()
        .at(0)
        .unwrap()
        .head()
        .unwrap()
        .terms()
        .at(0)
        .unwrap();
    assert_eq!(
        constant,
        TemplateTerm::Constant(program.domain().at(0).unwrap())
    );
}

#[test]
fn admitted_views_share_the_constant_payload() {
    let template = fact(Value::Symbol("a-longer-owned-symbol".into()));
    let program =
        Program::new(vec![template.clone(), template], AdmissionLimits::default()).unwrap();
    let domain = program.domain().at(0).unwrap();
    let zetesis_core::ValueNodeRef::Symbol(domain_text) = domain.descriptor() else {
        panic!("symbol domain");
    };
    let mut signatures = Vec::new();
    for template in program.templates() {
        let pattern = template.head().unwrap();
        signatures.push(pattern.predicate().name());
        let TemplateTerm::Constant(value) = pattern.terms().at(0).unwrap() else {
            panic!("closed fact");
        };
        let zetesis_core::ValueNodeRef::Symbol(text) = value.descriptor() else {
            panic!("symbol constant");
        };
        assert!(std::ptr::eq(text, domain_text));
    }
    assert!(std::ptr::eq(signatures[0], signatures[1]));
    assert_eq!(program.templates().len(), 2);
}

#[test]
fn admission_preserves_explicitly_admitted_depth() {
    let depth = 256;
    let mut nodes = vec![ValueNode::Tuple { arity: 1 }; depth - 1];
    nodes.push(ValueNode::Number(7));
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_depth: depth,
            ..ValueLimits::default()
        },
    )
    .unwrap();
    let program = Program::new(vec![fact(value)], AdmissionLimits::default()).unwrap();
    assert_eq!(program.domain().at(0).unwrap().depth(), depth);
}

#[test]
fn admission_bounds_template_metadata() {
    let pattern = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
    let template = Template::new(Some(pattern), vec![], vec![], vec![], vec![]);
    let error = Program::new(
        vec![template; 100],
        AdmissionLimits {
            max_bytes: 1024,
            ..AdmissionLimits::default()
        },
    )
    .unwrap_err();
    let AdmissionError::Canonical {
        error: CatalogError::Storage { required, limit },
        ..
    } = error
    else {
        panic!("named storage refusal");
    };
    assert_eq!(limit, 1024);
    assert!(required > limit as u128);
}

#[test]
fn admitted_keys_preserve_substitution_identity() {
    let predicate = Predicate::new("p", 3).unwrap();
    let pattern = AtomPattern::new(
        predicate.clone(),
        vec![
            Term::Variable(0),
            Term::Constant(Value::String("s".into())),
            Term::Variable(0),
        ],
    )
    .unwrap();
    let template = Template::new(
        Some(pattern.clone()),
        vec![pattern.clone()],
        vec![],
        vec![],
        vec![],
    );
    let program = Program::new(vec![template], AdmissionLimits::default()).unwrap();
    let assignment = [Value::Number(9)];
    let expected = Atom::new(
        predicate,
        vec![
            Value::Number(9),
            Value::String("s".into()),
            Value::Number(9),
        ],
    )
    .unwrap();
    let admitted = program
        .templates()
        .at(0)
        .unwrap()
        .head()
        .unwrap()
        .key(assignment.as_slice())
        .unwrap();
    let ingress = PatternRef::from(&pattern)
        .key(assignment.as_slice())
        .unwrap();
    assert_eq!(admitted, ingress);
    assert!(admitted.compare(&expected).is_eq());
    assert_eq!(admitted.value(0), admitted.value(2));
    let missing: &[Option<TermRef<'_>>] = &[None];
    assert_eq!(
        program
            .templates()
            .at(0)
            .unwrap()
            .head()
            .unwrap()
            .key(missing)
            .unwrap_err()
            .variable,
        0
    );
}

#[test]
fn admitted_key_comparison_stops_at_each_work_boundary() {
    let value = Value::String("compared text".into());
    let template = fact(value.clone());
    let program = Program::new(vec![template], AdmissionLimits::default()).unwrap();
    let empty: &[Value] = &[];
    let key = program
        .templates()
        .at(0)
        .unwrap()
        .head()
        .unwrap()
        .key(empty)
        .unwrap();
    let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap();
    let mut total = 0;
    assert!(
        key.compare_identity_with(&atom, || {
            total += 1;
            Ok::<_, usize>(())
        })
        .unwrap()
        .is_eq()
    );
    assert!(total > 0);
    for stop in 0..total {
        let mut calls = 0;
        assert_eq!(
            key.compare_identity_with(&atom, || {
                let at = calls;
                calls += 1;
                if at == stop { Err(stop) } else { Ok(()) }
            }),
            Err(stop)
        );
        assert_eq!(calls, stop + 1);
    }
}

#[test]
fn filter_views_share_the_ingress_evaluator() {
    let filter = Filter::Neq(
        Term::Constant(Value::Number(7)),
        Term::Constant(Value::String("7".into())),
    );
    let program = Program::new(
        vec![Template::new(
            None,
            vec![],
            vec![],
            vec![],
            vec![filter.clone()],
        )],
        AdmissionLimits::default(),
    )
    .unwrap();
    let empty: &[Value] = &[];
    assert_eq!(
        program
            .templates()
            .at(0)
            .unwrap()
            .filters()
            .at(0)
            .unwrap()
            .evaluate(empty),
        filter.evaluate(empty)
    );
    assert!(filter.evaluate(empty).unwrap());
}
