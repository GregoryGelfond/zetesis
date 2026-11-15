//! A pair proposal narrows the existing raw grammar, not its source authority.

use super::*;

#[derive(Debug)]
struct Read {
    result: Result<Option<source::Clause>, BudgetError>,
    work: u64,
    peak: u128,
}

fn read(owner: &Theory, limited: bool, maximum: u64, bytes: usize) -> Read {
    let cancellation = Cancellation::default();
    let mut charged = Meter::local(u64::MAX);
    let (result, work, peak) = {
        let mut budget = Budget::new(&mut charged, maximum, bytes, &cancellation);
        let mut storage = Storage::default();
        let result =
            storage.with_record(source::CLAUSE_SCRATCH_BYTES as u128, &mut budget, |s, b| {
                let clause = if limited {
                    source::positive_pair_clause(owner, 0, s, b)?
                } else {
                    source::clause(owner, 0, s, b)?
                };
                let Some(clause) = clause else {
                    return Ok(None);
                };
                b.charge(3)?;
                Ok((clause.support.len == 2 && clause.forbidden == 3).then_some(clause))
            });
        let work = budget.work();
        let peak = budget.peak_bytes();
        budget.finish();
        (result, work, peak)
    };
    assert_eq!(charged.charged(), work);
    Read { result, work, peak }
}

fn signed(terms: &[(usize, bool)], nested: bool, wrappers: bool) -> Theory {
    constructed_theory(10, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let truth = push(nodes, NodeView::Implies(falsum, falsum));
        let mut leaves: Vec<_> = terms
            .iter()
            .map(|&(atom, positive)| {
                let leaf = if positive {
                    atom
                } else {
                    push(nodes, NodeView::Implies(atom, falsum))
                };
                if wrappers {
                    push(nodes, NodeView::Implies(truth, leaf))
                } else {
                    leaf
                }
            })
            .collect();
        if nested && leaves.len() >= 2 {
            let prefix = push(nodes, NodeView::And(&leaves[..2]));
            leaves.splice(..2, [prefix]);
        }
        let body = if leaves.is_empty() {
            truth
        } else {
            push(nodes, NodeView::And(&leaves))
        };
        vec![push(nodes, NodeView::Implies(body, falsum))]
    })
}

#[test]
fn positive_pairs_match_the_full_signed_clause_filter() {
    for length in 0..=4_u32 {
        for mut code in 0..6_usize.pow(length) {
            let mut terms = Vec::new();
            for _ in 0..length {
                terms.push((code % 3, code % 6 < 3));
                code /= 6;
            }
            for nested in [false, true] {
                for wrappers in [false, true] {
                    let owner = signed(&terms, nested, wrappers);
                    assert_eq!(
                        read(&owner, true, u64::MAX, usize::MAX).result,
                        read(&owner, false, u64::MAX, usize::MAX).result,
                        "terms={terms:?}, nested={nested}, wrappers={wrappers}"
                    );
                }
            }
        }
    }
    for length in [8, 9, 10] {
        let terms: Vec<_> = (0..length).map(|atom| (atom, true)).collect();
        let owner = signed(&terms, true, true);
        assert_eq!(read(&owner, true, u64::MAX, usize::MAX).result, Ok(None));
        assert_eq!(read(&owner, false, u64::MAX, usize::MAX).result, Ok(None));
    }
}

#[test]
fn pair_recognition_stops_before_an_unneeded_source_tail() {
    for negative_first in [false, true] {
        let terms: Vec<_> = (0..8)
            .map(|atom| (atom, atom != 0 || !negative_first))
            .collect();
        let short = signed(&terms[..3], false, false);
        let long = signed(&terms, false, false);
        let prefix = read(&short, true, u64::MAX, usize::MAX);
        let whole = read(&long, true, u64::MAX, usize::MAX);
        assert_eq!(prefix.result, Ok(None));
        assert_eq!(whole.result, prefix.result);
        assert_eq!(whole.work, prefix.work, "the unused tail must not be read");
        assert!(read(&long, false, u64::MAX, usize::MAX).work > whole.work);
    }
}

#[test]
fn pair_proposals_keep_exact_refusal_receipts() {
    let owner = signed(&[(1, true), (0, true)], true, true);
    let full = read(&owner, true, u64::MAX, usize::MAX);
    let expected = full.result.unwrap().unwrap();
    let mut successful = false;
    // Canonical node views reserve up to 24 units before settling actual work.
    for maximum in 0..=full.work + 24 {
        let attempt = read(&owner, true, maximum, usize::MAX);
        match attempt.result {
            Ok(value) => {
                assert_eq!(value, Some(expected));
                assert_eq!(attempt.work, full.work);
                successful = true;
            }
            Err(error) => {
                assert_eq!(error, BudgetError::WorkDeclined);
                assert!(attempt.work <= maximum);
            }
        }
    }
    assert!(successful);
    let peak = usize::try_from(full.peak).unwrap();
    assert_eq!(
        read(&owner, true, u64::MAX, peak).result,
        Ok(Some(expected))
    );
    let short = read(&owner, true, u64::MAX, peak - 1);
    assert_eq!(
        short.result,
        Err(BudgetError::StorageDeclined {
            required: full.peak,
            limit: peak - 1
        })
    );
}

#[test]
fn cancelled_pair_admission_keeps_the_stop() {
    let owner = signed(&[(0, true), (1, true)], false, false);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut charged = Meter::local(u64::MAX);
    let mut budget = Budget::new(&mut charged, 0, usize::MAX, &cancellation);
    assert_eq!(
        source::positive_pair_clause(&owner, 0, &mut Storage::default(), &mut budget),
        Err(BudgetError::Stopped(zetesis_algebra::Stop::Cancelled))
    );
    assert_eq!(budget.work(), 0);
}
