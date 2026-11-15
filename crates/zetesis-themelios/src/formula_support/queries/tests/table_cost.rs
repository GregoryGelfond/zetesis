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
    // 50 and tables that hold 30: the tables may take half of the room, so
    // the preparation is weighed twice, and the masks' own words are not
    // what is weighed.
    let table = cost(10, 1_000_000, 400);
    assert!(table.fits(50, 30, &allowance(10, 880)));
    assert!(!table.fits(50, 30, &allowance(9, 880)));
    assert!(!table.fits(50, 30, &allowance(10, 879)));
    assert!(!table.fits(51, 30, &allowance(10, 880)));
    assert!(!table.fits(50, 31, &allowance(10, 880)));
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

/// Unary relations `t0`, `t1`, … of ten rows each, with the pattern that
/// reads each: a small table apiece.
fn tiny_relations(relations: usize) -> (super::super::super::SupportCatalog, Vec<AtomPattern>) {
    let mut catalog = super::super::super::SupportCatalog::default();
    let mut patterns = Vec::new();
    for relation in 0..relations {
        let predicate = Predicate::new(format!("t{relation}"), 1).unwrap();
        for value in 0..10 {
            let atom = Atom::new(predicate.clone(), vec![Value::Number(value)]).unwrap();
            catalog = catalog
                .insert(
                    &atom,
                    &FormulaLimits::default(),
                    &mut Counters::default(),
                    location(),
                )
                .unwrap();
        }
        patterns.push(AtomPattern::new(predicate, vec![Term::Variable(0)]).unwrap());
    }
    (catalog, patterns)
}

/// What a selection by domains came to.
#[derive(Debug, PartialEq, Eq)]
enum Selected {
    Rows,
    Declined,
    Inapplicable,
    Refused,
}

/// What a selection of the value 8 over `pattern`'s relation comes to under
/// `limits`.
fn selects(support: &Support<'_>, pattern: &AtomPattern, limits: &FormulaLimits) -> Selected {
    let mut counters = Counters::default();
    let pattern = PositivePattern::Flat(pattern.into());
    let binding: [Option<Value>; 1] = [None];
    let Ok(rows) = support.resolve(pattern.atom(), limits, &mut counters, location()) else {
        return Selected::Refused;
    };
    let permitted = [TermRef::from(&Value::Number(8))];
    let finite = FiniteDomains {
        values: &permitted,
        variables: &[(0, 0..1)],
    };
    let selected = support.select_domains_after(
        rows,
        pattern,
        binding.as_slice().into(),
        finite,
        GroundingWork::new(limits, &mut counters, location()),
    );
    match selected {
        Ok(DomainSelection::Rows(_)) => Selected::Rows,
        Ok(DomainSelection::Declined(_)) => Selected::Declined,
        Ok(DomainSelection::Inapplicable) => Selected::Inapplicable,
        Err(_) => Selected::Refused,
    }
}

fn under(max_support_bytes: usize) -> FormulaLimits {
    FormulaLimits {
        max_support_bytes,
        ..FormulaLimits::default()
    }
}

/// The bytes a selection over `pattern`'s relation holds while it decides:
/// the least room beside the live bytes under which it is not refused. With
/// no more room than that it keeps nothing, so the workspace is unchanged.
fn deciding_bytes(
    support: &Support<'_>,
    pattern: &AtomPattern,
    limits: impl Fn(usize) -> FormulaLimits,
) -> usize {
    let live = support.live_bytes();
    let room = (0..4096)
        .find(|room| selects(support, pattern, &limits(live + room)) != Selected::Refused)
        .expect("a selection decides within four kibibytes");
    assert_eq!(support.live_bytes(), live);
    room
}

#[test]
fn the_growth_of_the_table_list_is_weighed_with_the_table() {
    // Sixteen small tables fill their list, and the seventeenth comes with a
    // list of 32: sixteen entries more to hold, and both buffers live while
    // the tables move. A ceiling with room for the table's preparation and
    // not for the list leaves the selection declined, neither prepared nor
    // refused, and the least ceiling with room for both prepares it.
    let (catalog, patterns) = tiny_relations(17);
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
    for pattern in &patterns[..16] {
        assert_eq!(selects(&support, pattern, &limits), Selected::Rows);
    }
    let workspace = support.tables.get().unwrap();
    assert_eq!(workspace.indices.borrow().capacity(), 16);
    // The first decline enters the relation's count, which the rest find.
    let last = &patterns[16];
    let deciding = deciding_bytes(&support, last, under);
    let live = support.live_bytes() + deciding;
    assert_eq!(
        selects(&support, last, &under(live + size_of::<Declined<'_>>())),
        Selected::Declined
    );
    let live = support.live_bytes() + deciding;
    let rows = support
        .resolve(
            PositivePattern::Flat(last.into()).atom(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap()
        .unwrap();
    let cost = TableCost::of(rows, &[0], &limits, &mut counters, location()).unwrap();
    let growth = Growth::of::<Table<'_, '_>>(16, 16).unwrap();
    assert_eq!(growth.kept(), 16 * size_of::<Table<'_, '_>>());
    let preparation = usize::try_from(cost.preparation_bytes).unwrap();
    // The seventeen tables may take half of the room: twice the preparation,
    // with what the sixteen hold and the list of 32, beside the list's
    // growth. What the sixteen hold is what each retains beside its entry
    // in the list.
    let tables: usize = workspace
        .indices
        .borrow()
        .iter()
        .map(|table| table.statistics().retained_bytes - size_of::<Table<'_, '_>>())
        .sum();
    assert_eq!(workspace.held.get(), tables);
    let held = tables + growth.next;
    let fitting = live + growth.kept() + 2 * preparation + held;
    for ceiling in [
        live + 2 * preparation + held,
        live + growth.next,
        fitting - 1,
    ] {
        assert_eq!(
            selects(&support, last, &under(ceiling)),
            Selected::Declined,
            "{ceiling}"
        );
        assert_eq!(workspace.indices.borrow().len(), 16);
    }
    assert_eq!(selects(&support, last, &under(fitting)), Selected::Rows);
    assert_eq!(workspace.indices.borrow().len(), 17);
    assert_eq!(workspace.indices.borrow().capacity(), 32);
}

#[test]
fn a_count_the_limits_leave_no_room_for_is_not_kept() {
    // With nine support entries to spare no table of ten values fits, so
    // each relation's selection is declined and wants a count. The list of
    // counts doubles when full and is charged at its capacity; where the
    // limits leave no room for it to grow, the selection is inapplicable and
    // the join scans without a count.
    let (catalog, patterns) = tiny_relations(4);
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
    let scanning = |max_support_bytes| FormulaLimits {
        max_support_index_entries: 9,
        ..under(max_support_bytes)
    };
    let count = size_of::<Declined<'_>>();
    let live = support.live_bytes();
    let deciding = deciding_bytes(&support, &patterns[0], scanning);
    assert_eq!(
        selects(
            &support,
            &patterns[0],
            &scanning(live + deciding + count - 1)
        ),
        Selected::Inapplicable
    );
    assert_eq!(support.live_bytes(), live);
    for pattern in &patterns[..3] {
        assert_eq!(
            selects(&support, pattern, &scanning(usize::MAX)),
            Selected::Declined
        );
    }
    let workspace = support.tables.get().unwrap();
    assert_eq!(workspace.declined.borrow().len(), 3);
    assert_eq!(workspace.declined.borrow().capacity(), 4);
    assert_eq!(support.live_bytes() - live, 4 * count);
    // A fourth count has room; a list of five would double to eight, with
    // its four entries still live beside them.
    assert_eq!(
        selects(&support, &patterns[3], &scanning(usize::MAX)),
        Selected::Declined
    );
    assert_eq!(support.live_bytes() - live, 4 * count);
    assert!(workspace.indices.borrow().is_empty());
}
