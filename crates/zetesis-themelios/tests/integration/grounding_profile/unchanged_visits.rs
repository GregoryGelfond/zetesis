//! Without a whole-program plan, a rich producer is traversed after the first
//! round only when a predicate it reads gained rows.

use super::{Observer, compile};
use zetesis_themelios::FormulaLimits;

/// Support rounds, unchanged visits, join rows and binding snapshots, summed
/// over every phase of `source`'s admission.
fn receipts(source: &str) -> [u64; 4] {
    let observer = Observer::default();
    compile(source, &FormulaLimits::default(), Some(&observer)).expect("admitted");
    let records = observer.records.borrow();
    let sum = |field: fn(&super::Record) -> Option<u64>| -> u64 {
        records.iter().filter_map(field).sum()
    };
    [
        sum(|record| record.work.support_rounds),
        sum(|record| record.work.support_unchanged_visits),
        sum(|record| record.work.join_rows),
        sum(|record| record.work.binding_snapshots),
    ]
}

/// Facts `q`, a recursion of `depth` rounds, and the choice over the facts.
fn recursion(depth: usize, choice: bool) -> String {
    let mut source = format!("q(1..3). s(0). s(Y) :- s(X), X < {depth}, Y = X + 1.");
    if choice {
        source.push_str(" {a(X)} :- q(X).");
    }
    source
}

#[test]
fn a_choice_over_facts_adds_no_join_rows_in_later_rounds() {
    // The choice reads only facts, whose rows are new in the second round
    // alone; each extra round of the recursion is one more unchanged visit and
    // adds none of the choice's join rows or snapshots.
    let [rounds, unchanged, rows, snapshots] = receipts(&recursion(4, true));
    let [
        deeper_rounds,
        deeper_unchanged,
        deeper_rows,
        deeper_snapshots,
    ] = receipts(&recursion(5, true));
    let [_, _, plain_rows, plain_snapshots] = receipts(&recursion(4, false));
    let [_, _, deeper_plain_rows, deeper_plain_snapshots] = receipts(&recursion(5, false));
    assert_eq!(deeper_rounds, rounds + 1);
    assert_eq!(deeper_unchanged, unchanged + 1);
    assert_eq!(deeper_rows - deeper_plain_rows, rows - plain_rows);
    assert_eq!(
        deeper_snapshots - deeper_plain_snapshots,
        snapshots - plain_snapshots
    );
}

#[test]
fn a_rich_producer_over_a_growing_predicate_is_unaffected_only_in_the_final_round() {
    // `reach` gains a row in every round but the last, so the choice that
    // reads it is traversed in each of those rounds and skipped once.
    let source = "edge(0,1). edge(1,2). edge(2,3). reach(0). \
        reach(Y) :- reach(X), edge(X,Y). {pick(X)} :- reach(X).";
    let [_, unchanged, _, _] = receipts(source);
    assert_eq!(unchanged, 1);
    let observer = Observer::default();
    let admitted = compile(source, &FormulaLimits::default(), Some(&observer)).unwrap();
    let picks = admitted
        .atoms()
        .iter()
        .filter(|atom| atom.predicate().name() == "pick")
        .count();
    assert_eq!(picks, 4);
}
