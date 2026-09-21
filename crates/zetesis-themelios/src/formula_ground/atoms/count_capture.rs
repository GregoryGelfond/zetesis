//! Captured count members retain their meaning across actual catalog appends.

use std::collections::BTreeMap;

use themelios_base::source::{Source, SourceId};
use themelios_base::span::{ByteOffset, Location, Span};
use zetesis_core::Sign;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, AggregateComparison, AggregateElement, AggregateLimits, Interpretation,
    Limits, Node, Theory, append_aggregate, models,
};

use super::{Catalog, atom, commit, find, finish, intern};
use crate::CountPlanLimits;
use crate::formula_count_plan::{Bounds, Collector, Input, Outcome, Request};

const SOURCE: &str = "{a;b}1.{c;d}1.2{a;b;c;d}2.{late}.";

struct Fixture {
    source: Source,
    atoms: Catalog,
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
        Self {
            source,
            atoms: Catalog::default(),
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
        let id = intern(&mut self.atoms, &atom(name, Sign::Positive, vec![]));
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
        let eligible = members.iter().map(|&id| (self.heads[id], 1)).collect();
        let mut bounds = Bounds::new(members.len());
        bounds.guard(comparison, bound);
        let origin = location(&self.source, statement);
        commit(&mut self.atoms);
        self.collector.capture_group(
            Input {
                body: 1,
                eligible: &eligible,
                // Unsigned ordinary choices use implicit whole-atom keys.
                tuple_keys: BTreeMap::new(),
                nodes: &self.nodes,
                atoms: self.atoms.atoms(),
                bounds,
                origins: &[origin],
                location: origin,
            },
            within,
            asserted,
        );
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
    for (id, atom) in expected.iter().enumerate() {
        assert_eq!(find(&fixture.atoms, atom), Some(id));
    }
    let atoms = finish(fixture.atoms);
    assert_eq!(atoms, expected);
    let theory = Theory::new(
        atoms.len(),
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
    assert_eq!(plan.restriction().atom_count(), atoms.len());
    for origin in plan.origins() {
        assert_eq!(origin.source, fixture.source.id());
        assert!(
            ["{a;b}1.", "{c;d}1.", "2{a;b;c;d}2."]
                .contains(&fixture.source.slice(origin.span).unwrap())
        );
    }

    // Check every interpretation, including either value of the late atom.
    // This also checks that the collector received real bounds for its members.
    for mask in 0_usize..1 << atoms.len() {
        let left = (mask & 0b00011).count_ones();
        let right = (mask & 0b01100).count_ones();
        let original = holds(&theory, mask, &cancellation);
        let restricted = holds(plan.restriction(), mask, &cancellation);
        assert_eq!(original, left <= 1 && right <= 1 && left + right == 2);
        assert_eq!(restricted, left >= 1 && right >= 1);
        assert!(!original || restricted);
    }
}
