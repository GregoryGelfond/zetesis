//! Support postings are kept only for the columns a join can bind: a predicate
//! no join reads keeps none, and every probe a join makes finds its posting.

use super::{Observer, compile};
use zetesis_themelios::FormulaLimits;

/// Total index entries and probes that bound an unindexed column.
fn receipts(source: &str) -> (u64, u64) {
    let observer = Observer::default();
    compile(source, &FormulaLimits::default(), Some(&observer)).expect("admitted");
    let records = observer.records.borrow();
    let sum = |field: fn(&super::Record) -> Option<u64>| -> u64 {
        records.iter().filter_map(field).sum()
    };
    (
        sum(|record| record.work.support_index_entries),
        sum(|record| record.work.unindexed_probes),
    )
}

#[test]
fn a_predicate_no_join_reads_keeps_no_postings() {
    // u is read by nothing: its 1,600 rows keep no postings. Factorization can
    // bind X and Y through the head, so a keeps its one column: 40 entries.
    let (entries, unindexed) = receipts("{ a(1..40) }.\nu(X, Y) :- a(X), a(Y).\n");
    assert_eq!(entries, 40);
    assert_eq!(unindexed, 0);
}

#[test]
fn a_column_holding_a_lone_variable_keeps_no_postings() {
    // Y occurs once in the rule: no join binds b's second column.
    let (entries, unindexed) = receipts("{ b(1..20, 1..3) }.\nc(X) :- b(X, Y).\n");
    assert_eq!(entries, 60);
    assert_eq!(unindexed, 0);
}

#[test]
fn a_joined_column_keeps_its_postings() {
    // X joins a and b: one posting per row of each; c is read by nothing.
    let (entries, unindexed) = receipts("{ a(1..40) }.\n{ b(1..40) }.\nc(X) :- a(X), b(X).\n");
    assert_eq!(entries, 80);
    assert_eq!(unindexed, 0);
}

#[test]
fn every_kind_of_join_finds_its_posting() {
    // Each predicate b is read only at the named construct, with a column the
    // enclosing rule has already bound, so a probe of that column needs it.
    for source in [
        // An aggregate element condition.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\nn(X) :- a(X), #count{ Y : b(X, Y) } >= 1.\n",
        // A choice element condition.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\n{ c(X, Y) : b(X, Y) } :- a(X).\n",
        // A conditional literal, its consequent and its condition each
        // joined on the outer X.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\n{ e(1..6, 1..2) }.\nall(X) :- a(X), b(X, Y) : e(X, Y).\n",
        // A default-negated witness: some b(X, f(_)) must not hold.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\np(X) :- a(X), not b(X, f(_)).\n",
        // A projected rule body.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\nc(X) :- a(X), b(X, Y).\n#project c/1.\n",
        // An atom projection whose condition joins a and b on X.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\nc(X) :- a(X).\n#project c(X) : a(X), b(X, Y).\n",
        // A constraint whose comparison reads a joined column (totality).
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\n:- a(X), b(X, Y), Y > X + 8.\n",
        // An optimization element.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\n#minimize { Y, X : a(X), b(X, Y) }.\n",
        // A constant argument.
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\nf(X) :- a(X), b(X, 2).\n",
    ] {
        let (_, unindexed) = receipts(source);
        assert_eq!(unindexed, 0, "{source}");
    }
}

#[test]
fn a_streamed_constraint_finds_its_posting() {
    use zetesis_themelios::{AdmissionOptions, ExpansionLimits, prepare_formula};
    let observer = Observer::default();
    prepare_formula(
        "{ a(1..6) }.\n{ b(1..6, 1..2) }.\nc(X) :- a(X).\n:- c(X), b(X, 2).\n".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .expect("prepared")
    .ground_hybrid_with_observer(Some(&observer))
    .expect("admitted");
    let unindexed: u64 = observer
        .records
        .borrow()
        .iter()
        .filter_map(|record| record.work.unindexed_probes)
        .sum();
    assert_eq!(unindexed, 0);
}
