//! Independent finite selection and row-wise permissions for weighted aliases.
//!
//! `a :- g. c :- g. {g;o}.` supplies possible support for every generated head,
//! including recursive rows. It does not authorize either head when g is false.
//! The oracle never reads lowered nodes to compute its expected truth values.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::{canonical, holds, input, values};
use crate::support::head_element_reference::signed;

#[derive(Clone, Copy)]
struct World([bool; 4]);

impl World {
    fn head(self, second: bool) -> bool {
        self.0[usize::from(second)]
    }

    fn gate(self) -> bool {
        self.0[2]
    }

    fn activation(self) -> bool {
        self.0[3]
    }
}

fn condition(mode: u8, outer: World, inner: Option<World>) -> bool {
    if mode == 0 {
        return true;
    }
    let atom = |world: World| match (mode - 1) / 3 {
        0 => world.gate(),
        1 => world.head(false),
        2 => world.head(true),
        _ => unreachable!("three eligibility atoms"),
    };
    signed((mode - 1) % 3, atom(outer), inner.map(atom))
}

struct Selection {
    rows: Vec<(i32, u8, bool, u8)>,
    lower: i32,
    upper: i32,
    body: u8,
}

impl Selection {
    fn activation(&self, outer: World, inner: Option<World>) -> bool {
        self.body == 0
            || signed(
                self.body - 1,
                outer.activation(),
                inner.map(World::activation),
            )
    }

    fn original(&self, world: World) -> bool {
        let active: BTreeSet<_> = self
            .rows
            .iter()
            .filter_map(|&(weight, key, head, mode)| {
                (world.head(head) && condition(mode, world, None)).then_some((weight, key))
            })
            .collect();
        let total: i64 = active.iter().map(|&(weight, _)| i64::from(weight)).sum();
        let activated = self.activation(world, None);
        // Necessary-support guards test M only. The normal seed rules remain
        // real implications and do not establish unconditional head permission.
        let supported = [false, true].into_iter().all(|head| {
            !world.head(head)
                || world.gate()
                || (activated
                    && self
                        .rows
                        .iter()
                        .any(|&(_, _, other, mode)| head == other && condition(mode, world, None)))
        });
        (!world.gate() || (world.head(false) && world.head(true)))
            && supported
            && (!activated || (i64::from(self.lower) <= total && total <= i64::from(self.upper)))
    }

    fn frozen(&self, outer: World, inner: World) -> bool {
        let permissions = self.rows.iter().all(|&(_, _, head, mode)| {
            !outer.head(head)
                || !self.activation(outer, Some(inner))
                || !condition(mode, outer, Some(inner))
                || inner.head(head)
        });
        // Every root false in M freezes to false. The bound constraint and
        // necessary-support guards add no J-relative measurement or support.
        self.original(outer)
            && (!outer.gate() || inner.gate())
            && (!outer.activation() || inner.activation())
            && (!outer.gate() || (inner.head(false) && inner.head(true)))
            && permissions
    }

    fn source(&self, positive: bool) -> String {
        let function = if positive { "#sum+" } else { "#sum" };
        let mut source = format!("{{g;o}}.a:-g.c:-g.{}{function}{{", self.lower);
        for (index, &(weight, key, head, mode)) in self.rows.iter().enumerate() {
            if index != 0 {
                source.push(';');
            }
            let name = if head { "c" } else { "a" };
            let condition = [
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
            ][usize::from(mode)];
            write!(source, "{weight},{key}:{name}{condition}").unwrap();
        }
        let body = ["", ":-o", ":-not o", ":-not not o"][usize::from(self.body)];
        write!(source, "}}{}{body}.", self.upper).unwrap();
        source
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]

    #[test]
    fn weighted_aliases_match_independent_worlds(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, proptest::bool::ANY, 0_u8..10), 1..7),
        lower in -4_i32..=4, upper in -4_i32..=4,
        body in 0_u8..4, positive in proptest::bool::ANY,
    ) {
        let selection = Selection {
            rows: rows.into_iter().map(|(weight, key, head, mode)| {
                (if positive {weight.abs()} else {weight}, key, head, mode)
            }).collect(),
            lower, upper, body,
        };
        let source = selection.source(positive);
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
        proptest::prop_assert_eq!(names.len(), 4, "{}", source);
        let world = |mask: usize| {
            let member = |name| {
                names.iter().position(|atom| atom == name)
                    .is_some_and(|index| mask & (1 << index) != 0)
            };
            World(["a", "c", "g", "o"].map(member))
        };
        for outer in 0_usize..16 {
            let candidate = values(admitted.theory(), outer, None);
            proptest::prop_assert_eq!(
                holds(admitted.theory(), &candidate), selection.original(world(outer)),
                "{}: M={}", source, outer
            );
            for inner in 0_usize..16 {
                proptest::prop_assert_eq!(
                    holds(admitted.theory(), &values(admitted.theory(), inner, Some(&candidate))),
                    selection.frozen(world(outer), world(inner)),
                    "{}: M={} J={}", source, outer, inner
                );
            }
        }
    }
}
