use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use zetesis_core::catalog::TermRef;
use zetesis_core::{AtomPattern, Predicate, Term, Value, ValueNodeRef};

use super::*;
use crate::formula_support::{Support, SupportCatalog};

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

#[test]
fn assigned_atoms_preserve_argument_occurrences() {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut catalog = SupportCatalog::default();
    let pattern = crate::formula_support::testing::admit_pattern(
        &mut catalog,
        &AtomPattern::new(
            Predicate::new("assigned", 3).unwrap(),
            vec![
                Term::Variable(0),
                Term::Constant(Value::Number(4)),
                Term::Variable(0),
            ],
        )
        .unwrap(),
        &mut counters,
        location(),
    );
    let (relations, mut append) = catalog.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let mut binding = Binding::new(&computation, &limits, &mut counters, location()).unwrap();
    binding
        .extend_scope(1, &computation, &limits, &mut counters, location())
        .unwrap();
    let key = computation
        .number(7, &limits, &mut counters, location())
        .unwrap();
    binding
        .set(0, &key, &limits, &mut counters, location())
        .unwrap();
    let pattern = computation
        .static_pattern(pattern, &limits, &mut counters, location())
        .unwrap();
    let source = computation
        .atom(pattern, &binding, &limits, &mut counters, location())
        .unwrap();
    let atom = computation
        .source_atom(&source, &limits, &mut counters, location())
        .unwrap();
    let values: Vec<_> = atom.values().iter().map(TermRef::descriptor).collect();
    assert_eq!(
        values,
        vec![
            ValueNodeRef::Number(7),
            ValueNodeRef::Number(4),
            ValueNodeRef::Number(7)
        ]
    );
    let view = binding
        .view(computation.read(), &limits, &mut counters, location())
        .unwrap();
    assert!(
        !computation
            .contains(
                pattern.key(view).unwrap(),
                &limits,
                &mut counters,
                location()
            )
            .unwrap()
    );
    computation
        .support(&source, &limits, &mut counters, location())
        .unwrap();
    let view = binding
        .view(computation.read(), &limits, &mut counters, location())
        .unwrap();
    assert!(
        computation
            .contains(
                pattern.key(view).unwrap(),
                &limits,
                &mut counters,
                location()
            )
            .unwrap()
    );
}
