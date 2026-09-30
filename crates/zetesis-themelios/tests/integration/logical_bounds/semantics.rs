//! Original and frozen truth from declared source rows, not compiler nodes.

use super::{canonical, holds, input, values};

#[derive(Clone, Copy)]
struct Truth {
    whole: bool,
    frozen: bool,
}

impl Truth {
    fn constant(value: bool) -> Self {
        Self {
            whole: value,
            frozen: value,
        }
    }

    fn atom(whole: bool, inner: bool) -> Self {
        Self {
            whole,
            frozen: whole && inner,
        }
    }

    fn and(self, other: Self) -> Self {
        Self {
            whole: self.whole && other.whole,
            frozen: self.frozen && other.frozen,
        }
    }

    fn or(self, other: Self) -> Self {
        Self {
            whole: self.whole || other.whole,
            frozen: self.frozen || other.frozen,
        }
    }

    fn implies(self, other: Self) -> Self {
        let whole = !self.whole || other.whole;
        Self {
            whole,
            frozen: whole && (!self.frozen || other.frozen),
        }
    }

    fn negate(self) -> Self {
        self.implies(Self::constant(false))
    }

    fn choice(self) -> Self {
        self.or(self.negate())
    }
}

// A finite numeric measure compared with a nonnumeric bound accepts either all
// or none of its masks. Build the canonical subset formula explicitly: a
// classical tautology alone would not determine its arbitrary M/J reduct.
fn aggregate(elements: &[Truth], accepts: bool) -> Truth {
    let mut result = Truth::constant(true);
    for selected in 0..1 << elements.len() {
        if accepts {
            continue;
        }
        let mut antecedent = Truth::constant(true);
        let mut consequent = Truth::constant(false);
        for (index, &element) in elements.iter().enumerate() {
            if selected & (1 << index) != 0 {
                antecedent = antecedent.and(element);
            } else {
                consequent = consequent.or(element);
            }
        }
        result = result.and(antecedent.implies(consequent));
    }
    result
}

/// The first two rows alias one complete tuple. Unsigned atomic permissions
/// remain separate; the signed atom and Boolean rows do not supply producers.
pub fn check(bound: &str, relation: &str, accepts: bool, measure: usize, context: usize) {
    let function = ["", "#count", "#sum", "#sum+"][measure];
    let weight = if measure == 3 { 2 } else { -2 };
    let source = if context == 0 {
        let rows = if measure == 0 {
            "a:c;c:not a;not not a:g;#true:not c".into()
        } else {
            format!("{weight},0:a:c;{weight},0:c:not a;3,1:not not a:g;0,2:#true:not c")
        };
        format!("{{g;o}}.a:-g.c:-g.{function}{{{rows}}}{relation}{bound}:-o.")
    } else {
        // Ordinary body cardinality uses the same count measure; explicit
        // tuples keep this reference's three independently declared keys.
        let function = if measure == 0 { "#count" } else { function };
        let sign = ["", "not ", "not not "][context - 1];
        format!(
            "{{a;c;g;o}}.:-o,{sign}{function}{{{weight},0:a,c;{weight},0:c,not a;3,1:not not a,g;0,2:not c}}{relation}{bound}."
        )
    };
    let admitted = input(&source);
    let names: Vec<_> = admitted.atoms().iter().map(canonical).collect();
    assert_eq!(names.len(), 4, "{source}");
    for outer in 0..16 {
        let original = values(admitted.theory(), outer, None);
        for inner in 0..16 {
            let [a, c, g, o] = ["a", "c", "g", "o"].map(|name| {
                let index = names.iter().position(|other| other == name).unwrap();
                Truth::atom(outer & (1 << index) != 0, inner & (1 << index) != 0)
            });
            let first = a.and(c);
            let second = c.and(a.negate());
            let selected = if measure == 0 && context == 0 {
                // Ordinary a and c use different atom keys. Only explicit
                // function tuples coalesce the first two source rows.
                vec![first, second, a.negate().negate().and(g), c.negate()]
            } else {
                vec![first.or(second), a.negate().negate().and(g), c.negate()]
            };
            let mut measure = aggregate(&selected, accepts);
            let expected = if context == 0 {
                let permission_a = o.and(c).implies(a.choice());
                let permission_c = o.and(a.negate()).implies(c.choice());
                // Necessary support filters are part of the emitted theory and
                // freeze in M. This reference includes them explicitly.
                let support_a = !a.whole || g.whole || (o.whole && c.whole);
                let support_c = !c.whole || g.whole || (o.whole && !a.whole);
                g.choice()
                    .and(o.choice())
                    .and(g.implies(a))
                    .and(g.implies(c))
                    .and(permission_a)
                    .and(permission_c)
                    .and(o.and(measure.negate()).negate())
                    .and(Truth::constant(support_a && support_c))
            } else {
                for _ in 1..context {
                    measure = measure.negate();
                }
                a.choice()
                    .and(c.choice())
                    .and(g.choice())
                    .and(o.choice())
                    .and(o.and(measure).negate())
            };
            assert_eq!(
                holds(admitted.theory(), &original),
                expected.whole,
                "{source}: M={outer}"
            );
            assert_eq!(
                holds(
                    admitted.theory(),
                    &values(admitted.theory(), inner, Some(&original))
                ),
                expected.frozen,
                "{source}: M={outer} J={inner}"
            );
        }
    }
}
