//! The compact evidence reproduces the former two-count transitions, including
//! pending operand events, contradictory roots and every short work prefix.

use super::{Closure, Knowledge, Narrower, NarrowingScratch, Step, Stop, Work, bit};
use crate::regions::tests::{compact, compact_mut};

/// The previous representation, computed independently from processed events.
#[derive(Clone, Copy)]
struct Counts {
    sure: usize,
    never: usize,
}

impl Counts {
    fn new(values: [usize; 4]) -> Self {
        Self {
            sure: values.into_iter().filter(|&value| value == 4).count(),
            never: values.into_iter().filter(|&value| value == 2).count(),
        }
    }

    fn unit(
        self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let witnessed = if index.chains[0].disjunction {
            self.sure != 0
        } else {
            self.never != 0
        };
        if witnessed {
            Ok(Step::Unchanged)
        } else {
            super::scan(closure, index, work)
        }
    }

    /// The former event transition reads two exact counts. It never reads the
    /// replacement count or witness and uses the former ordered unit scan.
    fn changed(
        &mut self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let chain = &index.chains[0];
        if value {
            self.sure += 1;
        } else {
            self.never += 1;
        }
        Ok(match (chain.disjunction, value) {
            (true, true) => closure.sure(chain.root),
            (true, false) if self.never == chain.operands.len() => closure.never(chain.root),
            (true, false)
                if bit(closure.known.sure, chain.root)
                    && self.never + 1 == chain.operands.len() =>
            {
                self.unit(closure, index, work)?
            }
            (false, false) => closure.never(chain.root),
            (false, true) if self.sure == chain.operands.len() => closure.sure(chain.root),
            (false, true)
                if bit(closure.known.never, chain.root)
                    && self.sure + 1 == chain.operands.len() =>
            {
                self.unit(closure, index, work)?
            }
            _ => Step::Unchanged,
        })
    }

    fn teach(
        self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let chain = &index.chains[0];
        if bit(closure.known.sure, chain.root) {
            if chain.disjunction {
                if self.never + 1 == chain.operands.len() {
                    return self.unit(closure, index, work);
                }
            } else if self.sure != chain.operands.len() {
                return Self::force(closure, index, true, work);
            }
        } else if bit(closure.known.never, chain.root) {
            if chain.disjunction {
                if self.never != chain.operands.len() {
                    return Self::force(closure, index, false, work);
                }
            } else if self.sure + 1 == chain.operands.len() {
                return self.unit(closure, index, work);
            }
        }
        Ok(Step::Unchanged)
    }

    fn force(
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        value: bool,
        work: &mut Work<'_>,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for &operand in &index.chains[0].operands {
            work.tick()?;
            step = step.join(if value {
                closure.sure(operand)
            } else {
                closure.never(operand)
            });
            if step == Step::Contradiction {
                return Ok(step);
            }
        }
        Ok(step)
    }

    fn agrees(self, knowledge: &Knowledge, disjunction: bool) {
        let known = compact(knowledge);
        let (neutral, absorbing) = if disjunction {
            (self.never, self.sure)
        } else {
            (self.sure, self.never)
        };
        assert_eq!(known.neutral_operands.get(0), neutral);
        assert_eq!(bit(known.masks.slices()[5], 0), absorbing != 0);
    }
}

fn state(
    index: &Narrower,
    values: [usize; 4],
    root: Option<bool>,
) -> (Knowledge, NarrowingScratch) {
    let (mut knowledge, mut scratch) = super::state(index, values);
    let known = compact_mut(&mut knowledge);
    let flag = 1 << index.chains[0].root;
    known.sure[0] &= !flag;
    known.never[0] &= !flag;
    scratch.nodes.remove(0);
    if let Some(value) = root {
        if value {
            known.sure[0] |= flag;
        } else {
            known.never[0] |= flag;
        }
        scratch.nodes.insert(0, (index.chains[0].root, value));
    }
    (knowledge, scratch)
}

fn same_effects(
    actual: (&Knowledge, &NarrowingScratch, Result<Step, Stop>, u64),
    expected: (&Knowledge, &NarrowingScratch, Result<Step, Stop>, u64),
) {
    let (actual, actual_scratch, actual_result, actual_work) = actual;
    let (expected, expected_scratch, expected_result, expected_work) = expected;
    assert!(actual_result == expected_result);
    assert_eq!(actual_work, expected_work);
    let (actual, expected) = (compact(actual), compact(expected));
    // The reference deliberately does not update the replacement evidence.
    assert_eq!(actual.masks.slices()[..5], expected.masks.slices()[..5]);
    assert_eq!(actual.seeded, expected.seeded);
    assert_eq!(actual.unknown.len(), expected.unknown.len());
    for atom in 0..actual.unknown.len() {
        assert_eq!(actual.unknown.get(atom), expected.unknown.get(atom));
    }
    assert_eq!(actual_scratch.nodes, expected_scratch.nodes);
    assert_eq!(actual_scratch.learned, expected_scratch.learned);
    assert_eq!(actual_scratch.heads, expected_scratch.heads);
    assert_eq!(actual_scratch.pending, expected_scratch.pending);
}

fn values(mut code: usize) -> [usize; 4] {
    std::array::from_fn(|_| {
        let value = code % 5;
        code /= 5;
        value
    })
}

#[test]
fn processed_events_preserve_two_count_transitions() {
    for disjunction in [false, true] {
        let theory = super::chain(disjunction);
        let index = Narrower::new(&theory);
        // The repeated node 1 is coalesced, but distinct nodes 1 and 2 both
        // denote atom 1 and contribute separate processed operand events.
        assert_eq!(index.chains.len(), 1);
        assert_eq!(index.chains[0].operands, [0, 1, 2, 3]);
        for values in (0..5usize.pow(4)).map(values) {
            for root in [None, Some(false), Some(true)] {
                for (operand, value) in values
                    .into_iter()
                    .enumerate()
                    .filter(|(_, value)| [1, 3].contains(value))
                {
                    for limit in [0, 1, 3, 4, u64::MAX] {
                        let (mut actual, mut actual_scratch) = state(&index, values, root);
                        let (mut expected, mut expected_scratch) = state(&index, values, root);
                        // The caller removes the current event before dispatch.
                        for scratch in [&mut actual_scratch, &mut expected_scratch] {
                            let at = scratch
                                .nodes
                                .iter()
                                .position(|event| *event == (operand, value == 3))
                                .unwrap();
                            scratch.nodes.remove(at);
                        }
                        let (mut actual_work, mut expected_work) =
                            (Work::new(limit), Work::new(limit));
                        let mut counts = Counts::new(values);
                        counts.agrees(&actual, disjunction);
                        let actual_result = Closure {
                            known: compact_mut(&mut actual),
                            lists: &mut actual_scratch,
                        }
                        .operand_changed(
                            &index,
                            0,
                            value == 3,
                            &mut actual_work,
                        );
                        let expected_result = counts.changed(
                            &mut Closure {
                                known: compact_mut(&mut expected),
                                lists: &mut expected_scratch,
                            },
                            &index,
                            value == 3,
                            &mut expected_work,
                        );
                        counts.agrees(&actual, disjunction);
                        same_effects(
                            (&actual, &actual_scratch, actual_result, actual_work.spent),
                            (
                                &expected,
                                &expected_scratch,
                                expected_result,
                                expected_work.spent,
                            ),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn downward_teaching_preserves_two_count_transitions() {
    for disjunction in [false, true] {
        let theory = super::chain(disjunction);
        let index = Narrower::new(&theory);
        for values in (0..5usize.pow(4)).map(values) {
            for root in [None, Some(false), Some(true)] {
                for limit in [0, 1, 3, 4, u64::MAX] {
                    let (mut actual, mut actual_scratch) = state(&index, values, root);
                    let (mut expected, mut expected_scratch) = state(&index, values, root);
                    let (mut actual_work, mut expected_work) = (Work::new(limit), Work::new(limit));
                    let counts = Counts::new(values);
                    let actual_result = Closure {
                        known: compact_mut(&mut actual),
                        lists: &mut actual_scratch,
                    }
                    .teach_chain(&index, 0, &mut actual_work);
                    let expected_result = counts.teach(
                        &mut Closure {
                            known: compact_mut(&mut expected),
                            lists: &mut expected_scratch,
                        },
                        &index,
                        &mut expected_work,
                    );
                    counts.agrees(&actual, disjunction);
                    same_effects(
                        (&actual, &actual_scratch, actual_result, actual_work.spent),
                        (
                            &expected,
                            &expected_scratch,
                            expected_result,
                            expected_work.spent,
                        ),
                    );
                }
            }
        }
    }
}
