//! When a table that only narrows a probe is worth preparing.

use super::*;

fn cost(entries: u128, mask_words: u128, preparation_bytes: u128) -> TableCost {
    TableCost {
        entries,
        mask_words,
        preparation_bytes,
    }
}

#[test]
fn a_table_is_small_up_to_the_mask_bound() {
    // 1440 distinct keys over 1440 rows: 1440 masks of 45 words.
    assert!(cost(1_440, 64_800, 0).small());
    assert!(cost(1, 65_536, 0).small());
    assert!(!cost(1, 65_537, 0).small());
}

#[test]
fn rows_read_repay_a_table_at_eight_words_a_row() {
    assert!(!cost(1, 801, 0).repaid(100));
    assert!(cost(1, 800, 0).repaid(100));
    assert!(!cost(1, 1, 0).repaid(0));
    assert!(cost(1, u128::from(u64::MAX) * 8, 0).repaid(u64::MAX));
}

#[test]
fn a_table_fits_within_the_entries_and_the_bytes_left() {
    let allowance = |max_entries, max_bytes| table::Limits {
        max_entries,
        max_bytes,
        max_work: 0,
    };
    // Ten entries, and a preparation charged 400 bytes beside a relation of
    // 50; the masks' own words are not what is weighed.
    let table = cost(10, 1_000_000, 400);
    assert!(table.fits(50, &allowance(10, 450)));
    assert!(!table.fits(50, &allowance(9, 450)));
    assert!(!table.fits(50, &allowance(10, 449)));
    assert!(!table.fits(51, &allowance(10, 450)));
}

#[test]
fn a_declined_selection_enters_its_relations_count_once() {
    // 3000 distinct keys want 3000 masks of 94 words: not small, and no row
    // has been read in the table's place, so the selection is declined. The
    // first decline enters the relation's count and charges it to the
    // workspace; the second finds it.
    let mut catalog = super::super::super::SupportCatalog::default();
    for value in 0..3_000 {
        let atom = Atom::new(
            Predicate::new("big", 1).unwrap(),
            vec![Value::Number(value)],
        )
        .unwrap();
        catalog = catalog
            .insert(
                &atom,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
    }
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let relations = catalog
        .snapshot(&limits, &mut counters, location())
        .unwrap();
    let support = Support::completed(
        &relations,
        JoinStrategy::Indexed,
        &limits,
        &counters,
        location(),
    )
    .unwrap();
    let flat =
        AtomPattern::new(Predicate::new("big", 1).unwrap(), vec![Term::Variable(0)]).unwrap();
    let pattern = PositivePattern::Flat((&flat).into());
    let binding: [Option<Value>; 1] = [None];
    let values = binding.as_slice().into();
    let rows = support
        .resolve(pattern.atom(), &limits, &mut counters, location())
        .unwrap();
    let permitted = [TermRef::from(&Value::Number(8))];
    let finite = FiniteDomains {
        values: &permitted,
        variables: &[(0, 0..1)],
    };
    let live = support.live.get();
    for _ in 0..2 {
        let selected = support
            .select_domains_after(
                rows,
                pattern,
                values,
                finite,
                GroundingWork::new(&limits, &mut counters, location()),
            )
            .unwrap();
        assert!(matches!(selected, DomainSelection::Declined(0)));
        assert_eq!(support.live.get() - live, size_of::<Declined<'_>>());
    }
    support.count_scanned(0, 3_000);
    support.count_scanned(0, 500);
    let workspace = support.tables.get().unwrap();
    assert_eq!(workspace.declined.borrow().len(), 1);
    assert_eq!(workspace.declined.borrow()[0].rows, 3_500);
    assert!(workspace.indices.borrow().is_empty());
}
