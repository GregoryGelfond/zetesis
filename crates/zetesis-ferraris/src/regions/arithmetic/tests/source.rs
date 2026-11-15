use zetesis_algebra::Meter;

mod conditioning;
mod positive_pairs;
mod views;

use zetesis_algebra::Cancellation;

use crate::{
    AdmissionError, NodeView, Theory,
    regions::arithmetic::{
        budget::{Budget, BudgetError},
        root_facts::{self, CheckedRootFacts},
        source,
        storage::Storage,
    },
    test_support::{constructed_theory, push},
};

fn clauses(patterns: &[usize]) -> Theory {
    constructed_theory(2, |nodes| {
        let falsehood = push(nodes, NodeView::False);
        let negative = [
            push(nodes, NodeView::Implies(0, falsehood)),
            push(nodes, NodeView::Implies(1, falsehood)),
        ];
        patterns
            .iter()
            .map(|&pattern| {
                let terms: Vec<_> = (0..2)
                    .map(|atom| {
                        if pattern & (1 << atom) == 0 {
                            negative[atom]
                        } else {
                            atom
                        }
                    })
                    .collect();
                let body = push(nodes, NodeView::And(&terms));
                push(nodes, NodeView::Implies(body, falsehood))
            })
            .collect()
    })
}

fn budget<'a>(work: &'a mut Meter<'static>, cancellation: &'a Cancellation) -> Budget<'a, 'static> {
    Budget::new(work, u64::MAX, usize::MAX, cancellation)
}

fn checked_facts(owner: &Theory) -> CheckedRootFacts {
    root_facts::build(
        owner,
        &mut Storage::default(),
        &mut budget(&mut Meter::local(u64::MAX), &Cancellation::default()),
    )
    .unwrap()
}

#[test]
fn constraints_report_their_forbidden_body_patterns() {
    let owner = clauses(&[0, 1, 2, 3]);
    let cancellation = Cancellation::default();
    let mut meter_work = Meter::local(u64::MAX);
    let mut meter = budget(&mut meter_work, &cancellation);
    for ordinal in 0..4 {
        let record = source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
            .unwrap()
            .unwrap();
        assert_eq!(record.root, ordinal);
        assert_eq!(usize::from(record.forbidden), ordinal);
        assert_eq!(record.support.len, 2);
        assert_eq!(&record.support.atoms[..2], &[0, 1]);
        assert_eq!(&record.support.atoms[2..], &[0; 6]);
    }
}

#[test]
fn source_checks_agree_with_all_small_allowed_pattern_sets() {
    let cancellation = Cancellation::default();
    for forbidden in 1_usize..16 {
        let patterns: Vec<_> = (0..4).filter(|p| forbidden & (1 << p) != 0).collect();
        let owner = clauses(&patterns);
        let mut meter_work = Meter::local(u64::MAX);
        let mut meter = budget(&mut meter_work, &cancellation);
        let records: Vec<_> = (0..patterns.len())
            .map(|ordinal| {
                source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
                    .unwrap()
                    .unwrap()
            })
            .collect();
        for coefficients in 0_u64..4 {
            for constant in [false, true] {
                let expected = (0_u64..4)
                    .filter(|p| forbidden & (1 << p) == 0)
                    .all(|p| ((p & coefficients).count_ones() % 2 != 0) == constant);
                let checked = source::check_clause_equation(
                    &owner,
                    &checked_facts(&owner),
                    &records,
                    records[0].support,
                    (&[coefficients, 0], constant),
                    &mut Storage::default(),
                    &mut meter,
                )
                .unwrap();
                assert_eq!(
                    checked, expected,
                    "forbidden={forbidden}, row={coefficients}, c={constant}"
                );
            }
        }
    }
}

#[test]
fn unit_checks_agree_with_the_row_checks_at_every_one_atom_equation() {
    // A unit is accepted exactly when the row over its one atom is.
    let cancellation = Cancellation::default();
    for forbidden in 1_usize..16 {
        let patterns: Vec<_> = (0..4).filter(|p| forbidden & (1 << p) != 0).collect();
        let owner = clauses(&patterns);
        let mut meter_work = Meter::local(u64::MAX);
        let mut meter = budget(&mut meter_work, &cancellation);
        let records: Vec<_> = (0..patterns.len())
            .map(|ordinal| {
                source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
                    .unwrap()
                    .unwrap()
            })
            .collect();
        let facts = checked_facts(&owner);
        for atom in 0..2 {
            for value in [false, true] {
                let row = source::check_clause_equation(
                    &owner,
                    &facts,
                    &records,
                    records[0].support,
                    (&[1 << atom, 0], value),
                    &mut Storage::default(),
                    &mut meter,
                )
                .unwrap();
                let unit = source::check_clause_unit(
                    &owner,
                    &facts,
                    &records,
                    records[0].support,
                    (atom, value),
                    &mut Storage::default(),
                    &mut meter,
                )
                .unwrap();
                assert_eq!(unit, row, "forbidden={forbidden}, atom={atom}, v={value}");
            }
        }
    }
}

#[test]
fn a_unit_check_refuses_a_foreign_owner_and_an_atom_its_support_lacks() {
    // Forbidding both patterns in which `a1` holds leaves `a1 = 0`.
    let owner = clauses(&[2, 3]);
    let foreign = clauses(&[2, 3]);
    let cancellation = Cancellation::default();
    let mut meter_work = Meter::local(u64::MAX);
    let mut meter = budget(&mut meter_work, &cancellation);
    let records: Vec<_> = (0..2)
        .map(|ordinal| {
            source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
                .unwrap()
                .unwrap()
        })
        .collect();
    let mut check = |facts: &CheckedRootFacts, atom: usize| {
        source::check_clause_unit(
            &owner,
            facts,
            &records,
            records[0].support,
            (atom, false),
            &mut Storage::default(),
            &mut meter,
        )
        .unwrap()
    };
    assert!(check(&checked_facts(&owner), 1));
    assert!(!check(&checked_facts(&owner), 0), "`a0` is left open");
    assert!(!check(&checked_facts(&foreign), 1));
    assert!(
        !check(&checked_facts(&owner), 2),
        "no coordinate names atom 2"
    );
}

#[test]
fn a_unit_has_the_coefficient_of_its_atoms_coordinate_alone() {
    let cancellation = Cancellation::default();
    let mut meter_work = Meter::local(u64::MAX);
    let mut meter = budget(&mut meter_work, &cancellation);
    let support = source::Support {
        atoms: [7, 9, 12, 0, 0, 0, 0, 0],
        len: 3,
    };
    let mut coefficients =
        |atom: usize| source::unit_coefficients(support, atom, &mut meter).unwrap();
    assert_eq!(coefficients(7), Some(0b001));
    assert_eq!(coefficients(9), Some(0b010));
    assert_eq!(coefficients(12), Some(0b100));
    // Atom 0 fills the unused coordinates and is no atom of the support.
    assert_eq!(coefficients(0), None);
    assert_eq!(coefficients(8), None);
}

#[test]
fn checker_rereads_patterns_instead_of_trusting_the_proposal() {
    let owner = clauses(&[0, 1]);
    let cancellation = Cancellation::default();
    let mut meter_work = Meter::local(u64::MAX);
    let mut meter = budget(&mut meter_work, &cancellation);
    let mut records: Vec<_> = (0..2)
        .map(|ordinal| {
            source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
                .unwrap()
                .unwrap()
        })
        .collect();
    records[0].forbidden = 2;
    records[1].forbidden = 3;
    assert!(
        !source::check_clause_equation(
            &owner,
            &checked_facts(&owner),
            &records,
            records[0].support,
            (&[2, 0], false),
            &mut Storage::default(),
            &mut meter,
        )
        .unwrap()
    );
}

#[test]
fn checker_refuses_foreign_owners_and_padding_coefficients() {
    let owner = clauses(&[0, 3]);
    let foreign = clauses(&[0, 3]);
    let cancellation = Cancellation::default();
    let mut meter_work = Meter::local(u64::MAX);
    let mut meter = budget(&mut meter_work, &cancellation);
    let records: Vec<_> = (0..2)
        .map(|ordinal| {
            source::clause(&owner, ordinal, &mut Storage::default(), &mut meter)
                .unwrap()
                .unwrap()
        })
        .collect();
    assert!(
        !source::check_clause_equation(
            &owner,
            &checked_facts(&foreign),
            &records,
            records[0].support,
            (&[3, 0], true),
            &mut Storage::default(),
            &mut meter,
        )
        .unwrap()
    );
    assert!(
        !source::check_clause_equation(
            &owner,
            &checked_facts(&owner),
            &records,
            records[0].support,
            (&[3, 1], true),
            &mut Storage::default(),
            &mut meter,
        )
        .unwrap()
    );
}

#[test]
fn an_absent_root_is_a_typed_source_failure() {
    let owner = clauses(&[0]);
    let cancellation = Cancellation::default();
    assert!(matches!(
        source::clause(
            &owner,
            1,
            &mut Storage::default(),
            &mut budget(&mut Meter::local(u64::MAX), &cancellation)
        ),
        Err(BudgetError::Source(AdmissionError::Root))
    ));
}

#[test]
fn duplicate_atoms_decline_source_a() {
    let owner = constructed_theory(1, |nodes| {
        let falsehood = push(nodes, NodeView::False);
        let duplicate = push(nodes, NodeView::And(&[0, 0]));
        vec![push(nodes, NodeView::Implies(duplicate, falsehood))]
    });
    let cancellation = Cancellation::default();
    assert!(
        source::clause(
            &owner,
            0,
            &mut Storage::default(),
            &mut budget(&mut Meter::local(u64::MAX), &cancellation)
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn source_refusal_publishes_no_clause_prefix() {
    let owner = clauses(&[2]);
    let cancellation = Cancellation::default();
    let mut full_work = Meter::local(u64::MAX);
    let mut full = budget(&mut full_work, &cancellation);
    let expected = source::clause(&owner, 0, &mut Storage::default(), &mut full)
        .unwrap()
        .unwrap();
    for maximum in 0..full.work() {
        let mut limited_work = Meter::local(u64::MAX);
        let mut limited = Budget::new(&mut limited_work, maximum, usize::MAX, &cancellation);
        assert!(matches!(
            source::clause(&owner, 0, &mut Storage::default(), &mut limited),
            Err(BudgetError::WorkDeclined)
        ));
        assert!(limited.work() <= maximum);
    }
    // Bounded source views reserve their maximum, so exact successful work need
    // not be the smallest sufficient preparation allowance.
    assert_eq!(
        source::clause(&owner, 0, &mut Storage::default(), &mut full).unwrap(),
        Some(expected)
    );
}
