use super::*;
use crate::observation::evaluate::test_support::work;
use zetesis_core::{Atom, Model, Predicate, Sign, Value, ValueLimits, ValueNode};
use zetesis_cpu::Cancellation;
use zetesis_test_support::programs::function;

fn model(nodes: Vec<ValueNode>) -> Model {
    let value = Value::from_nodes(
        nodes,
        ValueLimits {
            max_nodes: 16_384,
            max_depth: 4096,
            max_bytes: 16_777_216,
        },
    )
    .unwrap();
    Model::new([Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()]).unwrap()
}
fn argument(model: &Model) -> TermRef<'_> {
    model.atoms().at(0).unwrap().arguments().get(0).unwrap()
}
fn no_construction(work: &mut Work<'_>) {
    work.construction.max_bytes = 0;
    work.limits.max_local_bytes = 0;
    work.limits.max_symbol_nodes = 0;
    work.limits.max_symbol_depth = 0;
    work.limits.max_symbol_bytes = 0;
}
fn pattern(
    shape: ValueNodeRef<'_>,
    arguments: Vec<Operand>,
) -> (std::sync::Arc<crate::metadata::MetadataVocabulary>, Operand) {
    let mut admission =
        crate::metadata::Admission::new(crate::MetadataStorageLimits::default(), 0).unwrap();
    let shape = admission.constructor(shape, 0).unwrap();
    (
        admission.finish(0).unwrap(),
        Operand::Construct(shape, arguments),
    )
}
fn runtime<'input, 'work, 'cancel>(
    metadata: &'input crate::metadata::MetadataVocabulary,
    model: &'input Model,
    work: &'work mut Work<'cancel>,
) -> Interpreter<'input, 'work, 'cancel> {
    let metadata = metadata.read_with(|| Ok::<_, ()>(())).unwrap();
    let terms = zetesis_core::catalog::DerivedTerms::new_with(
        &[metadata.catalog(), model.catalog().read()],
        work.limits.max_term_storage_bytes,
        || work.step(1),
    )
    .unwrap();
    let keys = super::super::anonymous::Keys::new(terms.read());
    Interpreter {
        metadata,
        terms,
        work,
        keys,
    }
}
fn binding<'input>(variables: usize, context: &mut Interpreter<'input, '_, '_>) -> Binding<'input> {
    Binding::new(
        variables,
        None,
        context.terms.read(),
        context.work.limits.max_term_storage_bytes,
        context.work,
    )
    .unwrap()
}

#[test]
fn nested_capture_borrows_a_model_subtree_beyond_output_limits() {
    let mut nodes = vec![function("f", 1)];
    nodes.extend((0..512).map(|_| function("g", 1)));
    nodes.push(ValueNode::String("large model text".repeat(1024)));
    let model = model(nodes);
    let value = argument(&model);
    let (metadata, pattern) = pattern(
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Positive,
            arity: 1,
        },
        vec![Operand::Variable(0)],
    );
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(1, &mut context);
    no_construction(context.work);
    let before = context.terms.storage_bytes();
    let mut undo = Vec::new();
    assert!(matches_value(&pattern, value, &mut binding, &mut undo, true, &mut context).unwrap());
    let captured = binding.input(0).expect("model capture must borrow");
    assert_eq!(captured.expanded_nodes(), 513);
    assert_eq!(captured, value.child(0).unwrap());
    assert_eq!(undo, [0]);
    assert_eq!(context.work.local_bytes, 0);
    assert_eq!(context.terms.storage_bytes(), before);
}

#[test]
fn mismatching_constructor_does_not_read_its_deep_child() {
    let mut nodes = vec![function("g", 1)];
    nodes.extend((0..512).map(|_| function("f", 1)));
    nodes.push(ValueNode::String("unused".repeat(1024)));
    let model = model(nodes);
    let (metadata, pattern) = pattern(
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Positive,
            arity: 1,
        },
        vec![Operand::Any],
    );
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(0, &mut context);
    no_construction(context.work);
    context.work.statistics.work = 0;
    context.work.limits.max_work = 8;
    assert!(
        !matches_value(
            &pattern,
            argument(&model),
            &mut binding,
            &mut Vec::new(),
            true,
            &mut context
        )
        .unwrap()
    );
}

#[test]
fn repeated_nested_capture_keeps_borrowed_undo_on_mismatch() {
    let model = model(vec![
        ValueNode::Tuple { arity: 2 },
        ValueNode::String("first".into()),
        ValueNode::String("second".into()),
    ]);
    let (metadata, pattern) = pattern(
        ValueNodeRef::Tuple { arity: 2 },
        vec![Operand::Variable(0), Operand::Variable(0)],
    );
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(1, &mut context);
    no_construction(context.work);
    let mut undo = Vec::new();
    assert!(
        !matches_value(
            &pattern,
            argument(&model),
            &mut binding,
            &mut undo,
            true,
            &mut context
        )
        .unwrap()
    );
    assert_eq!(
        binding.input(0).unwrap(),
        argument(&model).child(0).unwrap()
    );
    assert_eq!(undo, [0]);
    assert_eq!(context.work.local_bytes, 0);
}

#[test]
fn constructor_matching_preserves_nullary_normalization_and_type() {
    let (metadata, pattern) = pattern(
        ValueNodeRef::Function {
            name: "x",
            sign: Sign::Positive,
            arity: 0,
        },
        vec![],
    );
    for (node, expected) in [
        (ValueNode::Symbol("x".into()), true),
        (ValueNode::String("x".into()), false),
        (
            ValueNode::Function {
                name: "x".into(),
                sign: Sign::Negative,
                arity: 0,
            },
            false,
        ),
    ] {
        let model = model(vec![node]);
        let cancellation = Cancellation::default();
        let mut work = work(&cancellation);
        let mut context = runtime(&metadata, &model, &mut work);
        let mut binding = binding(0, &mut context);
        no_construction(context.work);
        assert_eq!(
            matches_value(
                &pattern,
                argument(&model),
                &mut binding,
                &mut Vec::new(),
                true,
                &mut context
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn canonical_equality_is_iterative_for_deep_foreign_inputs() {
    let mut nodes = vec![function("f", 1); 512];
    nodes.push(ValueNode::Number(7));
    let left = model(nodes.clone());
    let right = model(nodes);
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    no_construction(&mut work);
    assert!(read::equal(argument(&left), argument(&right), &mut work).unwrap());
    assert_eq!(work.local_bytes, 0);
}

#[test]
fn every_canonical_equality_work_cutoff_refuses_before_a_result() {
    let nodes = vec![
        function("f", 2),
        ValueNode::String("typed text".into()),
        ValueNode::Number(7),
    ];
    let left = model(nodes.clone());
    let right = model(nodes);
    let cancellation = Cancellation::default();
    let mut complete = work(&cancellation);
    assert!(read::equal(argument(&left), argument(&right), &mut complete).unwrap());
    for cutoff in 0..complete.statistics.work {
        let mut bounded = work(&cancellation);
        bounded.limits.max_work = cutoff;
        assert!(matches!(
            read::equal(argument(&left), argument(&right), &mut bounded)
                .unwrap_err()
                .kind(),
            super::super::ErrorKind::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(bounded.statistics.work <= cutoff);
        assert_eq!(bounded.local_bytes, 0);
    }
}

#[test]
fn generated_capture_charges_and_releases_only_its_logical_binding() {
    let model = Model::default();
    let (metadata, _) = pattern(ValueNodeRef::Tuple { arity: 0 }, vec![]);
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(1, &mut context);
    let key = context.number(7).unwrap();
    let metric = context.metric(&key).unwrap();
    let before = context.terms.storage_bytes();
    let mut undo = Vec::new();
    assert!(
        bind_key(
            &Operand::Variable(0),
            &key,
            &mut binding,
            &mut undo,
            &mut context
        )
        .unwrap()
    );
    assert_eq!(context.work.local_bytes, metric.payload());
    assert_eq!(context.terms.storage_bytes(), before);
    assert_eq!(
        binding
            .value(0, context.terms.read(), context.work)
            .unwrap()
            .unwrap()
            .descriptor(),
        ValueNodeRef::Number(7)
    );
    binding.clear(0, context.work);
    assert_eq!(context.work.local_bytes, 0);
    assert_eq!(undo, [0]);
}

#[test]
fn generated_capture_refuses_before_binding_an_unfunded_slot() {
    let model = Model::default();
    let (metadata, _) = pattern(ValueNodeRef::Tuple { arity: 0 }, vec![]);
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(1, &mut context);
    let key = context.number(7).unwrap();
    context.work.limits.max_local_bytes = 0;
    let mut undo = Vec::new();
    assert!(matches!(
        bind_key(
            &Operand::Variable(0),
            &key,
            &mut binding,
            &mut undo,
            &mut context
        )
        .unwrap_err()
        .kind(),
        super::super::ErrorKind::Limit {
            resource: Resource::LocalBytes,
            ..
        }
    ));
    assert!(!binding.is_bound(0));
    assert!(undo.is_empty());
    assert_eq!(context.work.local_bytes, 0);
}

#[test]
fn generated_alias_wildcard_does_not_register_a_deep_child() {
    let mut nodes = vec![function("f", 1)];
    nodes.extend((0..512).map(|_| function("g", 1)));
    nodes.push(ValueNode::String("unused".repeat(1024)));
    let model = model(nodes);
    let (metadata, pattern) = pattern(
        ValueNodeRef::Function {
            name: "f",
            sign: Sign::Positive,
            arity: 1,
        },
        vec![Operand::Any],
    );
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(0, &mut context);
    let alias = context.input(argument(&model)).unwrap();
    let before = context.terms.storage_bytes();
    no_construction(context.work);
    context.work.statistics.work = 0;
    context.work.limits.max_work = 32;
    assert!(
        bind_key(
            &pattern,
            &alias,
            &mut binding,
            &mut Vec::new(),
            &mut context
        )
        .unwrap()
    );
    assert_eq!(context.terms.storage_bytes(), before);
    assert_eq!(context.work.local_bytes, 0);
}

#[test]
fn generated_nested_mismatch_keeps_one_owned_undo_charge() {
    let model = model(vec![
        ValueNode::Tuple { arity: 2 },
        ValueNode::Number(7),
        ValueNode::Number(8),
    ]);
    let (metadata, pattern) = pattern(
        ValueNodeRef::Tuple { arity: 2 },
        vec![Operand::Variable(0), Operand::Variable(0)],
    );
    let cancellation = Cancellation::default();
    let mut work = work(&cancellation);
    let mut context = runtime(&metadata, &model, &mut work);
    let mut binding = binding(1, &mut context);
    let alias = context.input(argument(&model)).unwrap();
    let mut undo = Vec::new();
    assert!(!bind_key(&pattern, &alias, &mut binding, &mut undo, &mut context).unwrap());
    let key = binding.key(0).unwrap();
    assert_eq!(context.value(&key).descriptor(), ValueNodeRef::Number(7));
    let metric = context.metric(&key).unwrap();
    assert_eq!(context.work.local_bytes, metric.payload());
    assert_eq!(undo, [0]);
    binding.clear(0, context.work);
    assert_eq!(context.work.local_bytes, 0);
}
