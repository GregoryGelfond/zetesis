//! Closed ignored weights paired with independently specified model records.

use std::collections::BTreeSet;

use super::source_cases::Case;
use super::source_records::Records;

pub const WEIGHTS: [&str; 9] = [
    "foo", "-foo", "\"text\"", "\"\"", "f(1)", "-f(1)", "(1,2)", "#inf", "#sup",
];

pub const PROGRAM: &str = "{a;b}.";

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Minimize,
    Maximize,
    Weak,
}

pub const DIRECTIONS: [Direction; 3] = [Direction::Minimize, Direction::Maximize, Direction::Weak];

pub fn directive(direction: Direction, weight: &str, priority: i32, condition: &str) -> String {
    let tuple = format!("{weight}@{priority},k");
    match direction {
        Direction::Weak => format!(":~{condition}.[{tuple}]"),
        Direction::Minimize | Direction::Maximize => {
            let name = if matches!(direction, Direction::Minimize) {
                "minimize"
            } else {
                "maximize"
            };
            let condition = if condition.is_empty() {
                String::new()
            } else {
                format!(":{condition}")
            };
            format!("#{name}{{{tuple}{condition}}}.")
        }
    }
}

pub struct LiteralCase {
    pub reference: Case,
    pub priorities: Vec<i32>,
}

/// Both a and b are independent choices. Only b contributes numeric cost in
/// mixed cases; its duplicate source key is counted once at priority three.
fn records(mixed: bool) -> Records {
    (0..4)
        .map(|bits| {
            let atoms = ["a", "b"]
                .into_iter()
                .enumerate()
                .filter(|(index, _)| bits & (1 << index) != 0)
                .map(|(_, name)| name.into())
                .collect::<BTreeSet<_>>();
            let costs = mixed.then(|| vec![0, if bits & 2 == 0 { 0 } else { 2 }]);
            (atoms, costs)
        })
        .collect()
}

pub fn cases() -> Vec<LiteralCase> {
    let mut cases = Vec::new();
    for (index, weight) in WEIGHTS.into_iter().enumerate() {
        for direction in DIRECTIONS {
            for (body, condition) in ["", "a", "missing", "0=1"].into_iter().enumerate() {
                cases.push(LiteralCase {
                    reference: Case {
                        name: format!("weight_{index}_{direction:?}_body_{body}"),
                        source: format!("{PROGRAM}{}", directive(direction, weight, 7, condition)),
                        records: records(false),
                    },
                    priorities: vec![],
                });
            }
            for priority in [7, 9] {
                cases.push(LiteralCase {
                    reference: Case {
                        name: format!("weight_{index}_{direction:?}_mixed_{priority}"),
                        source: format!(
                            "{PROGRAM}{}#minimize{{0@7,k:a;2@3,k:b;2@3,k:b}}.",
                            directive(direction, weight, priority, "a")
                        ),
                        records: records(true),
                    },
                    priorities: vec![7, 3],
                });
            }
        }
    }
    cases
}
