use super::*;
use crate::formula_support::Counters;
use crate::formula_support::testing::Fixture;
use crate::{FormulaLimits, FormulaResource};
use themelios_base::span::Location;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};
use zetesis_core::{Atom, Predicate, Value, ValueLimits, ValueNode};

fn location() -> Location {
    Location {
        source: SourceId::new(98),
        span: Span::empty(ByteOffset::new(0)),
    }
}

struct Contributions {
    terms: TermTable,
    atoms: SourceSelection,
    grouped: CoordinateMap<GroundKey, (Measure, usize)>,
}

impl Contributions {
    fn shuffled(computation: &mut Computation<'_, '_>, counters: &mut Counters) -> Self {
        let limits = FormulaLimits::default();
        let mut terms = TermTable::new(computation, &limits, counters, location()).unwrap();
        let mut atoms = SourceSelection::new(computation, &limits, counters, location()).unwrap();
        let mut grouped = CoordinateMap::new(computation, &limits, counters, location()).unwrap();
        for (values, condition) in [(&[2][..], 20), (&[1, 9][..], 19), (&[1][..], 10)] {
            let mut nodes = vec![ValueNode::Tuple {
                arity: values.len(),
            }];
            nodes.extend(values.iter().copied().map(ValueNode::Number));
            let value = Value::from_nodes(nodes, ValueLimits::default()).unwrap();
            let key = computation
                .import((&value).into(), &limits, counters, location())
                .unwrap();
            let position = terms
                .insert(&key, None, computation, &limits, counters, location())
                .unwrap();
            grouped
                .insert(
                    GroundKey::Tuple(position),
                    (Measure::Numeric(1), condition),
                    None,
                    Context::new(&*computation, &limits, counters, location()),
                )
                .unwrap();
        }
        // String precedes Symbol in storage order; ASP comparison reverses
        // those classes. Neither order follows this discovery sequence.
        for (value, condition) in [
            (Value::Symbol("a".into()), 60),
            (Value::String("z".into()), 50),
            (Value::Number(3), 30),
        ] {
            let atom = Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap();
            let source = computation
                .atom_ref((&atom).into(), &limits, counters, location())
                .unwrap();
            let (position, _) = atoms
                .insert(
                    &source,
                    (FormulaResource::Atoms, limits.theory.max_atoms),
                    computation,
                    &limits,
                    counters,
                    location(),
                )
                .unwrap();
            grouped
                .insert(
                    GroundKey::Atom(position),
                    (Measure::Numeric(1), condition),
                    None,
                    Context::new(&*computation, &limits, counters, location()),
                )
                .unwrap();
        }
        Self {
            terms,
            atoms,
            grouped,
        }
    }

    fn conditions(&self) -> Vec<usize> {
        self.grouped.iter().map(|entry| entry.1.1).collect()
    }
}

fn conditions(entries: &Entries<GroundKey, (Measure, usize)>) -> Vec<usize> {
    entries.iter().map(|entry| entry.1.1).collect()
}

#[test]
fn contributions_use_typed_key_order() {
    Fixture::default().with(location(), |_, computation, counters| {
        let contributions = Contributions::shuffled(computation, counters);
        assert_eq!(contributions.conditions(), [20, 19, 10, 60, 50, 30]);
        let Contributions {
            terms,
            atoms,
            grouped,
        } = contributions;
        let entries = ordered(
            grouped,
            &terms,
            &atoms,
            Context::new(computation, &FormulaLimits::default(), counters, location()),
        )
        .unwrap();
        // List keys [1], [1,9], [2] compare components before tuple arity.
        // Associated conditions stay paired when keys move.
        assert_eq!(conditions(&entries), [10, 19, 20, 30, 50, 60]);
    });
}

#[test]
fn ordering_refusal_releases_unpublished_entries() {
    let (needed, released) = Fixture::default().with(location(), |_, computation, counters| {
        let Contributions {
            terms,
            atoms,
            grouped,
        } = Contributions::shuffled(computation, counters);
        let limits = FormulaLimits::default();
        let observer = computation.lease();
        let available = computation
            .allowance(&observer, &limits, location())
            .unwrap();
        let before = counters.accounting.work;
        let entries = ordered(
            grouped,
            &terms,
            &atoms,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        let needed = counters.accounting.work - before;
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            available
        );
        drop(entries);
        let released = computation
            .allowance(&observer, &limits, location())
            .unwrap()
            - available;
        assert!(released > 0);
        (needed, released)
    });
    for cutoff in 0..needed {
        Fixture::default().with(location(), |_, computation, counters| {
            let Contributions { terms, atoms, grouped } = Contributions::shuffled(computation, counters);
            let limits = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..Default::default()
            };
            let observer = computation.lease();
            let available = computation.allowance(&observer, &limits, location()).unwrap();
            assert!(matches!(ordered(grouped, &terms, &atoms,
                Context::new(computation, &limits, counters, location())),
                Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location: found })
                    if observed > limit && limit == u128::from(limits.max_work) && found == location()));
            // The consumed input is gone; no output or temporary sort buffer
            // survives refusal. Canonical payload and other lookup metadata stay.
            assert_eq!(computation.allowance(&observer, &limits, location()).unwrap(), available + released);
        });
    }
}
