//! Independent fixed-point and stopped-prefix checks of the shared schedule.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    Work,
    Observation(usize),
}

struct Recording {
    remaining: u64,
    spent: u64,
    stop_observation: Option<usize>,
    activations: Vec<(usize, Option<usize>)>,
}

impl Recording {
    fn new(remaining: u64) -> Self {
        Self {
            remaining,
            spent: 0,
            stop_observation: None,
            activations: Vec::new(),
        }
    }
}

impl Effects for Recording {
    type Error = Fault;

    fn before(&mut self, _point: Point, work: u64) -> Result<(), Self::Error> {
        if work > self.remaining {
            return Err(Fault::Work);
        }
        self.remaining -= work;
        self.spent += work;
        Ok(())
    }

    fn activated(&mut self, vertex: usize, source: Option<usize>) -> Result<(), Self::Error> {
        if self.stop_observation == Some(vertex) {
            return Err(Fault::Observation(vertex));
        }
        self.activations.push((vertex, source));
        Ok(())
    }
}

struct Fixture {
    states: Vec<State>,
    queue: Vec<usize>,
    words: [u64; 1],
}

impl Fixture {
    fn new() -> Self {
        Self {
            states: vec![
                State::Awaiting(1),
                State::Awaiting(1),
                State::Inactive,
                State::Awaiting(2),
                State::Awaiting(1),
            ],
            queue: Vec::with_capacity(5),
            words: [0],
        }
    }

    fn drain(&mut self, effects: &mut Recording) -> Result<(), Fault> {
        // Rules: atom 0 is a fact; atom 1 needs both occurrences of atom 0;
        // atom 0 also follows from atom 1. The last rule cannot requeue atom 0.
        let edges = [
            Edge {
                target: 3,
                weight: 1,
            },
            Edge {
                target: 3,
                weight: 1,
            },
            Edge {
                target: 4,
                weight: 1,
            },
            Edge {
                target: 0,
                weight: 1,
            },
            Edge {
                target: 1,
                weight: 1,
            },
            Edge {
                target: 0,
                weight: 1,
            },
        ];
        drain(
            2,
            &[0, 2, 3, 4, 5, 6],
            &edges,
            &mut self.states,
            &mut self.queue,
            &mut self.words,
            effects,
        )
    }

    fn run(&mut self, effects: &mut Recording) -> Result<(), Fault> {
        activate(2, None, &mut self.states, &mut self.queue, effects)?;
        self.drain(effects)
    }
}

#[test]
fn a_seed_closes_the_cycle_and_preserves_the_first_cause() {
    let mut fixture = Fixture::new();
    let capacity = fixture.queue.capacity();
    let mut effects = Recording::new(u64::MAX);
    assert_eq!(fixture.run(&mut effects), Ok(()));
    assert_eq!(fixture.words, [3]);
    assert_eq!(fixture.queue, [2, 0, 3, 1, 4]);
    assert_eq!(fixture.queue.capacity(), capacity);
    assert!(fixture.states.iter().all(|state| *state == State::True));
    assert_eq!(
        effects.activations,
        [
            (2, None),
            (0, Some(2)),
            (3, Some(0)),
            (1, Some(3)),
            (4, Some(1))
        ]
    );
}

#[test]
fn an_unseeded_cycle_does_not_derive_an_atom() {
    let mut fixture = Fixture::new();
    let original = fixture.states.clone();
    let mut effects = Recording::new(0);
    assert_eq!(fixture.drain(&mut effects), Ok(()));
    assert!(fixture.states == original);
    assert!(fixture.queue.is_empty());
    assert!(effects.activations.is_empty());
    assert_eq!(fixture.words, [0]);
    assert_eq!(effects.spent, 0);
}

#[test]
fn a_refused_activation_chunk_publishes_neither_state_nor_queue() {
    let mut fixture = Fixture::new();
    let mut effects = Recording::new(3);
    assert_eq!(fixture.run(&mut effects), Err(Fault::Work));
    assert_eq!(effects.spent, 1); // State inspection fits; the three writes do not.
    assert!(fixture.states[2] == State::Inactive);
    assert!(fixture.queue.is_empty());
    assert!(effects.activations.is_empty());
}

#[test]
fn every_refused_word_prefix_remains_a_sound_activation_prefix() {
    let mut full = Recording::new(u64::MAX);
    Fixture::new().run(&mut full).unwrap();
    let order = [2, 0, 3, 1, 4];
    for maximum in 0..full.spent {
        let mut fixture = Fixture::new();
        let mut effects = Recording::new(maximum);
        assert_eq!(fixture.run(&mut effects), Err(Fault::Work));
        assert!(effects.spent <= maximum);
        assert_eq!(fixture.queue, order[..fixture.queue.len()]);
        assert_eq!(effects.activations.len(), fixture.queue.len());
        for (vertex, state) in fixture.states.iter().enumerate() {
            assert_eq!(*state == State::True, fixture.queue.contains(&vertex));
        }
        assert!(matches!(fixture.words[0], 0 | 1 | 3));
        for atom in 0..2 {
            if fixture.words[0] & (1 << atom) != 0 {
                assert!(fixture.states[atom] == State::True);
            }
        }
    }
}

#[test]
fn a_failed_observer_stops_before_the_new_atom_is_dequeued() {
    let mut fixture = Fixture::new();
    let mut effects = Recording::new(u64::MAX);
    effects.stop_observation = Some(1);
    assert_eq!(fixture.run(&mut effects), Err(Fault::Observation(1)));
    assert_eq!(fixture.queue, [2, 0, 3, 1]);
    assert_eq!(fixture.words, [1]);
    assert!(fixture.states[1] == State::True);
    assert!(fixture.states[4] == State::Awaiting(1));
    assert_eq!(effects.activations, [(2, None), (0, Some(2)), (3, Some(0))]);
}

#[test]
fn atom_publication_preserves_borrowed_word_boundaries() {
    // Each queued atom costs six units through its word write, then two for
    // the empty outgoing span. A refused write must leave its lane unchanged.
    for maximum in [0, 2, 5, 6, 7, 8, 10, 13, 14, 16, 21, 22, 24] {
        let mut backing = [17_u64, 0, 0, 0, 23];
        let mut states = vec![State::True; 130];
        let mut queue = vec![63, 64, 129];
        let mut effects = Recording::new(maximum);
        let result = drain(
            130,
            &[0; 131],
            &[],
            &mut states,
            &mut queue,
            &mut backing[1..4],
            &mut effects,
        );
        assert_eq!(
            result,
            if maximum == 24 {
                Ok(())
            } else {
                Err(Fault::Work)
            }
        );
        assert_eq!(
            backing,
            [
                17,
                if maximum >= 6 { 1 << 63 } else { 0 },
                u64::from(maximum >= 14),
                if maximum >= 22 { 2 } else { 0 },
                23,
            ],
        );
        assert!(effects.spent <= maximum);
        assert_eq!(queue, [63, 64, 129]);
        assert!(states.iter().all(|state| *state == State::True));
        assert!(effects.activations.is_empty());
    }
}
