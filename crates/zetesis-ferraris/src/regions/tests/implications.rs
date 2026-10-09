//! Implication dispatch preserves ordered consequences and charged visits.

use zetesis_cpu::Stop;

use crate::{AdmissionLimits, FormulaParts, Knowledge, Narrower, NarrowingQuota, Node, Theory};

use super::super::{Closure, NarrowingScratch, Step, Subject, Work};
use super::{compact_mut, counters::same_knowledge};

fn implication(consequent: usize) -> Theory {
    Theory::new(
        2,
        FormulaParts::new(
            vec![Node::atom(0), Node::atom(1), Node::implies(0, consequent)],
            vec![],
        )
        .unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn knowledge(index: &Narrower, values: [Option<bool>; 3]) -> Knowledge {
    let mut knowledge = index.knowledge();
    let known = compact_mut(&mut knowledge);
    for (node, value) in values.into_iter().enumerate() {
        match value {
            Some(true) => known.sure[0] |= 1 << node,
            Some(false) => known.never[0] |= 1 << node,
            None => {}
        }
    }
    knowledge
}

/// A three-valued reference, independent of packed bits and Step joining.
/// Conflicting conclusions keep the earlier truth and record a contradiction.
struct Consequences {
    values: [Option<bool>; 3],
    events: Vec<(usize, bool)>,
    contradiction: bool,
}

impl Consequences {
    fn learn(&mut self, node: usize, value: bool) {
        match self.values[node] {
            None => {
                self.values[node] = Some(value);
                self.events.push((node, value));
            }
            Some(old) => self.contradiction |= old != value,
        }
    }

    fn implication(&mut self, consequent: usize, upward: bool) {
        if upward {
            let truth = match (self.values[0], self.values[consequent]) {
                (Some(false), _) | (_, Some(true)) => Some(true),
                (Some(true), Some(false)) => Some(false),
                _ => None,
            };
            if let Some(value) = truth {
                self.learn(2, value);
            }
        }
        match self.values[2] {
            Some(true) => match (self.values[0], self.values[consequent]) {
                (Some(true), _) => self.learn(consequent, true),
                (_, Some(false)) => self.learn(0, false),
                _ => {}
            },
            Some(false) => {
                self.learn(0, true);
                self.learn(consequent, false);
            }
            None => {}
        }
    }

    fn step(&self) -> Step {
        if self.contradiction {
            Step::Contradiction
        } else if self.events.is_empty() {
            Step::Unchanged
        } else {
            Step::Changed
        }
    }
}

fn compare_transitions(upward: bool) {
    // A repeated operand is one parent incidence. The unused second atom's
    // state must remain untouched when the consequent is also node zero.
    for consequent in [0, 1] {
        let theory = implication(consequent);
        let index = Narrower::new(&theory);
        for first in [None, Some(false), Some(true)] {
            for second in [None, Some(false), Some(true)] {
                for parent in [None, Some(false), Some(true)] {
                    let values = [first, second, parent];
                    let mut expected = Consequences {
                        values,
                        events: vec![],
                        contradiction: false,
                    };
                    expected.implication(consequent, upward);
                    let mut actual = knowledge(&index, values);
                    let mut lists = NarrowingScratch::default();
                    lists.prepare(2);
                    let mut closure = Closure {
                        known: compact_mut(&mut actual),
                        lists: &mut lists,
                    };
                    let result = if upward {
                        closure.revisit_implication(theory.view(), 2)
                    } else {
                        let subject = Subject::from(crate::OriginalSubject::new(&theory, None));
                        // Teaching an implication introduces no extra charged
                        // visit, even when it learns an operand or refutes.
                        let mut work = Work::new(0);
                        let result = closure
                            .teach_operands(subject, &index, 2, &mut work)
                            .unwrap();
                        assert_eq!(work.spent, 0);
                        result
                    };
                    assert!(result == expected.step(), "{values:?}, b={consequent}");
                    same_knowledge(&actual, &knowledge(&index, expected.values));
                    assert_eq!(lists.nodes, expected.events);
                    assert!(lists.learned.is_empty() && lists.heads.is_empty());
                    assert!(lists.pending.iter().all(|word| *word == 0));
                }
            }
        }
    }
}

#[test]
fn implication_events_follow_the_three_valued_rules() {
    compare_transitions(false);
}

#[test]
fn implication_parents_learn_before_teaching() {
    compare_transitions(true);
}

struct Quota {
    remaining: u64,
    calls: u64,
    failure: Stop,
}

impl NarrowingQuota for Quota {
    fn reserve(&mut self, _wanted: u64) -> Result<u64, Stop> {
        self.calls += 1;
        if self.remaining == 0 {
            return Err(self.failure);
        }
        self.remaining -= 1;
        Ok(1)
    }

    fn refund(&mut self, unspent: u64) {
        self.remaining += unspent;
    }
}

#[test]
fn implication_parent_keeps_two_charged_visits() {
    let theory = implication(1);
    let index = Narrower::new(&theory);
    let subject = Subject::from(crate::OriginalSubject::new(&theory, None));
    // Node 0 has learned to hold while its implication parent already holds.
    // The event first tells atom 0, then its charged parent visit forces node 1.
    // The parent's own event remains pending throughout either step.
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for limit in 0..=2 {
            let mut actual = knowledge(&index, [Some(true), None, Some(true)]);
            let mut expected = actual.clone();
            let mut lists = NarrowingScratch::default();
            lists.prepare(2);
            lists.nodes.push((2, true));
            let mut quota = Quota {
                remaining: limit,
                calls: 0,
                failure,
            };
            let mut work = Work::reserved(&mut quota);
            let result = Closure {
                known: compact_mut(&mut actual),
                lists: &mut lists,
            }
            .revisit(subject, &index, 0, true, &mut work);
            assert_eq!(work.spent, limit);
            work.settle();
            if limit < 2 {
                assert!(result == Err(failure));
            } else {
                assert!(matches!(result, Ok(Step::Changed)));
                compact_mut(&mut expected).sure[0] |= 1 << 1;
            }
            if limit > 0 {
                compact_mut(&mut expected).atom_sure[0] |= 1;
            }
            same_knowledge(&actual, &expected);
            assert_eq!(lists.learned, if limit == 0 { vec![] } else { vec![0] });
            let mut events = vec![(2, true)];
            if limit == 2 {
                events.push((1, true));
            }
            assert_eq!(lists.nodes, events);
            assert!(lists.heads.is_empty());
            assert!(lists.pending.iter().all(|word| *word == 0));
            assert_eq!(quota.calls, (limit + 1).min(2));
        }
    }
}
