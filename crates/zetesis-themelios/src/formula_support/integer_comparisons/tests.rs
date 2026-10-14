use std::cell::Cell;

use super::*;
use crate::formula_ir::Operation;
use crate::formula_support::Evaluation;
use crate::formula_support::testing::{Fixture, binding};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork, Profile};
use crate::{FormulaLimits, FormulaResource, GroundingObserver, ProgramSite};
use proptest::prelude::*;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use themelios_program::program::Relation;
use themelios_program::term::{BinaryOp, UnaryOp};
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
    Unary(UnaryOp, usize),
    Absolute(usize),
    Binary(BinaryOp, usize, usize),
}

/// The operand nodes over `nodes`, their constants admitted to the fixture.
fn operations(fixture: &mut Fixture, nodes: &[Node]) -> Vec<Operation> {
    nodes
        .iter()
        .map(|node| match *node {
            Node::Number(value) => {
                Operation::Constant(fixture.scalar(&Value::Number(value), location()))
            }
            Node::Symbol(name) => {
                Operation::Constant(fixture.scalar(&Value::Symbol(name.into()), location()))
            }
            Node::Variable(slot) => Operation::Variable(slot),
            Node::Unary(operator, argument) => Operation::Unary(operator, argument),
            Node::Absolute(argument) => Operation::Absolute(argument),
            Node::Binary(operator, left, right) => Operation::Binary(operator, left, right),
        })
        .collect()
}

/// The literal `left relation right`, built by the compiler's constructor over
/// the fixture's catalog: every immediate of its form is read from the
/// catalog the general evaluator then reads.
fn literal(fixture: &mut Fixture, left: &[Node], relation: Relation, right: &[Node]) -> Comparison {
    let (left, right) = (operations(fixture, left), operations(fixture, right));
    fixture.comparison(left, relation, right, location())
}

/// Records the work of the one phase a decision runs in.
#[derive(Default)]
struct Observer(Cell<Option<GroundingWork>>);
impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(Some(work));
    }
}

/// One decision of a comparison, what it counted, and the general
/// evaluator's verdict on the same binding.
#[derive(Debug)]
struct Outcome {
    /// The decision under the allowance.
    decision: Result<Option<bool>, FormulaFailure>,
    /// Work the decision counted.
    work: u64,
    /// Expression evaluations it recorded as entered.
    evaluations: u64,
    /// Expression nodes it recorded as reached.
    nodes: u64,
    /// Rows it decided, and rows it declined.
    rows_decided: u64,
    rows_declined: u64,
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

/// Decide `left relation right` on `values` under `allowance` further work
/// units, then with the general evaluator.
fn decide(
    left: &[Node],
    relation: Relation,
    right: &[Node],
    values: &[Option<Value>],
    allowance: u64,
) -> Outcome {
    let mut fixture = Fixture::default();
    let comparison = literal(&mut fixture, left, relation, right);
    fixture.with(location(), |_, computation, counters| {
        let source = binding(values, computation, counters, location());
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let previous = std::mem::replace(&mut counters.observed, profile.work());
        let before = counters.accounting.work;
        let bounded = FormulaLimits {
            max_work: before.saturating_add(allowance),
            ..FormulaLimits::default()
        };
        let decision = profile.phase(GroundingPhase::RuleInstantiation, None, || {
            super::decide(
                &comparison,
                &source,
                Context::new(&*computation, &bounded, counters, location()),
            )
        });
        let work = counters.accounting.work - before;
        counters.observed = previous;
        let receipt = observer.0.get().unwrap();
        let general = match Evaluation::default().source_comparison(
            comparison.operands(),
            &source,
            Context::new(computation, &FormulaLimits::default(), counters, location()),
        ) {
            Ok(holds) => General::Holds(holds),
            Err(FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })) => {
                General::Undefined
            }
            Err(error) => General::Refused(error.to_string()),
        };
        Outcome {
            decision,
            work,
            evaluations: receipt.expression_evaluations.unwrap(),
            nodes: receipt.expression_nodes.unwrap(),
            rows_decided: receipt.integer_comparisons.unwrap(),
            rows_declined: receipt.integer_comparison_declines.unwrap(),
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

/// `X + 1 < Y`: four steps and two variable steps, a constant of six.
fn successor_below() -> ([Node; 3], [Node; 1]) {
    ([X, Node::Number(1), Node::Binary(BinaryOp::Add, 0, 1)], [Y])
}

fn refused_as_work(decision: &Result<Option<bool>, FormulaFailure>) -> bool {
    matches!(
        decision,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    )
}

#[test]
fn a_decided_row_counts_its_constant_both_evaluations_and_every_step() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), u64::MAX);
    assert_eq!(decided.decision.as_ref().unwrap(), &Some(true));
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (6, 2, 4)
    );
    assert_eq!((decided.rows_decided, decided.rows_declined), (1, 0));
    assert_eq!(decided.general, General::Holds(true));
}

#[test]
fn a_left_operand_decline_counts_only_its_first_step() {
    // X is a symbol: the attempt stops at the left operand's first read, its
    // two units, one evaluation entered and one step reached.
    let (left, right) = successor_below();
    let values = [Some(Value::Symbol("a".into())), Some(Value::Number(3))];
    let decided = decide(&left, Relation::Lt, &right, &values, u64::MAX);
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (2, 1, 1)
    );
    assert_eq!((decided.rows_decided, decided.rows_declined), (0, 1));
}

#[test]
fn a_right_operand_decline_counts_the_left_operand_whole() {
    // Y is a symbol: the left operand's three steps and four units are
    // performed, then the right operand's read: two units, one step.
    let (left, right) = successor_below();
    let values = [Some(Value::Number(1)), Some(Value::Symbol("a".into()))];
    let decided = decide(&left, Relation::Lt, &right, &values, u64::MAX);
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (6, 2, 4)
    );
    assert_eq!((decided.rows_decided, decided.rows_declined), (0, 1));
}

#[test]
fn a_failing_operation_counts_the_steps_up_to_it() {
    // X + 1 overflows at the left operand's third step.
    let (left, right) = successor_below();
    let decided = decide(
        &left,
        Relation::Lt,
        &right,
        &numbers(&[i32::MAX, 3]),
        u64::MAX,
    );
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (4, 1, 3)
    );
}

#[test]
fn the_check_admits_an_attempt_at_its_constant() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), 6);
    assert_eq!(decided.decision.unwrap(), Some(true));
}

#[test]
fn the_check_refuses_one_below_its_constant_counting_nothing() {
    let (left, right) = successor_below();
    let decided = decide(&left, Relation::Lt, &right, &numbers(&[1, 3]), 5);
    assert!(refused_as_work(&decided.decision), "{:?}", decided.decision);
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (0, 0, 0)
    );
}

#[test]
fn a_row_that_would_decline_is_checked_for_the_whole_constant() {
    // The check precedes the reads: a row that would decline after two units
    // is refused at the same allowance as one that would be decided.
    let (left, right) = successor_below();
    let values = [Some(Value::Symbol("a".into())), Some(Value::Number(3))];
    let decided = decide(&left, Relation::Lt, &right, &values, 5);
    assert!(refused_as_work(&decided.decision), "{:?}", decided.decision);
}

#[test]
fn an_unbound_operand_declines_rather_than_failing() {
    let values = [Some(Value::Number(1)), None];
    let decided = decide(&[X], Relation::Lt, &[Y], &values, u64::MAX);
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
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
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
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
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
    assert_eq!(decided.general, General::Undefined);
}

#[test]
fn a_literal_without_a_form_is_left_to_the_general_path_unchecked() {
    let decided = decide(&[X], Relation::Lt, &[Node::Symbol("a")], &numbers(&[1]), 0);
    assert_eq!(decided.decision.as_ref().unwrap(), &None);
    assert_eq!(
        (decided.work, decided.evaluations, decided.nodes),
        (0, 0, 0)
    );
    assert_eq!((decided.rows_decided, decided.rows_declined), (0, 0));
    // Numbers precede constants in the ASP order.
    assert_eq!(decided.general, General::Holds(true));
}

/// A random operand tree, as the compiler folds a term: post-order, so every
/// node is an argument of a later one and the last node is the root.
#[derive(Clone, Debug)]
enum Tree {
    Number(i32),
    Variable(usize),
    Unary(UnaryOp, Box<Tree>),
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
            Self::Unary(operator, argument) => Node::Unary(*operator, argument.fold(nodes)),
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

fn leaf() -> BoxedStrategy<Tree> {
    prop_oneof![
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
    ]
    .boxed()
}

/// Trees of exactly `size` nodes, over every operator.
fn tree(size: usize) -> BoxedStrategy<Tree> {
    if size <= 1 {
        return leaf();
    }
    let unary = prop_oneof![
        (
            prop_oneof![Just(UnaryOp::Negate), Just(UnaryOp::BitwiseNot)],
            tree(size - 1)
        )
            .prop_map(|(operator, argument)| Tree::Unary(operator, Box::new(argument))),
        tree(size - 1).prop_map(|argument| Tree::Absolute(Box::new(argument))),
    ];
    if size == 2 {
        return unary.boxed();
    }
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
    let binary = (1..size - 1)
        .prop_flat_map(move |left| (operator.clone(), tree(left), tree(size - 1 - left)))
        .prop_map(|(operator, left, right)| {
            Tree::Binary(operator, Box::new(left), Box::new(right))
        });
    prop_oneof![unary, binary].boxed()
}

/// Operands of one to six nodes.
fn operand_strategy() -> impl Strategy<Value = Vec<Node>> {
    (1_usize..=6)
        .prop_flat_map(tree)
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

/// The constant an attempt is checked for: the steps and variable steps of
/// both operands.
fn constant(left: &[Node], right: &[Node]) -> u64 {
    left.iter()
        .chain(right)
        .map(|node| {
            if matches!(node, Node::Variable(_)) {
                2
            } else {
                1
            }
        })
        .sum()
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
        let constant = constant(&left, &right);
        // A decision is the general evaluator's verdict, and its attempt
        // performed every step of the form. A decline counts no more than the
        // check admitted, and over integer values a tree's form declines
        // exactly when the general evaluator fails.
        if let Some(holds) = decided.decision.unwrap() {
            prop_assert_eq!(decided.general, General::Holds(holds));
            prop_assert_eq!(decided.work, constant);
        } else {
            prop_assert!(decided.work <= constant);
            if integers {
                prop_assert_eq!(decided.general, General::Undefined);
            }
        }
    }
}

#[test]
fn a_frozen_decision_needs_no_admitted_operand_values() {
    // Decide `X + 1 < Y` on X = 1, Y = 3 in the frozen lane, whose vocabulary
    // admitted only 1 and 3: the operand value 2 was never admitted.
    let mut fixture = Fixture::default();
    let (left, right) = successor_below();
    let comparison = literal(&mut fixture, &left, Relation::Lt, &right);
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
    let limits = FormulaLimits::default();
    let mut counters = crate::formula_support::Counters::default();
    let support = completed
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let queries = support
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location())
        .unwrap();
    let mut computation = queries.computation(location()).unwrap();
    assert!(matches!(
        computation.number(2, &limits, &mut counters, location()),
        Err(FormulaFailure::UnadmittedTerm { .. })
    ));
    let source = binding(
        &numbers(&[1, 3]),
        &mut computation,
        &mut counters,
        location(),
    );
    let decision = super::decide(
        &comparison,
        &source,
        Context::new(&computation, &limits, &mut counters, location()),
    );
    assert_eq!(decision.unwrap(), Some(true));
}
