//! Ordered cut implications retain the full scan's effects, including masks
//! and contradictory targets, while skipping the unrelated end of a column.

use crate::regions::{ChainOperands, Closure, Known, Narrower, NarrowingScratch, Step, Work, bit};
use crate::test_support::SinglePermits;
use crate::{ExtremumCutRule, Theory};
use zetesis_cpu::Stop;

fn family(count: usize, lengths: &[usize], suffix: bool) -> (Theory, Vec<usize>) {
    let mut owner = super::owner(count);
    let conditions: Vec<_> = (0..count).collect();
    let ranks: Vec<_> = (1..=count as u64).collect();
    let guards: Vec<_> = lengths
        .iter()
        .map(|&length| {
            if suffix {
                ExtremumCutRule::Suffix((count + 1 - length) as u64)
            } else {
                ExtremumCutRule::Prefix((length + 1) as u64)
            }
        })
        .collect();
    let cuts = super::cuts(&mut owner, &conditions, &ranks, count as u64 + 2, &guards);
    (super::finish(owner, count, vec![]), cuts)
}

#[derive(Clone, Copy)]
struct Event {
    source: usize,
    value: bool,
    opposing: Option<usize>,
}

struct Attempt {
    result: Result<Step, Stop>,
    known: Known<u16>,
    events: Vec<(usize, bool)>,
}

/// The former complete column scan is an independent effects oracle: it never
/// calls the new range selection and retains both original frozen-mask gates.
fn scan(
    closure: &mut Closure<'_, u16>,
    index: &Narrower,
    chain: usize,
    frozen: Option<&[bool]>,
    work: &mut Work<'_>,
) -> Result<Step, Stop> {
    let source = &index.chains[chain];
    let ChainOperands::Ordinal { column, length } = source.operands else {
        unreachable!();
    };
    if frozen.is_some_and(|truth| !truth[source.root]) {
        return Ok(Step::Unchanged);
    }
    let value = bit(closure.known.sure, source.root);
    let mut step = Step::Unchanged;
    for &(size, node) in &index.extrema.columns[column].guards {
        work.tick()?;
        if (value && size < length)
            || (!value && size > length)
            || frozen.is_some_and(|truth| !truth[node])
        {
            continue;
        }
        step = step.join(if value {
            closure.sure(node)
        } else {
            closure.never(node)
        });
        if step == Step::Contradiction {
            break;
        }
    }
    Ok(step)
}

fn attempt(
    index: &Narrower,
    event: Event,
    frozen: Option<&[bool]>,
    scanning: bool,
    work: &mut Work<'_>,
) -> Attempt {
    let mut known = index.root_known::<u16>();
    let mut lists = NarrowingScratch::default();
    let mut closure = Closure {
        known: known.borrow(),
        lists: &mut lists,
    };
    if let Some(node) = event.opposing {
        let result = if event.value {
            closure.never(node)
        } else {
            closure.sure(node)
        };
        assert!(result == Step::Changed);
    }
    let seeded = if event.value {
        closure.sure(event.source)
    } else {
        closure.never(event.source)
    };
    assert!(seeded == Step::Changed);
    closure.lists.nodes.clear();
    let chain = crate::regions::chain_position(index.chain_of[event.source].unwrap());
    let result = if scanning {
        scan(&mut closure, index, chain, frozen, work)
    } else {
        closure.extremum_order(index, chain, frozen, work)
    };
    Attempt {
        result,
        known,
        events: lists.nodes,
    }
}

fn same_effects(left: &Attempt, right: &Attempt) {
    assert!(left.result == right.result);
    assert_eq!(left.known.masks.slices(), right.known.masks.slices());
    assert_eq!(left.events, right.events);
}

#[test]
fn ordered_ranges_preserve_equal_cuts_and_contradictions() {
    for suffix in [false, true] {
        let (theory, cuts) = family(4, &[1, 1, 2, 3, 3, 4], suffix);
        let index = Narrower::new(&theory);
        for &source in &cuts {
            for value in [false, true] {
                for opposing in std::iter::once(None).chain(
                    cuts.iter()
                        .copied()
                        .filter(|&node| node != source)
                        .map(Some),
                ) {
                    let event = Event {
                        source,
                        value,
                        opposing,
                    };
                    let selected = attempt(&index, event, None, false, &mut Work::new(u64::MAX));
                    let scanned = attempt(&index, event, None, true, &mut Work::new(u64::MAX));
                    same_effects(&selected, &scanned);
                }
            }
        }
    }
}

#[test]
fn ordered_ranges_preserve_both_frozen_gates() {
    let (theory, cuts) = family(4, &[1, 1, 2, 3, 3, 4], false);
    let index = Narrower::new(&theory);
    // Exercise each mask gate independently, including masks that would not
    // arise together in an evaluated interpretation. The enclosing module's
    // model test separately checks all valid original/frozen interpretations.
    for mask in 0..1usize << cuts.len() {
        let mut frozen = vec![true; index.chain_of.len()];
        for (position, &node) in cuts.iter().enumerate() {
            frozen[node] = mask & (1 << position) != 0;
        }
        for &source in &cuts {
            for value in [false, true] {
                let event = Event {
                    source,
                    value,
                    opposing: None,
                };
                let mut work = Work::new(u64::MAX);
                let selected = attempt(&index, event, Some(&frozen), false, &mut work);
                let scanned = attempt(&index, event, Some(&frozen), true, &mut Work::new(u64::MAX));
                same_effects(&selected, &scanned);
                if !frozen[source] {
                    assert_eq!(work.spent, 0);
                }
            }
        }
    }
}

#[test]
fn sparse_ordered_implications_skip_unrelated_guards() {
    let count = 128usize;
    let lengths: Vec<_> = (1..=count).collect();
    let (theory, cuts) = family(count, &lengths, false);
    let index = Narrower::new(&theory);
    // Each source has only one other consequence. A full-column scan cannot
    // finish within logarithmic boundary work plus those two target visits.
    let allowance = u64::from(count.ilog2()) + 4;
    for (value, source, target) in [(false, cuts[1], cuts[0]), (true, cuts[126], cuts[127])] {
        let event = Event {
            source,
            value,
            opposing: None,
        };
        let selected = attempt(&index, event, None, false, &mut Work::new(allowance));
        assert!(matches!(selected.result, Ok(Step::Changed)));
        assert_eq!(selected.events, [(target, value)]);
        let scanned = attempt(&index, event, None, true, &mut Work::new(allowance));
        assert!(scanned.result == Err(Stop::WorkLimit));
    }
}

#[test]
fn interrupted_ordering_retains_only_a_charged_effect_prefix() {
    let (theory, cuts) = family(4, &[1, 1, 2, 3, 3, 4], false);
    let index = Narrower::new(&theory);
    for source in cuts {
        for value in [false, true] {
            let event = Event {
                source,
                value,
                opposing: None,
            };
            let mut complete_work = Work::new(u64::MAX);
            let complete = attempt(&index, event, None, false, &mut complete_work);
            let chain = crate::regions::chain_position(index.chain_of[source].unwrap());
            let ChainOperands::Ordinal { column, length } = index.chains[chain].operands else {
                unreachable!();
            };
            let targets = index.extrema.columns[column]
                .guards
                .iter()
                .filter(|&&(size, _)| {
                    if value {
                        size >= length
                    } else {
                        size <= length
                    }
                })
                .count();
            // Even a source that teaches nothing must pay for every admitted
            // target visit. Full-range endpoints detect deletion of those
            // charges independently of the measured interruption bound.
            assert!(complete_work.spent >= targets as u64);
            for stop in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
                for limit in 0..=complete_work.spent {
                    let mut quota = SinglePermits::new(limit, stop);
                    let mut work = Work::reserved(&mut quota);
                    let partial = attempt(&index, event, None, false, &mut work);
                    assert_eq!(work.spent, limit);
                    if limit == complete_work.spent {
                        same_effects(&partial, &complete);
                    } else {
                        assert!(partial.result == Err(stop));
                        assert!(complete.events.starts_with(&partial.events));
                        let mut expected = index.root_known::<u16>();
                        let mut scratch = NarrowingScratch::default();
                        let mut closure = Closure {
                            known: expected.borrow(),
                            lists: &mut scratch,
                        };
                        for (node, truth) in
                            std::iter::once((source, value)).chain(partial.events.iter().copied())
                        {
                            assert!(
                                (if truth {
                                    closure.sure(node)
                                } else {
                                    closure.never(node)
                                }) == Step::Changed
                            );
                        }
                        assert_eq!(partial.known.masks.slices(), expected.masks.slices());
                    }
                    work.refund();
                    assert_eq!(quota.remaining, 0);
                }
            }
        }
    }
}
