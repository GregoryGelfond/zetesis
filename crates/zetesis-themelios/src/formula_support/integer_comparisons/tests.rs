use super::*;
use crate::formula_support::Evaluation;
use crate::formula_support::testing::{Fixture, binding};
use crate::{FormulaFailure, FormulaLimits, FormulaResource};
use proptest::prelude::*;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use zetesis_core::Value;

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(41),
        span: Span::empty(ByteOffset::new(3)),
    })
}

/// A test operand node; argument positions name earlier nodes.
#[derive(Clone, Copy, Debug)]
enum Node {
    Number(i32),
    Symbol(&'static str),
    Variable(usize),
    Negate(usize),
    Absolute(usize),
    Binary(BinaryOp, usize, usize),
}

fn expression(fixture: &mut Fixture, nodes: &[Node]) -> Expression {
    Expression {
        nodes: nodes
            .iter()
            .map(|node| match *node {
                Node::Number(value) => {
                    Operation::Constant(fixture.scalar(&Value::Number(value), location()))
                }
                Node::Symbol(name) => {
                    Operation::Constant(fixture.scalar(&Value::Symbol(name.into()), location()))
                }
                Node::Variable(slot) => Operation::Variable(slot),
                Node::Negate(argument) => Operation::Unary(UnaryOp::Negate, argument),
                Node::Absolute(argument) => Operation::Absolute(argument),
                Node::Binary(operator, left, right) => Operation::Binary(operator, left, right),
            })
            .collect(),
    }
}

/// What deciding one comparison came to, with the general evaluator's verdict
/// on the same binding.
struct Decided {
    /// The form's verdict at its first decision, which compiles it.
    first: Option<bool>,
    /// The form's verdict at its second decision, on the same row.
    second: Result<Option<bool>, FormulaFailure>,
    /// Work charged by the second decision alone.
    work: u64,
    /// The general evaluator's verdict.
    general: General,
}

/// The general evaluator's outcome on the same binding.
#[derive(Debug, PartialEq, Eq)]
enum General {
    Holds(bool),
    /// An evaluation failure: undefined or overflowing arithmetic.
    Undefined,
    /// Any other refusal, by its message.
    Refused(String),
}

/// Decide `left relation right` on `values` twice with one form, the second
/// time under `allowance` further work units, then with the general evaluator.
fn decide(
    left: &[Node],
    relation: Relation,
    right: &[Node],
    values: &[Option<Value>],
    allowance: u64,
) -> Decided {
    let mut fixture = Fixture::default();
    let left = expression(&mut fixture, left);
    let right = expression(&mut fixture, right);
    fixture.with(location(), |_, computation, counters| {
        let source = binding(values, computation, counters, location());
        let unbounded = FormulaLimits::default();
        let mut forms = IntegerComparisons::default();
        let mut lease = computation.lease();
        let first = forms
            .decide(
                &mut lease,
                0,
                (0, 1),
                ([&left, &right], relation),
                &source,
                Context::new(&*computation, &unbounded, counters, location()),
            )
            .unwrap();
        let before = counters.accounting.work;
        let bounded = FormulaLimits {
            max_work: before.saturating_add(allowance),
            ..FormulaLimits::default()
        };
        let second = forms.decide(
            &mut lease,
            0,
            (0, 1),
            ([&left, &right], relation),
            &source,
            Context::new(&*computation, &bounded, counters, location()),
        );
        let work = counters.accounting.work - before;
        let general = match Evaluation::default().source_comparison(
            ([&left, &right], relation),
            &source,
            Context::new(computation, &unbounded, counters, location()),
        ) {
            Ok(holds) => General::Holds(holds),
            Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })) => {
                General::Undefined
            }
            Err(error) => General::Refused(error.to_string()),
        };
        Decided {
            first,
            second,
            work,
            general,
        }
    })
}

const X: Node = Node::Variable(0);
const Y: Node = Node::Variable(1);

fn numbers(values: &[i32]) -> Vec<Option<Value>> {
    values
        .iter()
        .map(|value| Some(Value::Number(*value)))
        .collect()
}

/// `X + 1 < Y`: four nodes and two variable reads.
fn successor_below() -> ([Node; 3], [Node; 1]) {
    ([X, Node::Number(1), Node::Binary(BinaryOp::Add, 0, 1)], [Y])
}

#[test]
fn a_decided_comparison_charges_one_unit_per_node_and_read() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), u64::MAX);
    assert_eq!(decided.first, Some(true));
    assert_eq!(decided.second.unwrap(), Some(true));
    assert_eq!(decided.work, 6);
}

#[test]
fn the_one_charge_is_admitted_at_its_units() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), 6);
    assert_eq!(decided.second.unwrap(), Some(true));
}

#[test]
fn the_one_charge_is_refused_as_work_below_its_units() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), 5);
    assert!(
        matches!(
            decided.second,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ),
        "{:?}",
        decided.second
    );
}

#[test]
fn a_nonnumeric_operand_declines_without_a_charge() {
    let (left, right) = successor_below();
    let values = [Some(Value::Symbol("a".into())), Some(Value::Number(3))];
    let decided = decide(&left, Relation::Lt, &right, &values, u64::MAX);
    assert_eq!(decided.first, None);
    assert_eq!(decided.second.unwrap(), None);
    assert_eq!(decided.work, 0);
}

#[test]
fn an_unbound_operand_declines_rather_than_failing() {
    let values = [Some(Value::Number(1)), None];
    let decided = decide(&[X], Relation::Lt, &[Y], &values, u64::MAX);
    assert_eq!(decided.second.unwrap(), None);
    assert_eq!(decided.work, 0);
    // The general evaluator reports the unbound variable.
    assert_eq!(
        decided.general,
        General::Refused("unsafe formula variable 1".into())
    );
}

#[test]
fn a_zero_divisor_declines_to_the_general_failure() {
    let quotient = [X, Y, Node::Binary(BinaryOp::Div, 0, 1)];
    let decided = decide(
        &quotient,
        Relation::Lt,
        &[Node::Number(1)],
        &numbers(&[4, 0]),
        u64::MAX,
    );
    assert_eq!(decided.second.unwrap(), None);
    assert_eq!(decided.work, 0);
    assert_eq!(decided.general, General::Undefined);
}

#[test]
fn an_overflow_declines_to_the_general_failure() {
    let (left, _) = successor_below();
    let decided = decide(
        &left,
        Relation::Gt,
        &[Node::Number(0)],
        &numbers(&[i32::MAX]),
        u64::MAX,
    );
    assert_eq!(decided.second.unwrap(), None);
    assert_eq!(decided.general, General::Undefined);
}

#[test]
fn a_nonnumeric_constant_leaves_the_literal_without_a_form() {
    let decided = decide(
        &[X],
        Relation::Lt,
        &[Node::Symbol("a")],
        &numbers(&[1]),
        u64::MAX,
    );
    assert_eq!(decided.first, None);
    assert_eq!(decided.second.unwrap(), None);
    assert_eq!(decided.general, General::Holds(true));
}

#[test]
fn an_operand_beyond_the_bound_has_no_form() {
    // X followed by sixteen negations: seventeen nodes.
    let mut long = vec![X];
    long.extend((0..MAX_NODES).map(Node::Negate));
    let decided = decide(&long, Relation::Eq, &[X], &numbers(&[5]), u64::MAX);
    assert_eq!(decided.first, None);
    assert_eq!(decided.general, General::Holds(true));
}

#[test]
fn an_operand_at_the_bound_has_a_form() {
    // X followed by fifteen negations: sixteen nodes, an odd count of
    // negations, so the operand is -5.
    let mut long = vec![X];
    long.extend((0..MAX_NODES - 1).map(Node::Negate));
    let decided = decide(&long, Relation::Eq, &[X], &numbers(&[5]), u64::MAX);
    assert_eq!(decided.first, Some(false));
    assert_eq!(decided.general, General::Holds(false));
}

#[test]
fn a_forward_argument_leaves_the_operand_without_a_form() {
    let forward = Expression {
        nodes: vec![Operation::Absolute(1), Operation::Variable(0)],
    };
    assert_eq!(shape(&forward), Some(WithoutForm::ArgumentPosition));
    let itself = Expression {
        nodes: vec![
            Operation::Variable(0),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
    };
    assert_eq!(shape(&itself), Some(WithoutForm::ArgumentPosition));
}

/// The state a literal's first decision leaves, over integer values.
fn compiled_state(left: &[Node], right: &[Node]) -> LiteralState {
    let mut fixture = Fixture::default();
    let left = expression(&mut fixture, left);
    let right = expression(&mut fixture, right);
    fixture.with(location(), |_, computation, counters| {
        let source = binding(&numbers(&[1, 2]), computation, counters, location());
        let limits = FormulaLimits::default();
        let mut forms = IntegerComparisons::default();
        let mut lease = computation.lease();
        forms
            .decide(
                &mut lease,
                0,
                (0, 1),
                ([&left, &right], Relation::Lt),
                &source,
                Context::new(&*computation, &limits, counters, location()),
            )
            .unwrap();
        forms.states[0]
    })
}

#[test]
fn a_literal_without_a_form_records_why() {
    let (left, right) = successor_below();
    assert!(matches!(
        compiled_state(&left, &right),
        LiteralState::Form(0)
    ));
    assert_eq!(
        compiled_state(&[X], &[Node::Symbol("a")]),
        LiteralState::WithoutForm(WithoutForm::NonnumericConstant)
    );
    let mut long = vec![X];
    long.extend((0..MAX_NODES).map(Node::Negate));
    assert_eq!(
        compiled_state(&long, &[X]),
        LiteralState::WithoutForm(WithoutForm::Bound)
    );
}

#[test]
fn a_refused_reservation_refuses_the_decision() {
    // Preparing a form reserves leased storage like the join's other
    // containers; a refusal is a refusal, never a fallback that would let two
    // joins of one literal take different routes.
    let mut fixture = Fixture::default();
    let (left, right) = successor_below();
    let left = expression(&mut fixture, &left);
    let right = expression(&mut fixture, &right);
    let refused = fixture.with(location(), |_, computation, counters| {
        let source = binding(&numbers(&[1, 3]), computation, counters, location());
        let limits = FormulaLimits {
            max_support_bytes: 0,
            ..FormulaLimits::default()
        };
        let mut forms = IntegerComparisons::default();
        let mut lease = computation.lease();
        forms.decide(
            &mut lease,
            0,
            (0, 1),
            ([&left, &right], Relation::Lt),
            &source,
            Context::new(&*computation, &limits, counters, location()),
        )
    });
    assert!(
        matches!(
            refused,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ),
        "{refused:?}"
    );
}

#[test]
fn states_steps_and_forms_take_eight_bytes_each() {
    // The module documentation's retained-capacity formula.
    assert_eq!(size_of::<LiteralState>(), 8);
    assert_eq!(size_of::<Step>(), 8);
    assert_eq!(size_of::<Form>(), 8);
}

/// A random operand tree, as the compiler folds a term: post-order, so every
/// node is an argument of a later one and the last node is the root.
#[derive(Clone, Debug)]
enum Tree {
    Number(i32),
    Variable(usize),
    Negate(Box<Tree>),
    Absolute(Box<Tree>),
    Binary(BinaryOp, Box<Tree>, Box<Tree>),
}

impl Tree {
    fn nodes(&self) -> Vec<Node> {
        let mut nodes = Vec::new();
        self.fold(&mut nodes);
        nodes
    }

    fn fold(&self, nodes: &mut Vec<Node>) -> usize {
        let node = match self {
            Self::Number(value) => Node::Number(*value),
            Self::Variable(slot) => Node::Variable(*slot),
            Self::Negate(argument) => Node::Negate(argument.fold(nodes)),
            Self::Absolute(argument) => Node::Absolute(argument.fold(nodes)),
            Self::Binary(operator, left, right) => {
                let left = left.fold(nodes);
                let right = right.fold(nodes);
                Node::Binary(*operator, left, right)
            }
        };
        nodes.push(node);
        nodes.len() - 1
    }
}

fn operand_strategy() -> impl Strategy<Value = Vec<Node>> {
    let leaf = prop_oneof![
        prop_oneof![
            Just(i32::MIN),
            Just(-1),
            Just(0),
            Just(1),
            Just(2),
            Just(i32::MAX),
            -50_i32..50
        ]
        .prop_map(Tree::Number),
        (0_usize..3).prop_map(Tree::Variable),
    ];
    let operator = prop_oneof![
        Just(BinaryOp::Add),
        Just(BinaryOp::Sub),
        Just(BinaryOp::Mul),
        Just(BinaryOp::Div),
        Just(BinaryOp::Mod),
        Just(BinaryOp::Pow),
        Just(BinaryOp::BitAnd),
        Just(BinaryOp::BitOr),
        Just(BinaryOp::BitXor),
    ];
    leaf.prop_recursive(3, 12, 2, move |inner| {
        prop_oneof![
            inner.clone().prop_map(|tree| Tree::Negate(Box::new(tree))),
            inner
                .clone()
                .prop_map(|tree| Tree::Absolute(Box::new(tree))),
            (operator.clone(), inner.clone(), inner).prop_map(|(operator, left, right)| {
                Tree::Binary(operator, Box::new(left), Box::new(right))
            }),
        ]
    })
    .prop_map(|tree| tree.nodes())
}

fn value_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        prop_oneof![
            Just(i32::MIN),
            Just(-1),
            Just(0),
            Just(1),
            Just(3),
            Just(i32::MAX),
            -20_i32..20
        ]
        .prop_map(Value::Number),
        Just(Value::Symbol("a".into())),
    ]
}

fn relation_strategy() -> impl Strategy<Value = Relation> {
    prop_oneof![
        Just(Relation::Eq),
        Just(Relation::Neq),
        Just(Relation::Lt),
        Just(Relation::Le),
        Just(Relation::Gt),
        Just(Relation::Ge),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_form_agrees_with_the_general_evaluator_or_declines(
        left in operand_strategy(),
        right in operand_strategy(),
        relation in relation_strategy(),
        values in proptest::collection::vec(value_strategy(), 3),
    ) {
        let integers = values.iter().all(|value| matches!(value, Value::Number(_)));
        let values: Vec<Option<Value>> = values.into_iter().map(Some).collect();
        let decided = decide(&left, relation, &right, &values, u64::MAX);
        let second = decided.second.unwrap();
        prop_assert_eq!(decided.first, second);
        match second {
            // A decision is the general evaluator's verdict.
            Some(holds) => prop_assert_eq!(decided.general, General::Holds(holds)),
            // Over integer values a tree's form declines exactly when the
            // general evaluator fails.
            None if integers => prop_assert_eq!(decided.general, General::Undefined),
            None => prop_assert_eq!(decided.work, 0),
        }
    }
}

/// Decide `X + 1 < Y` on X = 1, Y = 3 in the frozen lane, whose vocabulary
/// admitted only 1 and 3: the operand value 2 was never admitted.
fn frozen_decision(max_support_bytes: usize) -> Result<Option<bool>, FormulaFailure> {
    let mut fixture = Fixture::default();
    let (left, right) = successor_below();
    let left = expression(&mut fixture, &left);
    let right = expression(&mut fixture, &right);
    fixture.with(location(), |_, computation, counters| {
        for value in [1, 3] {
            computation
                .import(
                    (&Value::Number(value)).into(),
                    &FormulaLimits::default(),
                    counters,
                    location(),
                )
                .unwrap();
        }
    });
    let (completed, _) = fixture.finish(location());
    let unbounded = FormulaLimits::default();
    let mut counters = crate::formula_support::Counters::default();
    let support = completed
        .snapshot(&unbounded, &mut counters, location())
        .unwrap();
    let queries = support
        .queries(
            crate::JoinStrategy::Indexed,
            &unbounded,
            &counters,
            location(),
        )
        .unwrap();
    let mut computation = queries.computation(location()).unwrap();
    assert!(matches!(
        computation.number(2, &unbounded, &mut counters, location()),
        Err(FormulaFailure::UnadmittedTerm { .. })
    ));
    let source = binding(
        &numbers(&[1, 3]),
        &mut computation,
        &mut counters,
        location(),
    );
    let limits = FormulaLimits {
        max_support_bytes,
        ..FormulaLimits::default()
    };
    let mut forms = IntegerComparisons::default();
    let mut lease = computation.lease();
    forms.decide(
        &mut lease,
        0,
        (0, 1),
        ([&left, &right], Relation::Lt),
        &source,
        Context::new(&computation, &limits, &mut counters, location()),
    )
}

#[test]
fn a_frozen_decision_needs_no_admitted_operand_values() {
    assert_eq!(
        frozen_decision(FormulaLimits::default().max_support_bytes).unwrap(),
        Some(true)
    );
}

#[test]
fn a_frozen_refused_reservation_refuses_rather_than_falling_back() {
    // Falling back would send the general evaluator to construct 2, which the
    // frozen vocabulary never admitted.
    let refused = frozen_decision(0);
    assert!(
        matches!(
            refused,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ),
        "{refused:?}"
    );
}
