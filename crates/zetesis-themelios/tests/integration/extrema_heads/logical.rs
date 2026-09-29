//! Independent finite source semantics with a declared ordered term carrier.
//! No expected bound or permission is computed from compiler IR or DAG nodes.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::{canonical, holds, input, values};

// This order is deliberately explicit, including the empty extrema. The test
// oracle compares ordinal positions, never Value::Ord or Value::compare_terms.
const TERMS: [&str; 9] = [
    "#inf", "-2", "0", "word", "\"word\"", "f(1)", "f(2)", "f(1,2)", "#sup",
];

#[derive(Clone, Copy)]
struct World([bool; 4]);

impl World {
    fn operand(self, index: u8) -> bool {
        let value = match index % 4 {
            0 | 1 => self.0[usize::from(index % 4)],
            2 => true,
            3 => false,
            _ => unreachable!("four operand kinds"),
        };
        signed(index / 4, value, None)
    }
}

fn signed(sign: u8, candidate: bool, inner: Option<bool>) -> bool {
    match sign {
        0 => candidate && inner.unwrap_or(true),
        1 => !candidate,
        2 => candidate,
        _ => unreachable!("three signs"),
    }
}

fn eligible(mode: u8, candidate: World, inner: Option<World>) -> bool {
    if mode == 0 {
        return true;
    }
    let index = [2, 0, 1][usize::from((mode - 1) / 3)];
    signed(
        (mode - 1) % 3,
        candidate.0[index],
        inner.map(|world| world.0[index]),
    )
}

/// Each row declares (optional term ordinal, suffix, signed operand, condition).
/// An absent ordinal denotes the one empty tuple: its suffix has no meaning.
/// The fixed seed rules provide possible support without unconditional choices.
pub(super) struct Selection {
    pub rows: Vec<(Option<u8>, u8, u8, u8)>,
    pub bound: u8,
    pub relation: u8,
    pub body: u8,
    pub maximum: bool,
}

impl Selection {
    fn activation(&self, candidate: World, inner: Option<World>) -> bool {
        self.body == 0 || signed(self.body - 1, candidate.0[3], inner.map(|world| world.0[3]))
    }

    fn bound(&self, world: World) -> bool {
        let tuples: BTreeSet<_> = self
            .rows
            .iter()
            .filter(|row| world.operand(row.2) && eligible(row.3, world, None))
            .map(|row| row.0.map(|value| (value, row.1)))
            .collect();
        let value = if self.maximum {
            tuples
                .iter()
                .filter_map(|row| row.map(|row| row.0))
                .max()
                .unwrap_or(0)
        } else {
            tuples
                .iter()
                .filter_map(|row| row.map(|row| row.0))
                .min()
                .unwrap_or(8)
        };
        match self.relation {
            0 => value == self.bound,
            1 => value != self.bound,
            2 => value < self.bound,
            3 => value <= self.bound,
            4 => value > self.bound,
            5 => value >= self.bound,
            _ => unreachable!("six comparisons"),
        }
    }

    fn original(&self, world: World) -> bool {
        let activated = self.activation(world, None);
        let supported = [0, 1].into_iter().all(|head| {
            !world.operand(head)
                || world.0[2]
                || (activated
                    && self
                        .rows
                        .iter()
                        .any(|row| row.2 == head && eligible(row.3, world, None)))
        });
        (!world.0[2] || (world.operand(0) && world.operand(1)))
            && supported
            && (!activated || self.bound(world))
    }

    fn frozen(&self, candidate: World, inner: World) -> bool {
        let permissions = self.rows.iter().all(|row| {
            row.2 >= 2
                || !candidate.operand(row.2)
                || !self.activation(candidate, Some(inner))
                || !eligible(row.3, candidate, Some(inner))
                || inner.operand(row.2)
        });
        // Activated bound and necessary-support constraints freeze in M.
        // The only remaining J-relative requirements come from genuine atom
        // permissions and the seed program {g;o}.a:-g.c:-g.
        self.original(candidate)
            && (!candidate.0[2] || inner.0[2])
            && (!candidate.0[3] || inner.0[3])
            && (!candidate.0[2] || (inner.operand(0) && inner.operand(1)))
            && permissions
    }

    pub fn source(&self) -> String {
        let function = if self.maximum { "#max" } else { "#min" };
        let mut source = format!("{{g;o}}.a:-g.c:-g.{function}{{");
        for (index, &(value, key, head, condition)) in self.rows.iter().enumerate() {
            if index != 0 {
                source.push(';');
            }
            if let Some(value) = value {
                write!(source, "{},{key}", TERMS[usize::from(value)]).unwrap();
            }
            source.push(':');
            source.push_str(["", "not ", "not not "][usize::from(head / 4)]);
            source.push_str(["a", "c", "#true", "#false"][usize::from(head % 4)]);
            source.push_str(
                [
                    "",
                    ":g",
                    ":not g",
                    ":not not g",
                    ":a",
                    ":not a",
                    ":not not a",
                    ":c",
                    ":not c",
                    ":not not c",
                ][usize::from(condition)],
            );
        }
        let relation = ["=", "!=", "<", "<=", ">", ">="][usize::from(self.relation)];
        let body = ["", ":-o", ":-not o", ":-not not o"][usize::from(self.body)];
        write!(
            source,
            "}}{relation}{}{body}.",
            TERMS[usize::from(self.bound)]
        )
        .unwrap();
        source
    }

    pub fn check_frozen(&self) {
        let source = self.source();
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
        assert_eq!(names.len(), 4, "{source}");
        let world = |mask: usize| {
            World(["a", "c", "g", "o"].map(|name| {
                let index = names.iter().position(|other| other == name).unwrap();
                mask & (1 << index) != 0
            }))
        };
        for outer in 0..16 {
            let candidate = values(admitted.theory(), outer, None);
            assert_eq!(
                holds(admitted.theory(), &candidate),
                self.original(world(outer)),
                "{source}: M={outer}",
            );
            for inner in 0..16 {
                assert_eq!(
                    holds(
                        admitted.theory(),
                        &values(admitted.theory(), inner, Some(&candidate))
                    ),
                    self.frozen(world(outer), world(inner)),
                    "{source}: M={outer} J={inner}",
                );
            }
        }
    }
}
