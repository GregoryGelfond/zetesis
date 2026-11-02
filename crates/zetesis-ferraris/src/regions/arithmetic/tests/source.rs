mod conditioning;
mod views;

use zetesis_cpu::Cancellation;

use crate::{
    AdmissionError, NodeView, Theory,
    regions::{
        Work,
        arithmetic::{
            budget::{Budget, BudgetError},
            root_facts::{self, CheckedRootFacts},
            source,
            storage::Storage,
        },
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

fn budget(cancellation: &Cancellation) -> Budget<'_, 'static> {
    Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, cancellation)
}

fn checked_facts(owner: &Theory) -> CheckedRootFacts {
    root_facts::build(
        owner,
        &mut Storage::default(),
        &mut budget(&Cancellation::default()),
    )
    .unwrap()
}

#[test]
fn constraints_report_their_forbidden_body_patterns() {
    let owner = clauses(&[0, 1, 2, 3]);
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
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
        let mut meter = budget(&cancellation);
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
fn checker_rereads_patterns_instead_of_trusting_the_proposal() {
    let owner = clauses(&[0, 1]);
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
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
    let mut meter = budget(&cancellation);
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
            &mut budget(&cancellation)
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
            &mut budget(&cancellation)
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn source_refusal_publishes_no_clause_prefix() {
    let owner = clauses(&[2]);
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let expected = source::clause(&owner, 0, &mut Storage::default(), &mut full)
        .unwrap()
        .unwrap();
    for maximum in 0..full.work() {
        let mut limited = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
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
