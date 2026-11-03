//! The compact evidence reproduces the former two-count transitions, including
//! pending operand events, contradictory roots and every short work prefix.

use super::{Closure, Knowledge, Meter, Narrower, NarrowingScratch, Step, Stop, bit};
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

    fn unit<Q: zetesis_algebra::Quota<Error = Stop> + ?Sized>(
        self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        work: &mut Meter<'_, Q>,
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
    fn changed<Q: zetesis_algebra::Quota<Error = Stop> + ?Sized>(
        &mut self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        value: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<Step, Stop> {
        let chain = &index.chains[0];
        if value {
            self.sure += 1;
        } else {
            self.never += 1;
        }
        Ok(match (chain.disjunction, value) {
            (true, true) => closure.sure(chain.root),
            (true, false) if self.never == chain.ordinary_leaves().len() => {
                closure.never(chain.root)
            }
            (true, false)
                if closure.known.sure.contains(chain.root)
                    && self.never + 1 == chain.ordinary_leaves().len() =>
            {
                self.unit(closure, index, work)?
            }
            (false, false) => closure.never(chain.root),
            (false, true) if self.sure == chain.ordinary_leaves().len() => closure.sure(chain.root),
            (false, true)
                if closure.known.never.contains(chain.root)
                    && self.sure + 1 == chain.ordinary_leaves().len() =>
            {
                self.unit(closure, index, work)?
            }
            _ => Step::Unchanged,
        })
    }

    fn teach<Q: zetesis_algebra::Quota<Error = Stop> + ?Sized>(
        self,
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        work: &mut Meter<'_, Q>,
    ) -> Result<Step, Stop> {
        let chain = &index.chains[0];
        if closure.known.sure.contains(chain.root) {
            if chain.disjunction {
                if self.never + 1 == chain.ordinary_leaves().len() {
                    return self.unit(closure, index, work);
                }
            } else if self.sure != chain.ordinary_leaves().len() {
                return Self::force(closure, index, true, work);
            }
        } else if closure.known.never.contains(chain.root) {
            if chain.disjunction {
                if self.never != chain.ordinary_leaves().len() {
                    return Self::force(closure, index, false, work);
                }
            } else if self.sure + 1 == chain.ordinary_leaves().len() {
                return self.unit(closure, index, work);
            }
        }
        Ok(Step::Unchanged)
    }

    fn force<Q: zetesis_algebra::Quota<Error = Stop> + ?Sized>(
        closure: &mut Closure<'_, u16>,
        index: &Narrower,
        value: bool,
        work: &mut Meter<'_, Q>,
    ) -> Result<Step, Stop> {
        let mut step = Step::Unchanged;
        for &operand in index.chains[0].ordinary_leaves() {
            work.tick().map_err(zetesis_algebra::Failure::into_stop)?;
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
    let mut known = compact_mut(&mut knowledge);
    let flag = 1 << index.chains[0].root;
    known.sure[0] &= !flag;
    known.never[0] &= !flag;
    scratch.common.nodes.remove(0);
    if let Some(value) = root {
        if value {
            known.sure[0] |= flag;
        } else {
            known.never[0] |= flag;
        }
        scratch
            .common
            .nodes
            .insert(0, (index.chains[0].root, value));
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
    assert_eq!(actual_scratch.common.nodes, expected_scratch.common.nodes);
    assert_eq!(
        actual_scratch.common.learned,
        expected_scratch.common.learned
    );
    assert_eq!(actual_scratch.common.heads, expected_scratch.common.heads);
    assert_eq!(
        actual_scratch.common.pending,
        expected_scratch.common.pending
    );
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
        assert_eq!(index.chains[0].ordinary_leaves(), [0, 1, 2, 3]);
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
                                .common
                                .nodes
                                .iter()
                                .position(|event| *event == (operand, value == 3))
                                .unwrap();
                            scratch.common.nodes.remove(at);
                        }
                        let (mut actual_work, mut expected_work) =
                            (Meter::local(limit), Meter::local(limit));
                        let mut counts = Counts::new(values);
                        counts.agrees(&actual, disjunction);
                        let actual_result = Closure::<_, zetesis_algebra::AnswerSetsSought> {
                            known: compact_mut(&mut actual),
                            lists: &mut actual_scratch.common,
                            learning: &mut actual_scratch.learning,
                        }
                        .operand_changed(&index, 0, value == 3, &mut actual_work)
                        .map_err(zetesis_algebra::Failure::into_stop);
                        let expected_result = counts.changed(
                            &mut Closure::<_, zetesis_algebra::AnswerSetsSought> {
                                known: compact_mut(&mut expected),
                                lists: &mut expected_scratch.common,
                                learning: &mut expected_scratch.learning,
                            },
                            &index,
                            value == 3,
                            &mut expected_work,
                        );
                        counts.agrees(&actual, disjunction);
                        same_effects(
                            (
                                &actual,
                                &actual_scratch,
                                actual_result,
                                actual_work.charged(),
                            ),
                            (
                                &expected,
                                &expected_scratch,
                                expected_result,
                                expected_work.charged(),
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
                    let (mut actual_work, mut expected_work) =
                        (Meter::local(limit), Meter::local(limit));
                    let counts = Counts::new(values);
                    let actual_result = Closure::<_, zetesis_algebra::AnswerSetsSought> {
                        known: compact_mut(&mut actual),
                        lists: &mut actual_scratch.common,
                        learning: &mut actual_scratch.learning,
                    }
                    .teach_chain(&index, 0, &mut actual_work)
                    .map_err(zetesis_algebra::Failure::into_stop);
                    let expected_result = counts.teach(
                        &mut Closure::<_, zetesis_algebra::AnswerSetsSought> {
                            known: compact_mut(&mut expected),
                            lists: &mut expected_scratch.common,
                            learning: &mut expected_scratch.learning,
                        },
                        &index,
                        &mut expected_work,
                    );
                    counts.agrees(&actual, disjunction);
                    same_effects(
                        (
                            &actual,
                            &actual_scratch,
                            actual_result,
                            actual_work.charged(),
                        ),
                        (
                            &expected,
                            &expected_scratch,
                            expected_result,
                            expected_work.charged(),
                        ),
                    );
                }
            }
        }
    }
}
