//! Source-level selection truth, independent of the lowered aggregate DAG.
//!
//! The fixed rules `{g;o}.a:-g.c:-g.` give each generated atom possible producer
//! support without permitting either head when g is false. Expected candidate
//! truth includes the admitted theory's necessary-support guards; those guards
//! and the activated numeric bound are frozen candidate constraints.

use crate::support::finite_bindings as formula;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

pub(crate) fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}

pub(crate) fn models(source: &str) -> formula::Models {
    let admitted = input(source);
    assert_eq!(admitted.source().text(), source);
    let result = formula::native(&admitted);
    assert_eq!(result, formula::exhaustive(&admitted), "{source}");
    result
}

pub(crate) fn expected(records: &[&[&str]]) -> formula::Models {
    records
        .iter()
        .map(|record| record.iter().map(|name| (*name).to_owned()).collect())
        .collect()
}

pub(crate) fn external(source: &str) {
    let result = formula::external(source, true);
    assert_eq!(result["Models"]["More"], "no");
    let mut records = BTreeSet::new();
    let mut count = 0;
    for call in result["Call"].as_array().unwrap() {
        for witness in call["Witnesses"].as_array().into_iter().flatten() {
            let costs: Option<Vec<i64>> = witness["Costs"]
                .as_array()
                .map(|values| values.iter().map(|value| value.as_i64().unwrap()).collect());
            assert!(
                records.insert((
                    witness["Value"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_str().unwrap().to_owned())
                        .collect(),
                    costs,
                ))
            );
            count += 1;
        }
    }
    assert_eq!(result["Models"]["Number"].as_u64(), Some(count));
    assert_eq!(cost_records(source), records, "{source}");
}

pub(crate) fn cost_records(source: &str) -> BTreeSet<(BTreeSet<String>, Option<Vec<i64>>)> {
    let admitted = input(source);
    models(source)
        .into_iter()
        .map(|names| {
            let model = zetesis_core::Model::from_positions(
                admitted.atom_catalog(),
                admitted
                    .atoms()
                    .iter()
                    .enumerate()
                    .filter_map(|(position, atom)| {
                        names
                            .contains(&formula::atom_text(atom))
                            .then_some(position)
                    }),
            )
            .unwrap();
            let evaluation = zetesis_objective::evaluate(
                admitted.objectives(),
                &model,
                zetesis_objective::Limits::default(),
                &zetesis_cpu::Cancellation::default(),
            )
            .unwrap();
            let score = evaluation.score();
            let costs = score
                .is_present()
                .then(|| score.costs().iter().map(|&(_, value)| value).collect());
            (names, costs)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct World([bool; 4]);

const OPERAND_KINDS: u8 = 4;
const SIGNED_OPERANDS: usize = 12;

impl World {
    fn head(self, index: u8) -> bool {
        let value = match index % OPERAND_KINDS {
            operand @ (0 | 1) => self.0[usize::from(operand)],
            2 => true,
            3 => false,
            _ => unreachable!("two atoms and two Boolean values"),
        };
        signed(index / OPERAND_KINDS, value, None)
    }
}

fn signed(sign: u8, outer: bool, inner: Option<bool>) -> bool {
    match sign {
        0 => outer && inner.unwrap_or(true),
        1 => !outer,
        2 => outer,
        _ => unreachable!("three literal signs"),
    }
}

fn eligible(mode: u8, outer: World, inner: Option<World>) -> bool {
    if mode == 0 {
        return true;
    }
    let index = match (mode - 1) / 3 {
        0 => 2,
        1 => 0,
        2 => 1,
        _ => unreachable!("gate and two recursive eligibility atoms"),
    };
    signed(
        (mode - 1) % 3,
        outer.0[index],
        inner.map(|world| world.0[index]),
    )
}

/// Each row is (numeric value, tuple suffix, head, eligibility).
/// Heads enumerate a, c, true, false under no, single, then double negation.
/// Measures are min, max, count, sum, sum+, and implicit choice count.
pub(crate) struct Selection {
    pub rows: Vec<(i32, u8, u8, u8)>,
    pub lower: i32,
    pub upper: i32,
    pub body: u8,
    pub measure: u8,
}

impl Selection {
    fn activation(&self, outer: World, inner: Option<World>) -> bool {
        self.body == 0 || signed(self.body - 1, outer.0[3], inner.map(|world| world.0[3]))
    }

    fn bound(&self, world: World) -> bool {
        let active = |row: &(i32, u8, u8, u8)| world.head(row.2) && eligible(row.3, world, None);
        let tuples: BTreeSet<_> = self
            .rows
            .iter()
            .filter(|row| active(row))
            .map(|row| (row.0, row.1))
            .collect();
        let value = match self.measure {
            0 => tuples.iter().map(|row| i64::from(row.0)).min(),
            1 => tuples.iter().map(|row| i64::from(row.0)).max(),
            2 => Some(i64::try_from(tuples.len()).unwrap()),
            3 | 4 => Some(tuples.iter().map(|row| i64::from(row.0)).sum()),
            5 => {
                // Atom keys include their sign. Boolean source occurrences
                // have distinct keys even when their truth values agree.
                let identities: BTreeSet<_> = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| active(row))
                    .map(|(index, row)| {
                        if row.2 % OPERAND_KINDS < 2 {
                            usize::from(row.2)
                        } else {
                            index + SIGNED_OPERANDS
                        }
                    })
                    .collect();
                Some(i64::try_from(identities.len()).unwrap())
            }
            _ => unreachable!("six aggregate measures"),
        };
        // Empty extrema lie outside every finite closed interval.
        value.is_some_and(|value| i64::from(self.lower) <= value && value <= i64::from(self.upper))
    }

    fn original(&self, world: World) -> bool {
        let active = self.activation(world, None);
        let supported = [0, 1].into_iter().all(|head| {
            !world.head(head)
                || world.0[2]
                || (active
                    && self
                        .rows
                        .iter()
                        .any(|row| row.2 == head && eligible(row.3, world, None)))
        });
        (!world.0[2] || (world.head(0) && world.head(1)))
            && supported
            && (!active || self.bound(world))
    }

    fn frozen(&self, outer: World, inner: World) -> bool {
        let permissions = self.rows.iter().all(|row| {
            row.2 >= 2
                || !outer.head(row.2)
                || !self.activation(outer, Some(inner))
                || !eligible(row.3, outer, Some(inner))
                || inner.head(row.2)
        });
        // The candidate must satisfy every original constraint. Only genuine
        // atom permissions and the fixed seed rules can retain atoms in J.
        self.original(outer)
            && (!outer.0[2] || inner.0[2])
            && (!outer.0[3] || inner.0[3])
            && (!outer.0[2] || (inner.head(0) && inner.head(1)))
            && permissions
    }

    pub fn source(&self) -> String {
        let function = ["#min", "#max", "#count", "#sum", "#sum+", ""][usize::from(self.measure)];
        let mut source = format!("{{g;o}}.a:-g.c:-g.{}{function}{{", self.lower);
        for (index, &(value, key, head, condition)) in self.rows.iter().enumerate() {
            if index != 0 {
                source.push(';');
            }
            if self.measure != 5 {
                write!(source, "{value},{key}:").unwrap();
            }
            source.push_str(["", "not ", "not not "][usize::from(head / OPERAND_KINDS)]);
            source.push_str(["a", "c", "#true", "#false"][usize::from(head % OPERAND_KINDS)]);
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
        let body = ["", ":-o", ":-not o", ":-not not o"][usize::from(self.body)];
        write!(source, "}}{}{body}.", self.upper).unwrap();
        source
    }

    pub fn check_frozen(&self) {
        let source = self.source();
        let admitted = input(&source);
        let names: Vec<_> = admitted.atoms().iter().map(formula::atom_text).collect();
        assert_eq!(
            names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["a", "c", "g", "o"])
        );
        let world = |mask: usize| {
            World(["a", "c", "g", "o"].map(|name| {
                let index = names.iter().position(|other| other == name).unwrap();
                mask & (1 << index) != 0
            }))
        };
        for outer in 0..16 {
            let candidate = formula::values(admitted.theory(), outer, None);
            assert_eq!(
                formula::holds(admitted.theory(), &candidate),
                self.original(world(outer)),
                "{source}: M={outer}"
            );
            for inner in 0..16 {
                assert_eq!(
                    formula::holds(
                        admitted.theory(),
                        &formula::values(admitted.theory(), inner, Some(&candidate))
                    ),
                    self.frozen(world(outer), world(inner)),
                    "{source}: M={outer} J={inner}",
                );
            }
        }
    }
}
