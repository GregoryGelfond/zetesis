//! Captured count members retain their meaning across actual catalog appends.

use crate::formula_support::Context;

use themelios_base::source::{Source, SourceId};
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::DefaultNegation;
use zetesis_core::{Atom, AtomPattern, Predicate, Sign};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateLimits, Interpretation,
    Limits, Node, Theory, append_aggregate, models,
};

use super::atom;
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_count_plan::{Bounds, Collector, Input, Outcome, Request};
use crate::formula_head_aggregate::validate_group;
use crate::formula_ir::{
    ChoiceIr, Element, HeadElementKey, HeadLiteral, HeadMeasure, HeadOperand, LocalFamily,
};
use crate::formula_support::{Computation, Counters, SourceSelection, Support, SupportCatalog};
use crate::{AdmissionOptions, CountPlanLimits, ExpansionLimits, FormulaLimits, FormulaResource};

const SOURCE: &str = "{a;b}1.{c;d}1.2{a;b;c;d}2.{late}.";

struct Fixture {
    source: Source,
    owner: SupportCatalog,
    counters: Counters,
    atoms: SourceSelection,
    names: Vec<String>,
    nodes: Vec<Node>,
    heads: Vec<usize>,
    roots: Vec<usize>,
    collector: Collector,
}

impl Fixture {
    fn new(cancellation: &Cancellation) -> Self {
        let source = Source::new(SourceId::new(7), SOURCE.into()).unwrap();
        let collector = Collector::new(
            Request {
                limits: CountPlanLimits::default(),
                cancellation,
            },
            location(&source, "{a;b}1."),
        );
        let limits = FormulaLimits::default();
        let mut owner = SupportCatalog::default();
        let mut counters = Counters::default();
        let atoms = {
            let origin = location(&source, "{a;b}1.");
            let (relations, mut append) = owner.split(&limits, &mut counters, origin).unwrap();
            let support = Support::indexed(&relations, &limits, &counters, origin).unwrap();
            let computation = Computation::new(&mut append, &support);
            SourceSelection::new(&computation, &limits, &counters, origin).unwrap()
        };
        Self {
            source,
            owner,
            counters,
            atoms,
            names: Vec::new(),
            // The collector's unconditional activation is canonical true 1.
            nodes: vec![Node::False, Node::Implies(0, 0)],
            heads: Vec::new(),
            roots: Vec::new(),
            collector,
        }
    }

    fn push(&mut self, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    fn choice(&mut self, name: &str) {
        let limits = FormulaLimits::default();
        let origin = location(&self.source, "{late}.");
        let id = {
            let (relations, mut append) = self
                .owner
                .split(&limits, &mut self.counters, origin)
                .unwrap();
            let support = Support::indexed(&relations, &limits, &self.counters, origin).unwrap();
            let mut computation = Computation::new(&mut append, &support);
            let input = atom(name, Sign::Positive, vec![]);
            let source = computation
                .atom_ref((&input).into(), &limits, &mut self.counters, origin)
                .unwrap();
            self.atoms
                .insert(
                    &source,
                    (FormulaResource::Atoms, limits.theory.max_atoms),
                    &computation,
                    &limits,
                    &mut self.counters,
                    origin,
                )
                .unwrap()
                .0
        };
        self.names.push(name.to_owned());
        assert_eq!(id, self.heads.len());
        let head = self.push(Node::Atom(id));
        self.heads.push(head);
        let absent = self.push(Node::Implies(head, 0));
        let permission = self.push(Node::Or(head, absent));
        self.roots.push(permission);
    }

    fn capture(
        &mut self,
        members: &[usize],
        comparison: AggregateComparison,
        bound: i32,
        statement: &str,
        cancellation: &Cancellation,
    ) {
        let elements: Vec<_> = members
            .iter()
            .map(|&id| AggregateElement {
                weight: 1,
                condition: self.heads[id],
            })
            .collect();
        let within = append_aggregate(
            &mut self.nodes,
            &elements,
            comparison,
            i64::from(bound),
            AggregateLimits::default(),
            cancellation,
        )
        .unwrap()
        .root();
        // Match the ordinary head-bound constraint: an active body cannot
        // violate the actual lowered aggregate. No placeholder root certifies it.
        let outside = self.push(Node::Implies(within, 0));
        let violated = self.push(Node::And(1, outside));
        let asserted = self.push(Node::Implies(violated, 0));
        self.roots.push(asserted);
        let eligible: Vec<_> = members.iter().map(|&id| (self.heads[id], 1)).collect();
        let mut bounds = Bounds::new(members.len());
        bounds.guard(comparison, bound);
        let origin = location(&self.source, statement);
        let group = ChoiceIr {
            measure: HeadMeasure::Count,
            guards: vec![],
            elements: members
                .iter()
                .map(|&id| Element {
                    family: LocalFamily(id),
                    key: HeadElementKey::Atom,
                    head: HeadLiteral {
                        negation: DefaultNegation::None,
                        operand: HeadOperand::Atom(crate::formula_support::testing::admit_pattern(
                            &mut self.owner,
                            &AtomPattern::new(Predicate::new(&self.names[id], 0).unwrap(), vec![])
                                .unwrap(),
                            &mut self.counters,
                            origin,
                        )),
                    },
                    condition: vec![],
                    body_variables: 0,
                    variables: 0,
                })
                .collect(),
        };
        let bijection = {
            let limits = FormulaLimits::default();
            let (relations, mut append) = self
                .owner
                .split(&limits, &mut self.counters, origin)
                .unwrap();
            let support = Support::indexed(&relations, &limits, &self.counters, origin).unwrap();
            let mut computation = Computation::new(&mut append, &support);
            let assignment =
                Binding::new(&computation, &limits, &mut self.counters, origin).unwrap();
            let mut budget = Budget::new(
                ExpansionLimits::default(),
                AdmissionOptions::default().core_limits.max_templates,
            );
            validate_group(
                &group,
                &assignment,
                &support,
                &mut budget,
                Context::new(&mut computation, &limits, &mut self.counters, origin),
            )
            .unwrap()
            .unwrap()
        };
        self.collector.capture_group(
            &Input {
                body: 1,
                eligible: &eligible,
                bijection,
                nodes: &self.nodes,
                atom_count: self.atoms.len(),
                bounds,
                origins: &[origin],
                location: origin,
            },
            within,
            asserted,
        );
    }

    fn assert_atoms(&mut self, expected: &[Atom]) {
        let limits = FormulaLimits::default();
        let origin = location(&self.source, "{late}.");
        let (relations, mut append) = self
            .owner
            .split(&limits, &mut self.counters, origin)
            .unwrap();
        let support = Support::indexed(&relations, &limits, &self.counters, origin).unwrap();
        let computation = Computation::new(&mut append, &support);
        assert_eq!(self.atoms.len(), expected.len());
        for (id, expected) in expected.iter().enumerate() {
            let actual = self
                .atoms
                .atom(id, &computation, &limits, &mut self.counters, origin)
                .unwrap();
            assert!(actual.compare(expected).is_eq());
        }
    }
}

fn location(source: &Source, statement: &str) -> Location {
    let start = source.text().find(statement).unwrap();
    Location {
        source: source.id(),
        span: Span::new(
            ByteOffset::new(u32::try_from(start).unwrap()),
            ByteOffset::new(u32::try_from(start + statement.len()).unwrap()),
        )
        .unwrap(),
    }
}

fn holds(theory: &Theory, mask: usize, cancellation: &Cancellation) -> bool {
    let interpretation = Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|&id| mask & (1 << id) != 0),
    )
    .unwrap();
    models(theory, &interpretation, Limits::default(), cancellation).unwrap()
}

#[test]
fn captured_members_keep_their_meaning_after_catalog_growth() {
    let cancellation = Cancellation::default();
    let mut fixture = Fixture::new(&cancellation);
    fixture.choice("a");
    fixture.choice("b");
    assert_eq!(fixture.atoms.len(), 2);
    fixture.capture(
        &[0, 1],
        AggregateComparison::Le,
        1,
        "{a;b}1.",
        &cancellation,
    );

    // Unlike complete source admission, this actually appends new atoms after
    // the first capture. Subsequent captures use the enlarged same catalog.
    fixture.choice("c");
    fixture.choice("d");
    assert_eq!(fixture.atoms.len(), 4);
    fixture.capture(
        &[2, 3],
        AggregateComparison::Le,
        1,
        "{c;d}1.",
        &cancellation,
    );
    fixture.capture(
        &[0, 1, 2, 3],
        AggregateComparison::Eq,
        2,
        "2{a;b;c;d}2.",
        &cancellation,
    );
    fixture.choice("late");
    let expected: Vec<_> = ["a", "b", "c", "d", "late"]
        .map(|name| atom(name, Sign::Positive, vec![]))
        .into();
    fixture.assert_atoms(&expected);
    let atom_count = fixture.atoms.len();
    let theory = Theory::new(
        atom_count,
        fixture.nodes,
        fixture.roots,
        AdmissionLimits::default(),
    )
    .unwrap();
    let Outcome::Ready(plan) = fixture.collector.finish(&theory) else {
        panic!("the three actual source bounds must produce a count plan");
    };
    assert_eq!(plan.statistics().groups, 3);
    assert_eq!(plan.statistics().members, 8);
    assert_eq!(plan.consequence_count(), 2);
    assert!(plan.original_theory().same_instance(&theory));
    assert_eq!(plan.restriction().atom_count(), atom_count);
    for origin in plan.origins() {
        assert_eq!(origin.source, fixture.source.id());
        assert!(
            ["{a;b}1.", "{c;d}1.", "2{a;b;c;d}2."]
                .contains(&fixture.source.slice(origin.span).unwrap())
        );
    }

    // Check every interpretation, including either value of the late atom.
    // This also checks that the collector received real bounds for its members.
    for mask in 0_usize..1 << atom_count {
        let left = (mask & 0b00011).count_ones();
        let right = (mask & 0b01100).count_ones();
        let original = holds(&theory, mask, &cancellation);
        let restricted = holds(plan.restriction(), mask, &cancellation);
        assert_eq!(original, left <= 1 && right <= 1 && left + right == 2);
        assert_eq!(restricted, left >= 1 && right >= 1);
        assert!(!original || restricted);
    }
}
